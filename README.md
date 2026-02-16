# Project

# 2026 Feb 15

Random Brainstorming

- Thinking about a WillowContext API, analagous to gpui's `Context<Self>`
- GPUI Entities are well-typed handles to a piece of state, which happens to be
  held in memory in EntityMap
- A similar thought-pattern in Willow might be that a WillowObject is a "handle"
  (representation) of a key-value object model, which happens to be implemented
  as a list of Willow Paths to entries.
- So `WillowContext<Self>` might come from `willow.namespace(...)` or ...
- How about this: an object handle is like a schema and materialized view at
  the same time. It's like a query over a pattern of keys, which are used as
  the "fields" of the object.
- In reverse, the way we'd actually end up using it would be like this:

```rust
#[derive(WillowObject /* bikeshed name */)]
struct ProfileObject {
    // where `path` is some DSL for /path/{patterns}
    #[willow(path = "avatar.png")]
    avatar: Image<Png>,
    #[willow(path = "name.txt")]
    name: String,
}

fn init(cx: &mut App) {
    // experimenting query DSL, this would represent iterating/streaming over
    // a result set of objects
    let profiles: Vec<ProfileObject> = cx
        .willow()
        .user(/* User query API */)
        .namespace(/* Namespace query API */)
        .prefix(
            // Path query API
            "/apps/willow_object_app/profile_objects/"
        )
        // if the query is choosing from the set of all objects in the full willow space,
        // then a "projection" is a handle that represents that selection of objects
        .project()
    
    // impl ProfileObject {
    cx.willow()
        .query(/* Some Query representation */)
        // Note: Wouldn't this API need to handle a result _set_?
        // feels very Bevy
        .write(|this: &mut ProfileObject, cx: &mut WillowContext<ProfileObject>| {
            // Like gpui's Entities would have access to the App API,
            // we provide an API with necessary operations during write time
        }) // analagous to gpui's -> Subscription
    // } // end impl ProfileObject
}
```

In this API design, you can imagine the Willow Space as the set of all entries (key/value pairs)
in all namespaces and all subspaces (users). The problem the API needs to solve is for querying
and rendering objects from the filesystem into memory, and then allowing for attaching behavior.

So GPUI specifies Entities as handles to stateful objects (which happen to be stored in the
global context `App`), where those objects may specify
Similarly, Willow "objects"/entities would be handles to stateful objects (where the "fields" of
the object are a set of paths into a willow path space (namespace/subspace)).

We could implement traits for those objects, like how we have `Entity<State>` and `impl Render for State`
in GPUI, on the Willow side (continuing with the `ProfileObject` example above), we might have
`WillowObject<ProfileObject>`, which would be a handle that acts as an API over the set of objects
in the actual Willow Space (/store). Just like an `entity` needs access to the global context (`App`)
to read or write itself, so too would the `WillowObject` need access to the Willow store in order
to read or write or iterate through objects in the Willow Space.

- Willow Store as the term for on-disk representation, Willow Space as the in-memory application space?

Willow App: an application which can use user-keys and/or capabilities to read or write data into a
Willow Store. For example, a Calandar app might have an app key (which is just a user key used by
an app), and when using the calandar, you issue a capability for that Calendar app to come look into
your userspace, but restricted to the Area under a specific Path prefix, such as `/apps/calendar/`.

---

Case study: Calendar

I want to plan and implement a usable and useful calendar app, where I'll attempt to create a
daily-usable calendar app based on the Willow data model. I'll try to consider and express 
the concepts of namespaces and subspaces. I think the user-facing language I'd like to use in the
app's expression of the data model would be "Spaces" for namespaces and "Profile" for subspaces.

- In some way this would be a UX experiment of a "user friendly instantiation" of Willow's parameters's names.

The app experience should visually emphasize a sharp distinction between different Profiles, and
express a boldness in the presentation of signatures on signed objects or data. I want to make an effort
to visually directly expresss all of the Willow core concepts, because I think it's a great model to
build trust on and I think this can be made to be beautiful and powerful.

So significant but consistent representation of e.g. tags and signatures and data objects visually.
I'm actually now thinking to really lean into Objects as a central term. Imagine the fundamental principle
of the UI is that we have feeds of objects. A feed takes the objects of a given query and renders them
according to the objects' plugin definition.

> Slightly later me is thinking twice about using the unqualified term "Object".
> Need a term that is cohesive and fits both the user's mental model as well as has a direct correlation to
> the Willow fundamentals

So an "Object" Api here should have a graceful integration of the GPUI visual entity system and the Willow
data model API. Ideally these could be the exact same definition, e.g. a struct, which both exists as in-meory
state in the gpui EntityMap and whose type also expresses the relationship between the in-memory representation
and the in-willow expression of the object.

So an app that is a bare minimum chat room, is just a feed of objects, where all objects are "Chat" objects
with the same rendering rules in the feed.

You could pin a feed by somehow "tagging" the query used to produce that feed result.

Apps act as custom expressions of Objects, they take a data representation and render a unit UI, that is
meant to be rendered in a feed of some sort.

Could easily have custom "feed" visual implementations, that could add widgets or whatever else is wanted
in expressing this feed of content. i.e. customizing the container around a feed and perhaps the children

To make a photo album application, all you need is a plugin that can render "Objects" as album thumbnails.
The user still writes a query over objects like they always do, and any matching objects that can be rendered
get shown in the resulting feed. Custom "feed" component implementations could be used to make the feed
show thumbnails in rows of 3 or 4, allowing the grid style usually used for photo albums.

Calendar User Journey

- First time using the calendar
- Assuming I already have a profile. let's assume multiple profiles for generality's sake
- Calendar app maybe needs to get a read/write capability to an app data directory, not sure if an extra
  key could be needed here or just use user's key directly? Need to figure out user/app relationship.
- Use the "query interface" (a standard tag-applying query API?, like a searchbar with badges) to choose
  profiles and spaces whose data will be displayed in the view.
- Panel: display Profiles and Spaces that matched the query, and their counts
- Center Item: The calendar display, start with a google calendar looking clone, I need the week view but
  with the little month calendar
> Collapsed view should show a count of matches?
- Fundamental Operations:
  - Create Event (just a key/value object)
    - Dialog or popup or other form input. Simple key-value list, direct mapping to object
  - Edit Event (modify keys/values)
  - Delete Event
  - Add friend's profile to calendar
    - Two potential implementations?
    - 1) Give friend's key capability to write into namespace?
    - 2) ... an idea I was too slow to write down
    - 3) Oh, this is a case where namespace kind matters
      - A calendar hosted in an owned namespace would require any Profile to get a signed capability
        from the namespace owner before being allowed to read or write anything, even with their own key.
      - A calendar in a communal namespace would not require any permissions to be given by the
        namespace. The Profile key (aka subspace aka user key) has its own authority to read and write
        into its own subspace within the communal namespace.

---

# 2026 Feb 14

- More imaginary reasons to call it `tagt`
  - `/tagged` might be used generically, doesn't feel unique enough
  - `/tagd` might be interpreted as "tagdee" as if to indicate a daemon
  - `/tagt` or `tagt` (an executable) are both short, quirky, and seems unique enough
  - Another one: `.expect("tagged path")` sounds useless, `.expect("tagt path")` feels
    more intuitive, as `tagt` is clearly a proper noun because it's unusual/unassociated.

- Imagining a new mental model for next-generation apps
  - In the context of Willow

- Namespaces are unique filesystem roots. Could imagine "mounting" namespaces into a
  global filesystem where each namespace's public key is a prefix into the subdirectory
  where that key has full authority, i.e. the root of the namespace, e.g. `/<namespace>/`

- Users are digital entities represented by a key. So-called "user keys" grant access to
  a "subspace" named after themselves. So a user's root directory from the view of a namespace
  would be located at `/<namespace>/<subspace a.k.a. user key>/`

- Apps may also be entities represented by keys, and they may request to read or write to
  a directory in your namespace named after their public key. Maybe there's standard
  presets of permissions for applications, for example allowing apps to keep a data
  directory, e.g. `/<namespace>/<subspace>/apps/<app_public_key>/`. The `/apps` path would
  be pure convention, it would be possible to mint a capability for an app's key that
  would give access to your entire subspace if you wanted.

- Setting good conventions is worth an intentional think
  - Convention idea: Namespace named after a subspace (user key) is the user's home "space" (directory)
    - `let user_key = todo!()`
    - `/<namespace:user_key>/<subspace:user_key>/[USER's ROOT]` (maybe?)
    - `/<namespace:user_key>/<subspace:user_key>/home/` (leave the root available for user metadata/upkeep?)
    - `/<namespace:user_key>/<subspace:user_key>/apps/<app_key>` (data directory for apps?)
    - `/<namespace:user_key>/<subspace:user_key>/tags/`

  - "Tagged" implies graph data / view. Rather than an always-implied universal ordering for a "path",
    we navigate by querying through tags to reach data
  
    - Kind of like a path, but where the ordering doesn't matter?
    - `/namespace/alice/` equivalent to
    - `/alice/namespace/`
    - `/#namespace:family/#user:alice/#path:pattern`
    - Maybe shouldn't render as a path, counter-intuitive (paths are hierarchical)

    - Rendered more like tags shown maybe as badges or breadcrumbs?
    - `/#namespace:family_key/#user:alice/#other_tag:value`
    - `[#namespace:family_key] | [#user:alice] | [#other_tag:value]`
    - Tags can point to any object (/entity? GPUI entity??). E.g. treat namespaces as objects that can
      be tagged, user keys can be tagged, (paths can be tagged? path query of some sort like `/chat/**` or regex as a tag)

- This access would be granted via a Meadowcap capability describing the permission to
  access (in Willow terms) the Area in

- TODO Willow Store Zed Ext Trait API `cx.willow()`
- IDEA nix but in Rust and p2p-native (MINTED)

# 2026 Feb 9

- I want to try making a UI for `rad`
- Tried making rad-ui, but zed and radicle have different sqlite dependencies which
  clash on native link. I left that attempt in a dangling branch.
- Investigating rad's p2p, early/incomplete thoughts: it seems a message-oriented protocol
  rather than providing a streaming p2p API. I don't think there's hole-punching like iroh,
  though it can use tor

---

## Catchup notes (what I've been up to)

- Playing with Iroh's API. Successfully peering over relay and exchanging topic messages.
- Attempted using iroh-docs, ran into trouble or found the API difficult to understand.
- Attempted using `iroh-examples/iroh-automerge-repo` approach, automerge-repo (samod)
  seems to be having trouble `.find()`ing a document by ID.
- I _think_ I'm running the sync task correctly, but I need to double-check.
- I really _really_ want a working Willow store implementation, but while I'm waiting I'm
  trying to learn the pros and cons of different p2p systems so I can imagine what a good
  API should feel like.
- I'd love to attempt a global-context-pattern API for Willow. For this project I'd want
  to just integrate with Zed's entity system, but that may not be a viable design for
  Willow proper.
- I'm trying to imagine what a generalized version of GPUI's entity system might
  look like. The pattern of composition is very powerful, I can't help but think there
  should be a way to generalize it so that many different types of applications could
  compose with each other. The main problem I see is that some apps may not want the
  overhead of the entity map bookkeeping, not to mention GPUI's `App` type is heavy with
  desktop-specific inherent features such as windowing. Plus some apps may not want their
  state to be heap-allocated or restricted to a single-threaded context.
- I was imagining something like `App<(A, B, C, ...)>`, where `A`, `B`, and `C` would
  represent "inherent" capabilities of the App, so for example `A` might be Zed's entity
  system implementation, `B` might be the windowing API. This would be a generalization
  in the sense that certain apps could choose to be incredibly lightweight (`App<()>`
  having no inherent state), or choose some default or custom bundle of inherent capabilities.
  There might need to be a way for plugins to express compatibility only to apps that satisfy
  certain inherent capability requirements, something like `impl<A: Inherent<Entities>>`,
  indicating an app or plugin that is only compatible with apps that have the `Entities`
  inherent capability. Unfortunately this level of generic wizardry would probably make
  most of the API surface much more complicated.

# 2026 Jan 28

I want to write down some guiding values and principles for this project, because
I want to pull this together from a mess of ideas into something actionable and useful.

## Project Vision and Values

Today, I see the centralization of software as a threat to Democracy. In the last
few decades, we have allowed our core productivity tools to shift from the Desktop
to the Cloud. At the time, the appeal was a profound increase in the ability to
collaborate, such as multi-cursor editing in google docs. However, it came at the
cost of data ownership, privacy, and soverignty, because the form-factor of Cloud
technology puts the data in control of the providers, not the users.

Consider the Browser, which is a marvel of technology. It is less of an application
and more of an application-platform. It's ultimately a program that can fetch content
from a network and render it to the screen. The content may be static, like a blog,
or dynamic, drawn by application code (JS/WASM) that itself is a form of content.
The Browser is the platform which provides the tools to draw on screen, make network
requests, play audio and video, and read/write to disk, and also takes on the responsibility
for isolating sites from one another to protect user privacy.

However, the greatest weakness of the Browser is that it fundamentally is only a **portal**
to view data that lives remotely on a server. When you use any web service, you're
not interacting with _your data on somebody else's machine_, you're interacting with
_somebody else's data **about you** on their machine_. Under this model, users are at
the mercy of application providers:

- Content that was once available to you may become unavailable
- Content that was once free may become paywalled
- Data that you consider private may be used by providers to train AI
- Data about you may be shared with arbitrary third-parties
- Providers may track your browsing and build a profile of you for purposes
  like targeted advertising or surveillance
- Your interactions with other users may be tracked and profiled
- In social media settings, the content you see is determined _by the provider_
  rather than by you, so people can be divided and isolated in bubbles.
- Content can be suppressed or boosted by providers, influencing public opinion
  and radicalizing users

The vision of this project is to create a next-generation application platform which
puts users in control of their data by using durable local-first and peer-to-peer
foundations. The spirit of this project is staunchly anti-capitalist, anti-fascist,
pro-democracy, and pro-sovereignty. Human beings should be able to use digital systems
while maintaining an expectation of privacy and anonymity, while also enjoying the
capabilities of live collaboration and group sharing.

# 2026 Jan 6

Status

- Working libp2p swarm, can detect peers with MDNS, good enough to start testing
- Need to get libp2p-stream connections established, unlocks e.g. automerge-repo

Todo

- Want to create mock discord-esque ui, where discord servers now correspond to Willow
  namespaces (or subspaces, yet to be seen)
- Add contacts window like discord friends, populate suggestions from detected peers?

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
