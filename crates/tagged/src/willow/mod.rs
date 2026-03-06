use std::{collections::HashMap, marker::PhantomData, path::PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use willow25::{
    entry::{
        randomly_generate_communal_namespace, randomly_generate_owned_namespace,
        randomly_generate_subspace,
    },
    storage::MemoryStore,
};
use zed::unstable::{
    gpui::{AppContext, Entity, Global},
    ui::{App, Context, SharedString},
};

use crate::state::{profile::Profile, space::Space};

pub fn init(cx: &mut App) {
    let store_path = zed::unstable::paths::data_dir();
    let willow = Willow::new(store_path, cx);
    cx.set_global(GlobalWillow(willow));

    // Insert dummy data to store
    cx.willow();
}

impl Global for GlobalWillow {}
struct GlobalWillow(Willow);

/// Willow API entrypoint
///
/// Willow "store" level operations
#[derive(Clone)]
pub struct Willow {
    /// Local state per Willow instance
    state: Entity<WillowState>,
}

/// State of a Willow instance. Probably 1:1 with a "store" on disk at a given path
struct WillowState {
    // TODO: Generalization of this, esp with Willow Ext traits
    profiles: Vec<Entity<Profile>>,
    spaces: Vec<Entity<Space>>,

    store_path: PathBuf,
    /// Payloads in simple impl are just bytes
    paths: HashMap<String, Vec<u8>>,

    store: MemoryStore,
}

impl Willow {
    fn new(store_path: impl Into<PathBuf>, cx: &mut App) -> Self {
        let state = cx.new(|cx| WillowState::new(store_path.into(), cx));

        Self { state }
    }

    pub fn create_profile(
        //
        &self,
        name: impl Into<SharedString>,
        cx: &mut App,
    ) -> Entity<Profile> {
        let (_subspace_id, sub_secret) = randomly_generate_subspace(&mut rand_core_0_6_4::OsRng);
        let profile = cx.new(move |cx| Profile::new(name, sub_secret, cx));

        self.state.update(cx, |state, _cx| {
            state.profiles.push(profile.clone());
        });

        profile
    }

    pub fn create_owned_space(&self, name: impl Into<SharedString>, cx: &mut App) -> Entity<Space> {
        let (_namespace_id, ns_secret) =
            randomly_generate_owned_namespace(&mut rand_core_0_6_4::OsRng);
        let space = cx.new(move |cx| Space::new(name, ns_secret, cx));

        self.state.update(cx, |state, _cx| {
            state.spaces.push(space.clone());
        });

        space
    }

    pub fn create_communal_space(
        &self,
        name: impl Into<SharedString>,
        cx: &mut App,
    ) -> Entity<Space> {
        let (_namespace_id, ns_secret) =
            randomly_generate_communal_namespace(&mut rand_core_0_6_4::OsRng);
        let space = cx.new(move |cx| Space::new(name, ns_secret, cx));

        self.state.update(cx, |state, _cx| {
            state.spaces.push(space.clone());
        });

        space
    }

    pub fn profiles(&self, cx: &mut App) -> Vec<Entity<Profile>> {
        self.state.read(cx).profiles.clone()
    }

    pub fn spaces(&self, cx: &mut App) -> Vec<Entity<Space>> {
        self.state.read(cx).spaces.clone()
    }
}

impl WillowState {
    fn new(store_path: PathBuf, _cx: &mut Context<Self>) -> Self {
        let spaces = vec![
            // cx.new(|cx| Space::new("Home".to_string(), cx)),
            // cx.new(|cx| Space::new("Family".to_string(), cx)),
        ];

        let profiles = vec![
            // cx.new(|cx| Profile::new("Myselfandi", cx)),
            // cx.new(|cx| Profile::new("Alterego", cx)),
        ];

        let store = MemoryStore::new();

        Self {
            spaces,
            store_path,
            paths: Default::default(),
            profiles,
            store,
        }
    }

    fn it(&mut self) -> anyhow::Result<()> {
        // Note: Consider to be low-level interface.
        // Want to create a GPUI simple DSL on top
        // self.store
        //     .create_entry(data, payload_producer, payload_length, ingredients)

        // let mut csprng = rand_core::OsRng;
        // let (subspace_id, secret) = randomly_generate_subspace(&mut csprng);
        // let namespace_id = NamespaceId::from_bytes(&[17; 32]);

        // let entry = Entry::builder()
        //     .namespace_id(namespace_id.clone())
        //     .subspace_id(subspace_id.clone())
        //     .path(path!("/ideas"))
        //     .timestamp(12345)
        //     .payload(b"chocolate with mustard")
        //     .build()
        //     .unwrap();

        // let cap = WriteCapability::new_communal(namespace_id.clone(), subspace_id.clone());
        // let authed = entry.authorise(&cap, &secret)?;

        Ok(())
    }
}

pub trait WillowExt {
    fn willow(&mut self) -> Willow;
}

impl WillowExt for App {
    fn willow(&mut self) -> Willow {
        self.global::<GlobalWillow>().0.clone()
    }
}

pub struct WillowObject<T> {
    _phantom: PhantomData<T>,
}

pub struct WillowFeed<T> {
    _phantom: PhantomData<T>,
}

/// A Willow Entity is a handle representing an object with a well-known type
///
/// To be a somewhat complete and well-addressed handle, a WillowEntity includes
/// information about the namespace and subspace of the underlying Entry.
///
/// So an Entity is like an address/handle for an Area, so it's defined by its
/// namespace, subspace, and path prefix (directory). The definition of a Willow
/// Area also includes a time range, I want to think about how to represent time
/// in a dedicated brainstorm.
///
/// - Area in the spec has `subspace_id: SubspaceId | any`, which implies an
///   arbitrary restriction in the expressiveness of the API. I think it should
///   easily be possible to specify a list of subspaces we're interested in.
struct WillowEntity<T: WillowModel> {
    _phantom: PhantomData<T>,
}

struct WillowContext<T> {
    _phantom: PhantomData<T>,
}

impl<T: WillowModel> WillowEntity<T> {
    fn read(&self, _cx: &mut WillowContext<T>) -> Option<&T> {
        None
    }
}

// WillowComponent?
// WillowSpec
// WillowArea
// WillowModel <-- expresses paths to multiple files, typed extractors
// - Model would refer to a multi-"file" data construction which is located
//   at a path and described by the set of files the model refers to, as well
//   as the types of those files.
pub trait WillowModel: JsonSchema + Serialize + for<'de> Deserialize<'de> {}
