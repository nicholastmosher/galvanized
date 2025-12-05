# Project

# 2025 Dec 5

I've written about this elsewhere but I can't find my notes so I'm going to write a summary here.
I want Zed to become a pluggable ecosystem for p2p local-first applications. I've already got a
fork of Zed that allows plugins in the shape of `fn init(cx: &mut App)` that works.
I want to prototype at least one full vertical including a p2p network and a distributed file system.
However I eventually want to have abstractions over network type and file system. For example, here
are building blocks I've been eyeing for some time:

- Willow for data store / filesystem
- Iroh for p2p network (or libp2p, etc.)
- Automerge for collaborative editing (but I also now see iroh-docs)
- Zed itself has a native CRDT Text type, I'd want other doc formats to be pluggable so they could
  be used first-class in Zed buffers/editors.

One of the main challenges I foresee trying to abstract over several types of systems is needing some
way to unify an identity system. P2p systems tend to use some kind of public key for identity, but it's
not guaranteed for this to be the case and even so, there may be different key types. For example, Iroh
and Libp2p may have different formats used for public keys / Peer IDs. I think there may be a way to use
signed messages as a way to "associate" identities/keys, but I also worry about leaking such associations
in a world where privacy/anonymity should be treated as first-class.

For now, I think I'll choose one full vertical stack and try to make an end-to-end prototype. Today I'm
mostly interested in Willow, Iroh, and Automerge. I see Iroh already has a working example of an automerge
repository. I think that Willow could serve as more of a "cold store" which is also the basis for permissioning,
given that Willow+Meadowcap already have permissioning and privacy built in mind. I think Automerge documents
could be cold-saved into Willow and be subject to Namespace/Subspace rules and capability sharing. On opening
a document, we could create a "Session" which is essentially loading into automerge-repo/samod to do live
editing. We could write a Session file into Willow, then when it syncs to other Willow peers, the hosting
Zed plugin could visually show that the document is being edited by somebody (subject to the visibility rules
of Willow namespaces).

Side thought: Could a standard OS filesystem be expressed as a special case of a Willow namespace? In other
words, could Willow's own API be used as a filesystem abstraction that Zed could be pointed to, so that Zed
could be a first-class editor over both OS and Willow filesystems? Similarly, could we find an abstraction
that encompasses both Automerge and Zed CRDTs?

Today's plan (because I need to focus)

- Follow the iroh-examples/iroh-automerge-repo example and try to integrate it visually in Zed
- Can start with ad-hoc Iroh keys but eventually need a key store / identity/contact management system
