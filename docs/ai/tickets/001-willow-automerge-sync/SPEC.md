# SPEC — Willow-gated automerge sync

> **Target-state specification.** The system we are building *toward*. Where
> this spec and the current code disagree, the spec is the goal; the code is
> the present. Rulings are RATIFIED (Nick); sub-decisions still open are
> PROPOSED. Nothing here is implemented yet.

## Purpose

Build **`galvanized-sync`**: a from-scratch automerge sync engine that only
syncs documents between peers who **hold and prove** a valid signed Willow
capability for that document. This is L0's "permission without a referee"
(Q3) and "two enforcement points, one truth" (Q4) applied to the dynamic-data
substrate (L2), and it satisfies T4's acceptance criteria.

> **Privacy is the primary case; public sharing is a granted exception.**
> Every sync requires positive proof of a valid signed capability. There is
> no ungated path. "Public" is a kind of capability, not a mode of the engine.

## RATIFIED design decisions

1. **New crate `galvanized-sync`**, from scratch. `samod`/`samod-core` are a
   reference (sans-IO actor shape, codec ideas), not a dependency or
   compatibility target. No `automerge-repo`/JS interop. *(Supersedes the
   earlier "extend samod-core with seams" ruling.)*
2. **Capabilities are presented in the extended sync handshake.**
3. **Challenge-response per session.** Recipient-specific: prove the *named*
   `SubspaceId`. Public: prove *some* `SubspaceId` (sybil-resistant identity;
   the cap doesn't restrict *which*).
4. **Revocation: re-check on reconnect first;** in-flight teardown is a fast
   follow.
5. **Write-gate first; read-only `SyncMode` is a fast follow.**
6. **`DocumentId` and Willow path are independent**, bound by the capability's
   signed claims.
7. **Revocation via the `CapabilityStore` seam** (today: attributable-public
   tombstones in a per-peer automerge index doc; recipient-specific
   out-of-band; anonymous-public in a future identity-less store).
8. **One new protocol + ALPN.** No vanilla ALPN, no `automerge-repo` compat.
9. **Public caps: read-only, tight TTL, revokable.** Revoking stops *future*
   sync, not already-synced bytes (stated honestly).
10. **Both public modes valid:** anonymous-public (hard requirement) and
    attributable-public.
11. **Anonymous-public signed by the namespace secret** (owned namespace);
    attributable-public & recipient-specific signed by the owner's
    `SubspaceSecret`. Communal namespaces are attributable by nature.
12. **`CapabilityStore` trait seam, swappable.** Self-sufficient now
    (automerge index doc + local store); Willow-namespace-path impl later.

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│ Galvanized app plugins (chat, contacts, form, …)               │  above
├──────────────────────────────────────────────────────────────┤
│ plugin-automerge (new-protocol wiring + Automergable seam)     │
├──────────────────────────────────────────────────────────────┤
│ galvanized-sync  (the new engine)                              │
│  • Sans-IO actor core (Hub + DocumentActor) — inspired by samod│
│  • Peer identity = Willow SubspaceId                           │
│  • Trait seams (for OUR evolution):                            │
│      PeerAuthenticator, CapabilityValidator,                   │
│      CapabilityStore, SyncMode                                 │
│  • Built-in Willow impls are the defaults (no no-op open path) │
│  • New wire protocol + ALPN (carries identity + caps natively) │
│  • Deny-by-default: no valid signed cap → no sync              │
├──────────────────────────────────────────────────────────────┤
│ galvanized-cap  (AutomergeSyncCapability + WillowScope + keys) │
├──────────────────────────────────────────────────────────────┤
│ Willow25 + Iroh (transport: QUIC, hole-punching)               │
└──────────────────────────────────────────────────────────────┘
```

The seams exist so we can refine rules and swap implementations (notably the
`CapabilityStore`) over time. They are **not** for wrapping samod, and the
defaults are the real Willow implementations, not permissive stubs.

## Peer identity

- A peer is a **Willow `SubspaceId`** (public key from a `SubspaceSecret`).
  This replaces samod's ephemeral `PeerId` entirely.
- The handshake proves the peer holds the corresponding `SubspaceSecret` by
  signing a challenge (ruling #3).
- `PeerIdentity` is the post-handshake per-connection handle: the verified
`SubspaceId` plus the granted capability set. It is the input to every
per-doc gating decision.

## The capability object

`AutomergeSyncCapability` (in `galvanized-cap`, extending existing
`WillowScope` + signed-cap machinery):

```
AutomergeSyncCapability {
  signer: Signer,                // Subspace(SubspaceId) | Namespace(NamespaceId)
  recipient: Recipient,          // Subspace(SubspaceId) | Public
  document_id: DocumentId,       // the automerge doc authorized
  willow_scope: WillowScope,     // (namespace, subspace, path_prefix) binding
  permission: AutomergeSyncPermission,  // Read | Write
  expiry: UnixTimestamp,
  nonce: [u8; 32],               // revocation tracking id
}
signed by the signer's secret
```

- **Signer (ruling #11):** `Subspace(SubspaceId)` for recipient-specific &
  attributable-public (names the owner Profile); `Namespace(NamespaceId)` for
  anonymous-public (names only the namespace, not a Profile). Validator checks
  the signature against the carried public key.
- **Recipient:** `Subspace(SubspaceId)` for private; `Public` for public caps.
- **Public caps (ruling #9):** `permission = Read`, tight TTL, revokable.
  No public `Write`.
- **Binding (ruling #6):** `document_id` and `willow_scope` independent; the
  signed claims bind them. No external registry.
- **Dual form (L0):** the serialized object is the sync-checked ground truth;
  a Rust type-token is minted from a verified object to gate the local API.
- **Minting:** gated by `StrictSendCap<WillowWrite, WillowScope>` (reusing
  `WillowKeyStore`); the signer secret (subspace or namespace) signs.
- **Honest limit:** revoking a public cap stops future sync, not already-synced
  bytes.

## The handshake (connection-level gate)

On the new galvanized ALPN, before normal sync:

1. Exchange `SubspaceId`s + fresh challenge nonces.
2. Each side signs the other's nonce with its `SubspaceSecret`; verify →
   proves key possession (leaked-token mitigation).
3. Each side presents `AutomergeSyncCapability`(s) it holds (sourced via the
   `CapabilityStore`), for docs the other owns or that are public. Each side
   verifies: signature valid against the signer's public key, recipient
   matches the local `SubspaceId` or is `Public`, permission covers the
   direction, not expired, nonce not revoked.
4. On success, record a `PeerIdentity` and proceed. On failure, refuse with a
   visible `Error` (never a silent close). **No fallback to ungated sync.**

## Per-document gating (message-level gate)

The engine's `CapabilityValidator` is consulted on every `Request`/`Sync`:

- Input: `(peer_identity, document_id, direction)`.
- Output: `Allow | Deny(reason)`.
- Check: a presented cap whose `document_id` matches, `recipient` is the peer
  or `Public`, `permission` covers the direction, not expired, nonce not in
  the live revocation view.
- On `Deny`: emit a visible `Error` for that doc and skip its sync. The
  connection stays up — other docs may be authorized.
- The validator reads the revocation view **at call time**, so a revocation
  landing mid-session takes effect at the next message (best-effort, on top
  of the reconnect re-check floor).

## Capability distribution — `CapabilityStore` seam

No syncing Willow store exists yet, so the engine is self-sufficient. The
`CapabilityStore` trait abstracts where caps live and how they propagate:

- **Attributable-public caps + tombstones:** a per-peer public-read-only
  automerge index doc, synced by the engine itself. Caps are immutable
  append-only entries; tombstones revoke. Bootstrap: the index doc id travels
  out-of-band (contact ticket); thereafter the index is the source of further
  public caps.
- **Recipient-specific caps:** out-of-band (contact ticket / direct message /
  the handshake). Held locally. **Never** in a public index (would publish
  the sharing graph).
- **Anonymous-public caps:** need an identity-less store (future Willow
  namespace path, or travel-with-the-doc). Until then, travel out-of-band like
  recipient-specific. Behind the same seam.

Swappable: a future Willow-namespace-path `CapabilityStore` impl replaces the
index-doc impl without touching the engine.

## Revocation

- Tombstones (via the `CapabilityStore`): a revoked cap's `nonce` is recorded
  so the validator refuses it. For attributable-public, the tombstone is an
  append to the per-peer index doc (propagates by syncing it). For
  recipient-specific, a local tombstone + out-of-band notification.
- **Re-check on reconnect** (ruling #4 floor): sync the revocation view before
  accepting a peer's caps.
- **Mid-session best-effort:** the validator's live-view read denies the next
  message after a tombstone lands locally.
- **In-flight teardown** (active session kill on revocation) is a fast follow.
- **Honest limit for public caps:** revocation stops future sync; peers who
  already synced keep the bytes.

## Read-only (fast follow, ruling #5)

First deliverable is write-gated: no `Write` cap → no sync. Read-only
(`Read` = receive but not push) is ticket 008: `SyncMode::ReadOnly` per
connection, set by the cap permission. The plumbing lands in 001 so 008 is a
policy activation, not a re-arch.

## Sync-authority vs read-authority (L0 §5, restated)

The engine governs **propagation** (sync-authority, a capability).
**Decryption** (read-authority, a key) is a separate layer above Willow and
is **not the engine's concern**. The engine syncs ciphertext to a
sync-authorized peer who may lack the read key. The two authorities are
independent and both originate in L0.

## Acceptance criteria (carried from T4, distributed across tickets)

- A peer without a valid capability cannot sync a gated document; the attempt
  is refused with a visible error. (004, 005)
- A peer with a `Write` cap can sync (push + receive). (005)
- A peer presenting a cap without proving key possession (stolen token) is
  refused. (004)
- Revoking a cap prevents future sync after reconnect. (006)
- A public doc (anonymous or attributable) syncs to any peer presenting its
  public cap + key proof; a non-public doc does not. (004, 005)
- The contacts shared-display-name doc migrates to the gated path. (007)
- (Fast follow) A `Read` cap lets a peer receive but not push. (008)

## Out of scope for this arc

- The encryption-at-rest *mechanism* (L0 governs its keys; the engine only
  carries ciphertext).
- A capability-management UI (caps minted programmatically here).
- Capability delegation/chaining (mint-by-signer only in v1).
- In-flight revocation teardown (fast follow).
- The syncing Willow store itself (the `CapabilityStore` seam lets us plug it
  in later).
- Discovery (out-of-band tickets, per `OPEN_QUESTIONS.md`).