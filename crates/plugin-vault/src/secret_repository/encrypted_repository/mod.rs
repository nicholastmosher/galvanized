use std::{collections::BTreeSet, path::PathBuf};

use anyhow::{Context, Result};
use base64::Engine as _;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value as JsonValue;
use zed::unstable::db::kvp::KEY_VALUE_STORE;

use crate::secret_repository::SecretRepository;

pub mod encryption;

const SECRETS_PREFIX: &str = "gzed/secrets";
const SECRETS_INDEX: &str = "gzed/secrets/index";

/// Secret repository that stores encrypted secrets in the Zed key value store
pub struct EncryptedRepository {
    //
}

impl EncryptedRepository {
    pub fn new() -> Self {
        Self {}
    }
}

impl SecretRepository for EncryptedRepository {
    fn list(&self) -> impl Future<Output = Result<Vec<String>>> {
        async move {
            let index_string = KEY_VALUE_STORE
                .read_kvp(SECRETS_INDEX)
                .context("failed to read secrets index")?;

            let index = index_string
                .map(|s| serde_json::from_str::<SecretsIndex>(&s))
                .transpose()
                .context("failed to deserialize secrets index")?
                .unwrap_or_default();

            let keys = index.keys.iter().cloned().collect::<Vec<_>>();
            Ok(keys)
        }
    }

    fn read(&self, key: String) -> impl Future<Output = Result<Option<Vec<u8>>>> {
        async move {
            let key = format!("{SECRETS_PREFIX}/{key}");
            let value_base64 = KEY_VALUE_STORE.read_kvp(&key)?;
            let value = value_base64
                .map(|s| base64::engine::general_purpose::STANDARD.decode(&s))
                .transpose()
                .context("failed to base64 decode secret")?;
            Ok(value)
        }
    }

    fn write(&mut self, key: String, value: Vec<u8>) -> impl Future<Output = Result<()>> {
        let key = format!("{SECRETS_PREFIX}/{key}");

        async move {
            let index_string = KEY_VALUE_STORE
                .read_kvp(SECRETS_INDEX)
                .context("failed to read secrets index")?;

            let mut index = index_string
                .map(|s| serde_json::from_str::<SecretsIndex>(&s))
                .transpose()
                .context("failed to deserialize secrets index")?
                .unwrap_or_default();
            index.keys.insert(key.clone());
            let index_string =
                serde_json::to_string(&index).context("failed to serialize secrets index")?;

            let value_string = base64::engine::general_purpose::STANDARD.encode(value);
            KEY_VALUE_STORE
                .write_kvp(key, value_string)
                .await
                .context("failed to update secret")?;
            KEY_VALUE_STORE
                .write_kvp(SECRETS_INDEX.to_string(), index_string)
                .await
                .context("failed to update secrets index")?;

            Ok(())
        }
    }
}

/// Metadata about secrets kept in the encrypted repository
///
/// For now this is just a set of keys stored so we can list entries.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SecretsIndex {
    keys: BTreeSet<String>,
}
