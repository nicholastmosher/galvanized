use thiserror::Error;

use crate::{encryption::CryptError, vault_data::VaultId};

/// Errors that can occur when interacting with a [`Vault`].
#[derive(Debug, Error)]
pub enum VaultError {
    #[error("failed to create vault: duplicate vault ID")]
    DuplicateVaultId(VaultId),
    #[error("failed vault encryption or decryption")]
    CryptoError(#[from] CryptError),
    #[error("failed to find vault with ID {0}")]
    MissingVault(VaultId),
    #[error(transparent)]
    Other(anyhow::Error),
}
