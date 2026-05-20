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
    #[error("vault is locked")]
    VaultLocked(VaultId),
    #[error("failed to validate vault capability")]
    InvalidCapability(#[from] capsec::CapSecError),
    #[error(transparent)]
    Other(anyhow::Error),
}
