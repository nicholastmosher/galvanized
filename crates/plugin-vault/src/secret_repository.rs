use std::{collections::BTreeMap, pin::Pin};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use zed::unstable::db::kvp::KEY_VALUE_STORE;

/// Interface for a secure key-value store
///
/// > I'm skipping properly implementing secure storage for now, I'm in a
/// > sketching phase and need to get the shape of all the pieces together,
/// > then I'll circle back and create real implementations for things like this.
pub trait SecretRepository: 'static {
    fn list(&self) -> impl Future<Output = Result<Vec<(String, String)>>>;
    fn read(&self, key: String) -> impl Future<Output = Result<Option<String>>>;
    fn write(&mut self, key: String, value: String) -> impl Future<Output = Result<()>>;
}

pub trait DynSecretRepository: 'static {
    fn list(&self) -> Pin<Box<dyn Future<Output = Result<Vec<(String, String)>>> + '_>>;
    fn read(&self, key: String) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + '_>>;
    fn write(
        &mut self,
        key: String,
        value: String,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + '_>>;
}

impl<T: SecretRepository> DynSecretRepository for T {
    fn list(&self) -> Pin<Box<dyn Future<Output = Result<Vec<(String, String)>>> + '_>> {
        Box::pin(self.list())
    }

    fn read(&self, key: String) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + '_>> {
        Box::pin(self.read(key))
    }

    fn write(
        &mut self,
        key: String,
        value: String,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + '_>> {
        Box::pin(self.write(key, value))
    }
}

/// DO NOT USE IN PRODUCTION, STORES SECRETS IN PLAINTEXT
#[derive(Debug, Default, Serialize, Deserialize)]
struct InsecureSecrets {
    entries: BTreeMap<String, String>,
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
    async fn list(&self) -> Result<Vec<(String, String)>> {
        let secrets_text = KEY_VALUE_STORE.read_kvp(INSECURE_KV_KEY)?;
        let secrets = secrets_text
            .map(|it| serde_json::from_slice::<InsecureSecrets>(it.as_bytes()))
            .transpose()?
            .unwrap_or_default();
        let entries = secrets
            .entries
            .iter()
            .map(|it| (it.0.clone(), it.1.clone()))
            .collect::<Vec<_>>();
        Ok(entries)
    }

    async fn read(&self, key: String) -> Result<Option<String>> {
        let secrets_text = KEY_VALUE_STORE.read_kvp(INSECURE_KV_KEY)?;
        let secrets = secrets_text
            .map(|it| serde_json::from_slice::<InsecureSecrets>(it.as_bytes()))
            .transpose()?;
        let entry = secrets.map(|it| it.entries.get(&key).cloned()).flatten();
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
