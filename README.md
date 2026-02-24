# Project

# 2026 Feb 23

> pm

- Maybe work on Object rendering
- Maybe work on cleaning up data modeling interface?
- Goal is an interface that allows one data definition
  to be used across UI and storage (and maybe remotes)
- aside: `trait ContentAddressable { type Address; fn address(&self) -> Result<Self::Address>; }`
- Some UI trait describing typical CRDT workflows, to auto-translate CRDT operations
  (abstractly/generically) into UI wrapping/decoration
- Try to make a glue between UI and data that makes a visual language for data

# 2026 Feb 23

> am

- Working on ObjectWidget
- Want to make a generic interface to bridge with CRDTS
  - Get live dynamic rendering of CRDT data
- For now just to get rendering sorted, using serde_json::Value

# 2026 Feb 21

- Term idea: Create a "lens" of a particular object type at a directory
- The lens sticks around like an object and lets you view that directory
  as that object quickly. Imagine the lens being tagged or pinned.

---

- More chat work
- Need interaction path from Profile > Namespace to > Create chat and then
  to open ChatUi item
  - Let's hardcode it for now and return later to generalize

```rust
/// Project from the Zed:Entity space to another space (like Willow Objects)
impl SomeExt for Entity<T> {
    type Handle<T> = ()
    fn to_other_handle<T>(&self) -> Self::Handle<T> {
        // pass the entity to the new handle type
        Handle::new(self.clone())
    }
}
```

```rust
// Zed already has:
pub trait ItemHandle: 'static + Send {
    // things in here can be domain-specific behavior as long
    // as it can be accomplished via an `Entity<Self>`
}
```

# 2026 Feb 19

Todo

- Chat use-case
- Need to instantiate new chat. How to go from 0 to 1 chat-feed objects?
- Profile > Space > Directory-to-chat-data > chat room (feed) id > objects
- In Path view, at the bottom in a dashed rounded box (like ButtonInput),
  have row of "create object" buttons, based on the object types registered
  by all installed plugins.
- Bet

# 2026 Feb 18

How about: a tour of the codebase and a quick story about how this started
and how far it's come.

Firstly, the way I've been conducting this project is to just make a crate
and jam on it, trying experiments and iterating design attempts and going
with the flow. I try not to interrupt an idea when I'm having it, so if I
eventually hit a dead end, I'll just create a new crate to act as the top
level app. But GPUI kind of makes this into a strengh as well, because I
can keep any successful reusable components, such as UI elements.

> future me: I got into the conceptual weeds, but I wanted to mostly point
> out which code is recent and which is older:
>
> - `crates/tagged`: The (as of now) most recent iteration, and looking to
>   become top-level app integration point. I'm thinking that `tagged` is
>   a good name to stick with, I'll have to see if there's any reason not
>   - However, this one started out called `iroh-ui`, and was an iroh-heavy
>     experiment. Iroh worked pretty well! Relaying works out of the box,
>     but mdns still didn't work for me. I think I've messed up some local
>     network setting by installing things like docker and VMs.
>
> - `crates/willow-api-derive`: This one needed to be a separate crate due
>   to being a proc macro, but I didn't end up actually needing it yet. I
>   left the crate just to be a set-up scratchpad for when I do need macros.
>
> - `crates/libp2p-ui`: I was investigating `libp2p` as a potential p2p
> solution, but it felt a bit clunky to use its API in GPUI, so I dropped it.
>
> - `crates/iroh-automerge`: This was the first time I'd played with Iroh,
>   but I think I also didn't know what I was doing with GPUI yet, so I was
>   getting bogged down trying to do p2p but not knowing how to render what
>   the state of everything was.
>
> - `willow-rummager`: I think when I first made this repo was back when the
>   Willow crew was talking about doing a general-purpose explorer-type app.
>   I don't want to step on their toes with naming though, so I've decided
>   to name this app something different.


Ok, so I'll say I've been thiniking about this project for several years
now, but until about one year ago, I'd mostly felt that I didn't have good
enough foundations in all the tech that'd be needed. So I've been trying to
follow and learn over time, checking out different p2p, crdt, and UI crates
over the years.

So sometime last year, I started really diving into Zed's source to try to
learn about it. I'd been using Zed for some time and was highly impressed
with it, and the codebase is spectacular and makes me appreciate the project
that much more. The base patterns established by GPUI are so buttery smooth
to work with: when I'm creating a component, it's starting with a rectangle
and dividing inward. Using a tailwind DSL as the styling system was such a
brilliant move, it's so easy to make "good enough" UI by just chaining a
bunch of calls together.

At some point, when I was poking around `App` to see how it worked, I noticed
that the base structure of GPUI is essentially the same as Bevy's, where there's
one central `App`, and all of the app's state and behavior get installed to it.
In Bevy, they talk about how almost all of the functionality of the game engine
itself is defined as plugins, rather than being baked in as a native or intrinsic
behavior of the system. I think Bevy's entity system and scheduler are two
examples of "inherent" `App` behavior. I have definitely thought a lot about
whether it could be possible to make Bevy and Zed compose with each other, though
I haven't figured that out yet.

One of the great things about `Bevy`'s plugins is that they can be published as
crates, and installed into other applications. For example, a camera control
library could be written once but used in many applications. So there becomes an
inherent incentive for the community to build and share plugins, because it
raises the ceiling for future projects because of being able to reuse prior
solutions.

How does Bevy's composition work, and then what is similar about Zed's patterns?

In Bevy, a small app might look like this:

```rust
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(TagPlugin)
        .run();
}

struct TagPlugin;
impl Plugin for TagPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(NextTag(0.into()));
        app.add_systems(Startup, startup_system);
    }
}

#[derive(Component)]
struct Tag(u64);

#[derive(Resource)]
struct NextTag(AtomicU64);

fn startup_system(mut commands: Commands, next: Res<NextTag>) {
    // Create a first tag entity at startup
    commands.spawn(Tag(next.next()));
}
```

I personally _love_ Bevy's API, I've been being inspired by it for years now,
even though I've really never done an extended project with Bevy (lots of tiny ones).

So let me point out some patterns I think are super neat and deceptively powerful.

- The `App` is a container where all of the state and behavior live.
  - Imagine an infite filing cabinet, with a way to put or interact with things
    in each drawer.
  - Each drawer is an Entity, and each drawer may have zero or one Component of any type.
- There are two notable kinds of state management, Resources and Components.
  - There may be more, my knowledge of Bevy is very incomplete
- Resources are single values, and are looked up by their type. They are effectively
  the same thing as Zed's `Global`s.
- Components are pieces of data that can be attached to an Entity. Component data
  is stored in some internal representation within `App`.

- (some of) Bevy's behavior is expressed as "Systems", which are functions that run
  once per frame of the game (or some schedule).
- Systems use the "magic function" pattern, which can be seen elsewhere such as axum
  and actix-web. In Bevy, systems are just functions, and they define inputs and outputs.
  The cool part is that by changing the shape of a system, such as by adding another
  expected input, the App API `.add_systems(magic_function)`, through some trait magic,
  will effectively derive a plan for querying the inputs you asked for from the central
  App state (the `World`, in Bevy) and passing those inputs to your system when it's
  time to run.

Ok, so to summarize, Bevy has a bunch of cool APIs that allow for installing state and
behavior into the `App`, which then schedules and executes that behavior, calling the
magic system functions with references to the data it queried by just defining its own
inputs. In one little API, there's separation of state and behavior, composition of
common behavior as Plugins, and the ability to control the scheduling of the behavior.

Now, let's look at what we see in Zed:

Zed also uses what I like to call the "Global Context" pattern, because it has an `App`
and all of the state and behavior of the application get installed into it. Unfortunately,
Zed is lacking a Plugin API, and therefore has what I consider to be a composition
problem. I'll talk more about that later, but TLDR I have a fork of Zed which is very
minimal but completely solves the problem and unlocks the composition I want.

So what does Zed's state management story look like? It has two primitives (that I know),
which are `Global`s and `Entity<T>`s. As I mentioned before, Zed `Global`s are about
equivalent to Bevy's `Resource`s in that up to 1 instance of any type held in the context.
`Global`s and `Resource`s also each provide APIs that use the _type_ of the singleton state
stored in the context. In other words, they both offer access by type-lookup.

`Entity<T>` is the solution for storing zero-to-many instances of a kind of state. The
only requirement is `T: 'static`, consider it to be living in a hashmap where `Entity<T>`
is the key.

One of the key units of behavioral composition in GPUI happens when you have an `Entity<T>`
and know traits implemented by `T`, such as `Render`. When this is the case, the `Entity<T>`
handle can be used directly as a visual object.

Let's jump into some code to see how it works. In this codebase, I'm depending on my fork
of Zed where this is possible:

```rust
fn main() {
    Application::new()
        .add_plugins(zed::init)
        .add_plugins(tagged::init)
        .run();
}
```

This effectively allows me to build in a GPUI app that is
- 1) the entire Zed application (GPL3! <3), and
- 2) additionally whatever I stuff into the `App`.

The typical Zed "plugin" starts with `fn init(cx: &mut App) {}`. For most of my experiments,
I've wanted to be able to create custom UI, and the best starting points for me have been
to implement the Zed `Workspace`'s `Panel` and `Item` traits. Panels live in the side or bottom
docks, including views like `ProjectPanel`, `CollabPanel`, or `OutlinePanel`. Items fill the
main content window, like when you open a file to edit in Zed.

For this example, I'll show how to make a custom Panel, then how to integrate it into `Workspace`
using it's already-existing APIs, no modification to workspace source needed.

```rust
use gpui::*;

fn init(cx: &mut App) {
    // To instantiate a new Entity, we use `cx.new`.
    // The returned value is the Entity handle that we can use to look up or edit that state later.
    let chat_ui: Entity<ChatUi> = cx.new(|cx| ChatUi::new(cx));
    
    cx.observe_new({
        let willow_ui = willow_ui.clone();
        move |workspace: &mut Workspace, window, cx| {
            let Some(window) = window else {
                warn!("WillowUi: no Window in Workspace");
                return;
            };
            
            // Add panel to Workspace, then immediately toggle it open
            // Note: It appears that Workspace panels are limited to one per type
            workspace.add_panel(chat_ui.clone(), window, cx);
            workspace.toggle_panel_focus::<ChatUi>(window, cx);
        }
    });
}

struct ChatUi {
    focus_handle: FocusHandle,
}

impl ChatUi {
    pub fn new(cx: &mut Context<Self>) -> Self {
        //
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Item for ChatUi {
    // ...
}
impl Focusable for ChatUi {
    fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }
}
impl EventEmitter<PanelEvent> for ChatUi {}
impl Render for ChatUi {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        div()
            .child()
            .child()
    }
}

// There's some more to this example to make it work but this is a good illustration
```

So while Bevy and Zed have several distinctions and specializations to their domain,
their pattern of composition is (in an abstract way) very similar.

- They each embed their state in the "global context" (`App`)
- They each have a lifecycle and call the installed behavior on-demand
  - For Bevy this is a runloop, as a game engine, but
  - For Zed this is more like a Browser-style element tree with event
    propagation and callbacks.

There's a lot more to talk about with regard to the development experience from
within Zed, all good things. The design choices make it such a breeze to visually
design a representation of the inner state, while Entity handles allow for a cheap
and flexible way to pass state between contexts or to new components.

---

- Split Profile, Space, ButtonInput, ObjectWidget to separate modules
  - GPUI is so good at composition

- Chat as an instantiation of a more general feed, maybe `Feed<Chat>`
  - Feed could provide container-level common behavior, such as scrolling
    or animations.

---

- Portable private key, e.g. yubikey, allows logging in to any peer's
  "browser". The private key can generate the public key, and then perhaps
  there's a p2p layer that indexes nodes on the network according to their
  key and then allows lookup of any nodes hosting your key's content.

- So effectively if you leave your home computer on, and go to a friend's
  and use their laptop and the yubikey, you'd expect to find a path to your
  home node, or potentially any node that may be replicating data belonging to
  your key.

- Imagine your family's nodes have a "pull" permission to replicate a
  designated "backup" folder (or tag of folders) of yours, without you needing
  to give them any access to the contents.
  - Logging onto a new node via yubikey could sync the nearest copy of your
    namespaces to you.

---

- Need to do some solid coding, I have enough ideas to work with
- Contacts & Chat UI (peers/nodes UI?)
- Simple impl of Willow store API in-memory for testing? Would be nice
  for testing UX to actually be able to e.g. make profiles and entries.

- Thinking of using `.home/` as the convention for meta files, for a
  directory, or maybe `.here/`? so e.g. `.home/schemas/chat.schema.json`,
  or `.here/capabilities/chat.json`
  
  > slightly later me: I do quite like `.here/`

- I want to consider UI implications of read/write/pull

# 2026 Feb 17

Experimenting, notes

- Using a Bevy-style Query to represent a Willow query across the given
  namespaces, subspaces, and paths. The names are bikesheddable but the
  patterns are what's interesting.

```rust
fn init(cx: &mut App) {
    cx.willow().read(
        // Like gpui callbacks pass &mut App (or Context) to everything, do
        // we have a need to provide a "willow" context?
        // 
        // Oh wow, this could be really cool. So `Query<(...)>` describes the
        // search area for lookup in Willow, e.g. choosing one namespace and
        // one user subspace, and some path prefix. The `WillowCx<Datatype>`
        // 
        |
            query: Query<(Namespaces<(...)>, Subspaces<(...)>, PathPatterns<(...)>)>,
            willow_cx: &mut WillowCx<DataType>,
        | {
            // this can happen whenever Willow finishes IO for the query
            for entry in query.iter() {
                let ns = entry.namespace();
                let sub = entry.subspace();
                let data: &DataType = entry.data(willow_cx);
                let data: &mut DataType = entry.data_mut(willow_cx);
            }
        },
    );
    
    // Second attempt:
    // 
    // Bevy-inspired query API for interacting with Willow entries
    cx.willow().query(
        //
        |query: Query<
            // Data type portion of query
            Photo,
            // or, for example, to select multiple matching patterns:
            // Any<(Photo, String, Contact, Calendar)>
            // Automerge
            // Automerge<AutoSurgeonDerivedType>
            // IrohDoc<SerdeDatatype>
            
            // Search area portion of query
            (
                Namespaces<(/* list of type tokens for hardcoded namespaces? */)>,
                Subspaces<(/* list of type tokens for hardcoded subspaces / user keys */)>,
                Paths<(/* list of type-encoded paths or path patterns*/)>,
            )
            
            // It really seems to me that the only kind of subspace or namespace searching
            // /filtering capability you'd need would be "select from this list". 
            // But maybe with the ability to do any/all compositions on those lists
            // 
            // So e.g. the user looking at a search bar, could add a search fragment to
            // describe looking for entries which are located in any of a list of namespaces,
            // or subspaces, or path patterns
            // 
            // Need to think about whether that's actually useful, or whether it makes more
            // sense to have a runtime representation of queries, to accept dynamic lists of values
        >| {
            
        // Ooh, maybe the query callback can happen quickly by kicking off work in the
        // background, and just returning handles representing access to those objects?
        for photo: WillowObject<Photo> in query.iter() {
            //
        }
    });
    
    // Dynamic Area construction, rather than attempting via type composition
    let area = Area::new((
        Namespaces(vec![...]),
        Subspaces(vec![...]),
    ));
    // Even simpler
    let area = Area::builder()
        .with_namespaces(vec![..])
        .with_subspaces(vec![..])
        .build();
    cx.willow().query(area, |query: Query<Photo>| {
        //
    });
    
    // Fire a callback when any matching-typed object in the given area is created or modified
    cx.willow().observe(area, |query: Query<Photo>| {
        for photo: &Photo in query.iter() {
            //
        }
    });
}

#[derive(WillowObject)]
struct Photo {
    #[willow(path = "img.png")]
    data: Vec<u8>
}
```

---

Top-down time, UX brainstorming:

- Use-case: Chat
- Chat's unit visual object:
  - Profile name
  - Profile key
  - Profile icon (maybe default to key-derived generated?)
    - Oh! A functional, maybe styled QR code with the public key encoded?
    - Use as some kind of iroh ticket, passed by QR
    - Would want a toggle view, with default displayed not including any
      sensitive information, if anything.
  - Chat icon, one chat is probably one directory in a namespace, and whose
    chat-objects are written as entries in that directory. Possibly
  - Consider how to namespace schema definitions, would be useful to be have
    easy stable addresses to schemas 🔥
  - Feed: Needs to display some context like chat name, icon, status(es)?
    maybe custom styling
  - Feed visual objects: bubble-renders of chat data-objects, each bubble is a
    single message, maybe display sender or read status or whatever else
  - Send interface, text box and buttons. Consider multimedia now? Sigh
  - How to represent "others" (remote profiles? friends, contacts, connections)
    - Visually: face bubbles in chat?
    - As an object: `.receipts/{mary,jane}/{fields}`
  - I open a chat in a namespace. Where do I see known others in this chat?
    - Contact feed, next to the chat feed
    - "Space" interface is just a space full of feeds? Feed of profiles, namespaces,
      paths, tags.
  
- As an early user of this data space, what prefixing conventions would be good
  to establish? Want to be a good citizen and not make dumb choices people will
  need to live with forever.
- Right now I want to put app data in like `/apps/{app_name}/{here}`, because in
  Willow that path will also exist per-user and per-namespace, so in a way we
  already have a fairly globally unique prefix to our "root" directory.
- How easy or hard would it be to show the difference between:
  - This is a file in my namespace/profile-subspace that I'm editing, vs
  - This is a file in another namespace or profile-subspace that I happen to have
    the capability to use?

- Handle called Peer representing an online, syncing store of p2p protocols?
  - Allow instantiating multiple IO-unique peers in one process? Seems counter
    to the in-memory composition style going on, but still maybe
- "Peers" become another standard tag used for searching for feeds of objects.
  - Local peer: Shows only data that is persisted locally
  - All peers: Shows all the data we _know about_, e.g. we keep an index of files
    that may be in namespaces that we have read capabilities for. We could show a
    view where all local/saved/here objects are opaque and bold-colored, and remote
    ones are somewhat greyed-out or otherwise visually distinguished.
  - Tag queries could include:
    - All document between these three peers

- Namespace creation needs to have owned/communal from the start

- Bevy-inspired Asset interface?

- Per-profile "user styling" data object that's like a definition for how your
  profile should be rendered to other people. Like a specific purple glow rendered
  around all data objects associated with you. Custom shaders?

Data Spaces

- In-memory, in-application
- On-disk, over-wire
- REMOTE? FOREIGN-PEER OBJECT HANDLE?

What if it were possible to have an abstraction that treated in-memory and on-disk
objects with the same fundamental interface? `Entity<T>` handles are a delightfully
simple interface for interacting with app state. I'd like to make a similar handle
API for "on-disk" access (implemented as a Willow store), and if possible find a way
to compose the API of the two handles, ideally with the same handle type.

Down-the-line thought: physical storage, first-class replica management
- Replica: A remote peer that you have entered into a replication agreement with
- Probably implemented as some standard kind of capability (e.g. into a well-defined Area)
- Need user-facing terminology for remote interactions. And visual convention

- Workspace-item: Render a "space" as a sort of open canvas for feeds and object types.
  Could eventually extend to games, etc.
- API for plugins: Submit an object schema and path to the app, get rendered into the
  user's space as data objects integrated into existing feeds, or as newly-opened feeds
- Subject to standard query language, this becomes something users learn to apply to
  everything (tags, namespace/subspace or space/profile, paths)

- Need to figure out a first-class relationship between Willow and CRDT systems. I want
  first-class instant collaboration from CRDTS. I wonder if Willow would serve the purpose
  of namespacing and "addressing" a CRDT document, which could be represented as another
  custom handle type? When in an active session, the object is represented by a "CRDT handle"
  which might interact with it's "Context" which provides the machinery for sync and state
  managment and live collaboration.

- Want first-class consideration and integration of a reputation system, something that
  would maybe be useful for local organization purposes? Would be a target for abuse,
  might render it useless immediately

- Proxy render API as serializable DSL to host API-compatible interface inside WASM
  context, boom remote-portable apps

- Capability schemas??

- Make a UI dedicated to communicating how suspicious a requested capability is? For exmaple,
  assuming capabilities are first-class objects that users are familiar with handling, apps
  might request access to certain directories of the user's space.

- UI exercise: Need to practice distinguishing owned/communal namespaces. This matters
  very first thing in namespace (discord server) creation. Should probably heavily favor
  owned namespaces, I think I'd consider a communal namespace to be 

- Zed/GPUI: How hard is it to launch a program that's a handler for a file type? Just become
  a proper file explorer

---

- Thinking about how to display other users belonging to a mutual namespace
  - This is distinct from how to display multiple profiles controlled by one user
- If navigation is Profile > Namespace, in the namespace context it should be possible
  to view other (foreign, non-local) profiles which have participated in or have access
  to the namespace.
- Should capabilities be written to a standard directory in a namespace? Should there
  be a standard namespace index of participants?

- Need to think of a visual convention for Capabilities. I think they're easily just an
  instantiation of a "WillowObject" (as in earlier), so just a key-value object.
  Thinking rounded table window or bubble, with color-coded key/value cells and maybe a
  visually distinct rendering per data type, could be made generic and extendable.
- API: `trait ObjectLike` (name bikeshed), with `#[derive(ObjectLike)]`,

```rust
#[derive(ObjectLike)]
struct ProfileObject {}
```

- Network 

# 2026 Feb 16

- Calendar was a good thought experiment, but I think I need to pursue chat as
  a first motivating use-case to deliver.
- Navigation for Chat:
  - Open Profile
  - Open Namespace
  - optional: Open default chat for namespace?
  - Conveniently show directories with chat-compliant schemas?
  - Open a Path with a schema* matching the Chat objects
    > *I think it's fair to apply the term schema to a directory. Filesystems and objects are
        both key-value constructions. If a schema of an object can describe a mapping of
        unique field names to the types expected for the values, then a schema of a directory
        describes a mapping of unique paths to corresponding files and the expected format of
        those files.
    
  - Convention?: `.types/chat.schema.json`
    - `.types` as local directory convention that is specially recognized by Willow to
      describe/store the contents of a directory through the potential lens of many kinds
      of object. So different schemas in the same `.types` could describe a particular
      composition of fields into an object. A schema would act as a lens into the directory
      contents, so with careful design, directories/objects could be made to have strategic overlap

- UI design note: Standardized colors applied to object fields of primitive types
  - E.g. green for strings, blue for objects/enums

- UI design idea: Special "object" component that's just a little rectangle bubble table
  with a display of the object's keys and values. Could be made to have standard navigation
  through keys and lists, make objects predictable and understandable to users.
  - In an app (as a gpui plugin), define data types that have Render implementations, and
    associate those objects with a Willow object schema/handle, so the UI becomes a tangible
    and predictable interface to data objects.
  - Made to be a new-window component?

- An "object" as a directory in Willow may be uniquely identified by a
  (namespace, subspace, prefix) tuple
  - Object schema could either be explicitly given by the tuple, or implicitly identified
    by convention encoded in files, e.g. `.types/calendar.schema.json`
  - (namespace, subspace, prefix, schema) as a conventional unit may eliminate the need
    for special-casing schemas into the data model

---

Willow API: abstraction to generalize over "handles" to data. In the GPUI case, the handle
to a T is an `Entity<T>`, other handle kinds could include keys to maps.

```rust
trait ObjectHandle {
    type Context<T>;
    fn read(&self, cx: &mut Self::Context<T>) -> &T;
}

impl<T> ObjectHandle for Entity<T> {
    type Context = gpui::Context<T>;
    fn read(&self, cx: &mut gpui::Context<T>) -> &T {}
}

impl<T> ObjectHandle for WillowObject<T> {
    type Context = WillowContext<T>;
    fn read(&self, cx: &mut WillowContext<T>) {}
}
```

Here, `WillowContext<T>` represents an "open directory" which has maybe done schema
validation to ensure the directory fits the shape of the given `T` data object.

---

Random thought: The fully generalized App is the holy grail of abstraction. The
key problem is how to make an app without any intrinsics, and allow plugins to
somehow submit first-class app state (e.g. the entity system should itself be a
plugin). Fully generalized I suppose would mean that the App starts completely
empty, with no state and no API. Plugins would somehow provide all building
blocks to the global context, including inherent state like direct fields. The
way I'm imagining this is something like:

```rust
struct App<T> {
    intrinsics: T,
}

// here, universal T works with any instantiation of App
fn do_something<T>(cx: &mut App<T>) {}

// here, we require the app given has a particular necessary intrinsic, provided by a plugin
fn do_something_needing_entity_system(cx: &mut App<T>)
    where App<T>: EntitySystem
{
    cx.new(|cx| ...);
}

fn main() {
    // instantiate an App with intrinsics using a tuple
    App::new((
        EntitySystem::new(),
        WillowSystem::new(),
    ))
}

// I _think_ this API would require some kind of extractor pattern, which would
// allow the App to "pick" a given system out of the tuple of systems. I think
// this could be done but I haven't succeeded with it yet
```

---

Bigger picture: Roadmap and design

- Need plugins/apps for use-cases:
  - Chat
  - (maybe email UX for long-form)
  - Calendar
  - Documents,
  - Filesystem explorer,
  - Profile/Space management,
  - Peer network,
  - Storage,
  - integrate with Zed Settings
- UI considerations
  - Make objects first-class, make it clear that app's components are just
    direct renderings of data

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
data model API. Ideally these could be the exact same definition, e.g. a struct, which both exists as in-memory
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
