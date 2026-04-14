use std::sync::Arc;

use zed::unstable::{
    gpui::{App, Entity, Global, IntoElement, ParentElement as _, SharedString, Styled as _, div},
    workspace::ui::{FluentBuilder, ListItem},
};

pub fn init(cx: &mut App) {
    // Everybody who imports this module gets this API
    let willow = cx.willow();

    // ===== API iteration 1 =====

    let ns_key: NamespaceKey = willow.by_name("your namespace alias");
    let user: User = willow.login();

    // API representing access to a user logged-in to a specific namespace
    let store: WillowStore = {
        // - API indicating
        // - Capability checking is the next level down
        // - Read/write only to indicate mutability of access.
        // - Should we worry about a store's read/write access here?

        willow.read(ns_key, user); // or
        willow.write(ns_key, user); // or
        willow.store(ns_key, user)
    };

    // ===== API iteration 2 =====

    // Mental model: Step 1) I sit down and log in. I now have access to the things I can do as a user.
    // I can log in or "unlock" one or more user profiles at a time
    // UX: Maybe allow some profiles/identities to specify they may not be opened concurrently,
    // to avoid making it easy to leak sensitive data from one identity to another
    //
    // Imagination: This is like a digital soul, the elemental interface for doing anything in this digital world
    // But more like if it was a keychain of digital souls, so it's convenient to have more than one
    let user: User = willow.login();

    // Mental model: Step 2) I open a namespace, which is like a Discord server that my user belongs to
    let namespaces: Vec<WillowNamespace> = user.namespaces(); // List joined namespaces. Like Discord servers

    // Perhaps opened when in UI the user clicks the "Discord server" button
    let namespace: WillowNamespace = user.namespace("by name or id"); // Open a Discord server

    // So we've made user login orthogonal to namespace access.
    // We don't have to consider a hierarchy of `/namespace/subspace/`
    // We can keep the login state of multiple users in the `willow` API.
    // Logged in users should keep metadata about their joined namespaces.
    // Consider `/namespace/subspace/`
    // Consider the same but as `/user_key/user_key`
    // Convention: The namespace with the same name as a user's key should
    // be an exclusively private namespace of the user's own.
    // Should user keys always be generated so they belong to the set of owned namespaces?

    // ===== API iteration 3 =====
    // Leaning into the "digital soul" idea, just for fun

    // Does one log into a soul? need better verbiage
    // let me: Soul = willow.login();

    // lol, but manifest might feel like Cargo.toml so maybe not
    let me: Soul = willow.manifest();

    // as if logging out were going to sleep, awakening would be logging in
    let me: Soul = willow.awaken();

    // to fill a need like a keychain for more than one,
    // but being whimsical
    // perform login for all users on keychain
    // perform awakening ritual for all inhabitant souls
    // Handle held by UI to perform actions on the user's behalf, API to all user/multi-user actions
    let ring: SoulRing = willow.awakening();
    // API for listing profiles/users/souls that are "public" or "not hidden"
    // Use-case: rendering the list of logged in or logged-out-but-present profiles
    // We are my school soul, my family soul, my social souls, and my work soul
    // Should not leak associations between profiles
    let we = ring.souls(); // -> impl Iterator<Item=(&Name, &Soul)>
    // let us = ring.souls_mut(); // lol
    // let us = ring.into_souls(); // lolol

    // ===== API iteration 4 ======
    // less whimsy, for the sake of sanity

    // User is only a HANDLE to a user's data. Might correspond to a GPUI entity
    // here the handle would start in unlocked state but could transition to locked
    let user: User = willow.login();
    let users = willow.users(); // -> impl Iterator<Item=User>

    fn sample_render_users(cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .children(
                cx.willow()
                    .users()
                    .into_iter()
                    .enumerate()
                    .map(|(i, user)| {
                        // One user per list item (hoverable)
                        ListItem::new(SharedString::from(format!("user-{i}"))).child(
                            div()
                                .p_2()
                                .flex()
                                .flex_col()
                                .child(
                                    // Name and locked status
                                    div()
                                        .flex()
                                        .flex_row()
                                        .child(
                                            div()
                                                .flex_grow()
                                                .child(format!("User: {}", user.name())),
                                        )
                                        .when(user.is_unlocked(), |div| div.child("🟢"))
                                        .when(user.is_locked(), |div| div.child("🔴")),
                                )
                                // Public key below name
                                .child(format!("Public key: {}", user.id()))
                                // Namespaces of user shown like a subdirectory under the user
                                .children(
                                    cx.willow()
                                        .namespaces_of(&user)
                                        .into_iter()
                                        // .spaces_of(user)
                                        .enumerate()
                                        .map(|(i, space)| {
                                            ListItem::new(SharedString::from(format!(
                                                "space-{}-{}",
                                                user.id(),
                                                i
                                            )))
                                            //
                                            .child(space.name())
                                        }),
                                ),
                        )
                    }),
            )
    }
}

// Login API. There are many potential API options here
fn user_key_api_exploration(willow: Willow) -> UserKey {
    // // 1) Use a known Willow app data directory to store profiles
    // //    in a known state, including a default identity.
    // //
    // // I suppose this would need to prompt for password interface or similar
    // let user_key: UserKey = willow.login();

    // // 2) Login to a default user identity at a custom data directory
    // let user_key: UserKey = willow.login_from("<custom data directory>");

    // // 3) Use "profile" terminology to invoke the feeling of a user's own identity
    // let user_key: UserKey = willow.unlock_profile();

    // // 4) Quick and easy, like opening a car or a wallet
    // let user_key: UserKey = willow.unlock();

    // {
    //     trait IntoKey {}
    //     let key = "<string literal, multiformat?>";
    //     let key = fs::file::read("path/to/key")?;
    //     let key = PathBuf::from("path/to/key");
    //     let user_key: UserKey = willow.with_key(key /* impl IntoKey */);
    // }

    todo!()
}

impl Global for GlobalWillow {}
struct GlobalWillow(Willow);

/// Willow API entrypoint
///
/// Willow "store" level operations
#[derive(Clone)]
pub struct Willow {
    state: Arc<WillowState>,
}
struct WillowState {}
#[derive(Copy, Clone)]
struct NamespaceKey {}
#[derive(Clone)]
struct User {
    entity: Entity<UserState>,
}
struct UserState {}
impl User {
    fn id(&self) -> impl std::fmt::Display {
        todo!()
    }
    fn name(&self) -> &str {
        todo!()
    }

    fn is_unlocked(&self) -> bool {
        todo!()
    }

    fn is_locked(&self) -> bool {
        todo!()
    }

    fn namespaces(&self) -> Vec<WillowNamespace> {
        todo!()
    }

    fn namespace(&self, arg: &str) -> WillowNamespace {
        todo!()
    }
}
struct UserKey {}
impl UserKey {
    fn namespaces(&self) -> Vec<WillowNamespace> {
        todo!()
    }

    fn namespace(&self, arg: &str) -> WillowNamespace {
        todo!()
    }
}

struct Soul {}
struct SoulRing {}
impl SoulRing {
    fn souls(&self) -> impl IntoIterator<Item = Soul> {
        []
    }
}

/// API entrypoint for all operations at a known namespace and user (subspace root)
struct Space {}
impl Space {
    fn name(&self) -> String {
        todo!()
    }
}

/// API entrypoint for namespace-level operations
struct WillowNamespace {
    //
}

impl WillowNamespace {
    fn new(// TODO login or sign or something
    ) -> Self {
        Self {}
    }
}

impl Willow {
    // Public API for Willow available here
    pub fn namespace(&self, ns: NamespaceKey) -> WillowNamespace {
        //
        WillowNamespace::new()
    }

    fn login(&self) -> User {
        todo!()
    }

    fn login_from(&self, arg: &str) -> User {
        todo!()
    }

    fn by_name(&self, arg: &str) -> NamespaceKey {
        todo!()
    }

    fn read(&self, ns_key: NamespaceKey, user_key: User) -> Space {
        todo!()
    }

    fn write(&self, ns_key: NamespaceKey, user_key: User) -> Space {
        todo!()
    }

    fn manifest(&self) -> Soul {
        todo!()
    }

    fn awaken(&self) -> Soul {
        todo!()
    }

    fn awakening(&self) -> SoulRing {
        todo!()
    }

    fn users(&self) -> impl IntoIterator<Item = User> {
        []
    }

    fn namespaces_of(&self, user: &User) -> impl IntoIterator<Item = Space> {
        []
    }

    fn store(&self, ns_key: NamespaceKey, user: User) -> WillowStore {
        todo!()
    }
}

/// API for a user successfully logged-in to a namespace and subspace (user key)
// Could be WillowStore, WillowProfile, etc. Needs to imply a logged-in user
struct WillowStore {
    //
}

trait WillowExt {
    fn willow(&mut self) -> Willow;
}

impl WillowExt for App {
    fn willow(&mut self) -> Willow {
        self.global::<GlobalWillow>().0.clone()
    }
}
