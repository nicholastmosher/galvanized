use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use zed::unstable::{db::kvp::KEY_VALUE_STORE, gpui::AppContext};

pub trait VaultExt {
    type Context: AppContext;
    fn vault(&mut self) -> impl SecretRepository;
}
pub struct VaultCx<'a, C: AppContext> {
    //
    cx: &'a mut C,
}

impl<C: AppContext> VaultExt for C {
    type Context = C;
    fn vault(&mut self) -> impl SecretRepository {
        InsecureSecretRepository::new()
    }
}

/// Interface for a secure key-value store
///
/// > I'm skipping properly implementing secure storage for now, I'm in a
/// > sketching phase and need to get the shape of all the pieces together,
/// > then I'll circle back and create real implementations for things like this.
pub trait SecretRepository {
    //
    fn read(&self, key: &str) -> impl Future<Output = Result<Option<String>>> + Send + Sync;
    fn write(&mut self, key: String, value: String) -> impl Future<Output = Result<()>>;
}

/// DO NOT USE IN PRODUCTION, STORES SECRETS IN PLAINTEXT
#[derive(Debug, Default, Serialize, Deserialize)]
struct InsecureSecrets {
    entries: HashMap<String, String>,
}

const INSECURE_KV_KEY: &str = "insecure-secrets";

#[non_exhaustive]
pub struct InsecureSecretRepository {
    //
}

impl InsecureSecretRepository {
    pub fn new() -> Self {
        Self {}
    }
}

impl SecretRepository for InsecureSecretRepository {
    async fn read(&self, key: &str) -> Result<Option<String>> {
        let secrets_text = KEY_VALUE_STORE.read_kvp(INSECURE_KV_KEY)?;
        let secrets = secrets_text
            .map(|it| serde_json::from_slice::<InsecureSecrets>(it.as_bytes()))
            .transpose()?;
        let entry = secrets.map(|it| it.entries.get(key).cloned()).flatten();
        Ok(entry)
    }

    async fn write(&mut self, key: String, value: String) -> Result<()> {
        let secrets_text = KEY_VALUE_STORE.read_kvp(INSECURE_KV_KEY)?;
        let mut secrets = secrets_text
            .map(|it| serde_json::from_slice::<InsecureSecrets>(it.as_bytes()))
            .transpose()?
            .unwrap_or_default();
        secrets.entries.insert(key, value);
        let secrets = serde_json::to_string(&secrets)?;
        KEY_VALUE_STORE
            .write_kvp(INSECURE_KV_KEY.to_string(), secrets)
            .await?;
        Ok(())
    }
}
