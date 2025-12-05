use samod::{PeerId, storage::TokioFilesystemStorage};
use zed::unstable::gpui::{App, AppContext, AsyncApp, Entity, Global};

use crate::iroh_repo::IrohRepo;

mod codec;
mod iroh_repo;

pub struct GlobalIrohRepo(Option<Entity<IrohRepo>>);
impl Global for GlobalIrohRepo {}

pub fn init(cx: &mut App) {
    let base_path = "/tmp/iroh-automerge";
    cx.set_global(GlobalIrohRepo(None));
    cx.spawn(async move |cx: &mut AsyncApp| {
        let secret_key = iroh::SecretKey::generate(&mut rand::rng());
        let endpoint = iroh::Endpoint::builder()
            .secret_key(secret_key)
            .bind()
            .await?;
        let repo = samod::Repo::build_tokio()
            .with_peer_id(PeerId::from_string(endpoint.id().to_string()))
            .with_storage(TokioFilesystemStorage::new(format!(
                "{}/{}",
                base_path,
                endpoint.id(),
            )))
            .load()
            .await;
        let iroh_repo = IrohRepo::new(endpoint.clone(), repo);
        let _router = iroh::protocol::Router::builder(endpoint)
            .accept(IrohRepo::SYNC_ALPN, iroh_repo.clone())
            .spawn();

        let repo_entity = cx.new(move |_cx| iroh_repo).ok();
        cx.update_global(|&mut GlobalIrohRepo(ref mut repo), _| *repo = repo_entity)?;
        anyhow::Ok(())
    })
    .detach();
}
