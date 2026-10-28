# T4 — Capability-gated automerge shared objects

**Status:** DRAFT
**Depends on:** T1 (signed-data capabilities), T3 (p2p messaging, for
capability exchange over the peer boundary)

## Why

A second p2p-communication tool, alongside T3's message passing, is
**direct shared objects** synced by automerge (samod). Today samod
syncs a document with any peer that dials it; there is no enforcement
that the peer is authorized to read or write that object. The
requirement: a shared object must only be synced with peers who **hold
and prove** an explicit capability to read or write that object,
enforced by the sync engine. The capability comes from Willow (the
signed-data form from T1) and is presented and verified at sync time.

## Current state (anchors — re-derive before leaning on)

- `plugin-automerge`: `crates/plugin-automerge/src/lib.rs`. `init`
  observes new `Profile`s and registers an `AutomergeProtocol` on the
  Profile's p2p endpoint, building a `samod::Repo` with
  `InMemoryStorage` (the on-disk `TokioFilesystemStorage` lines are
  commented out at `:29`).
- `Automergable` trait (`:47`) + `AutomergeHandle` (`:56`) give
  `Entity<T: Automergable>` a `read`/`set`/`update` API over the
  `DocHandle`. This is the local handle; it does not gate peer sync.
- `IrohSamod` (the samod-over-Iroh glue, used in `plugin-p2p/src/v1.rs`
  via `crate::iroh_repo`) does `dial_peer` and `repo.when_connected`;
  nothing checks a capability before establishing sync.
- `crates/plugin-contacts/src/protocol.rs:130` — `ContactPeer::new`
  creates an automerge doc and hands the `DocumentId` to the remote
  peer over the control stream (`RequestConnectContact` /
  `AcceptConnectContact`). The contact handshake authenticates the
  peer's Profile (via `SignedEnvelope`) but does **not** gate the
  automerge sync on a read/write capability for the shared object.
- Relevant open questions in `OPEN_QUESTIONS.md`: "Multi-member chat /
  shared-doc permissioning" and "CRDT + Willow capability composition"
  ("How to lift Willow permissioning into CRDT participation so a
  leaked token isn't enough to join?").

## Scope

1. **Define the capability for a shared object.** A shared object
   lives at a Willow path (or is identified by the doc id bound to a
   Willow path). Its owner mints a signed `ReadCapability` /
   `WriteCapability` (the T1 signed-data form) naming the object and
   the authorized recipient Profile. The capability is shared out of
   band (e.g. in a contact ticket / a T3 message) and presented by the
   recipient at sync time.

2. **Augment the sync engine to enforce capabilities.** Before
   establishing an automerge sync session for a document, the sync
   engine requires the peer to present a valid capability for that
   document:
   - Signature verifies against the owner's Profile key.
   - Recipient matches the local Profile.
   - Scope matches the document/object being synced.
   - Read vs write: a read-only capability must not let the peer push
     changes (the sync engine applies the direction).
   On failure: refuse the sync, surface the error (visible, not
   silent — `CONVENTIONS.md`).

3. **Capability exchange.** Use T3's message-passing to carry the
   capability over the peer boundary, OR keep it in the existing
   contacts handshake (which already carries signed envelopes). Decide
   with Nick. The capability is a signed data object; it should be
   verifiable offline (no round-trip to the owner needed).

4. **"Leaked token isn't enough" question.** Address the
   `OPEN_QUESTIONS.md` "CRDT + Willow capability composition" concern:
   a leaked capability token alone should not be sufficient to join a
   live session. Options to rule on with Nick: (a) capability must be
   presented fresh per session and the owner revokes via Willow
   namespace capability invalidation; (b) the sync engine consults the
   owner's Willow namespace for a revocation list before accepting;
   (c) a handshake challenge proving the recipient still holds the
   corresponding Profile key. Recommendation: (a) + revocation via
   Willow entry removal, with the sync engine re-checking on
   reconnect. This is security-shaped — Nick must rule.

5. **`AutomergeHandle` direction awareness.** `Entity<T:
   Automergable>`'s `set`/`update` (`plugin-automerge/src/lib.rs:97` /
   `:111`) should refuse writes when the local holder only has a
   read capability. Plumb a read/write mode into the handle bound at
   sync-establishment time.

## Out of scope

- Multi-member chat permissioning UI (a future app-level concern on
  top of this). T4 provides the *mechanism*; chat builds the *policy*.
- Automerge on-disk storage (the commented `TokioFilesystemStorage`).
- A capability-management UI. Out of scope; capabilities are minted
  programmatically here.

## Design questions for Nick

1. **Capability exchange path.** T3 message vs. existing contacts
   handshake vs. Willow entry (the owner writes the capability to a
   Willow namespace the recipient can read). Recommendation: T3 message
   for the initial grant; Willow entry for revocation. Confirm.

2. **"Leaked token" mitigation.** See scope §4. This is the security
   call. Recommendation: per-session presentation + revocation via
   Willow entry removal + re-check on reconnect.

3. **Read/write enforcement granularity.** Does samod expose a hook to
   make a sync session read-only (drop outgoing changes)? If not, is
   enforcing "read-only" a hard requirement now, or can T4 ship
   write-only-gated (you either have write or you can't sync) and add
   read-only later? Recommendation: ship write-gated now; read-only as a
   fast follow — but flag if samod can't express it.

4. **Object identity.** Is the capability scoped to the `DocumentId`,
   or to a Willow path the doc is bound to? Recommendation: Willow
   path (so revocation is a Willow operation, consistent with T1),
   with the `DocumentId` carried as a claim inside the capability.

## Acceptance criteria

- A peer without a valid capability cannot establish an automerge sync
  for a gated document; the attempt is refused with a visible error.
- A peer with a read capability can sync (receive changes) but cannot
  push (or, per §3, is gated at the write level only if read-only is
  deferred — state this explicitly).
- Revoking the capability (per the ruled-on mechanism) prevents future
  sync; an in-flight session is torn down or refuses further changes.
- The existing contacts shared-display-name doc (`ContactPeer`) is
  migrated to the gated path as the reference consumer.

## Verification

- `cargo fmt && cargo clippy && cargo test` for `plugin-automerge`,
  `plugin-willow`, and the migrated consumer.
- Two-peer test: peer A mints a read cap for peer B; B syncs; A mints
  no cap for peer C; C's sync attempt is refused and the error is
  visible. If two live instances aren't drivable, a two-`Endpoint`
  in-process test is acceptable — state what you ran.

## Records

- Move "Multi-member chat / shared-doc permissioning" and
  "CRDT + Willow capability composition" open questions to
  `DECISIONS.md` once ruled on.
- Add a "capability-gated sync" section to `ARCHITECTURE.md`.