use std::path::PathBuf;

use thiserror::Error;

use crate::{encryption::CryptError, vault_db::VaultId};

/// Domain-level errors that can occur when interacting with the vault system.
#[derive(Debug, Error)]
pub enum VaultError {
    #[error("failed to open vault database")]
    OpenVault(#[from] OpenVaultError),

    #[error("failed to create new vault")]
    CreateVault(#[from] CreateVaultError),

    #[error("failed to list vaults")]
    ListVaults(#[from] ListVaultsError),

    #[error("failed to lock vault")]
    LockVault(#[from] LockVaultError),

    #[error("failed to read vault")]
    ReadVault(#[from] ReadVaultError),

    #[error("failed to read vault metadata")]
    ReadVaultMetadata(#[from] ReadVaultMetadataError),

    #[error("failed to rotate encryption key")]
    RotateKey(#[from] RotateKeyError),

    #[error("failed to unlock vault")]
    UnlockVault(#[from] UnlockError),

    #[error("failed to update vault")]
    UpdateVault(#[from] UpdateVaultError),
}

#[derive(Debug, Error)]
pub enum CreateVaultError {
    #[error("failed initial encryption")]
    Crypto(#[from] CryptError),

    #[error("failed initial database setup")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Error)]
pub enum FlushVaultError {
    #[error("failed database op while flushing vault '{0}'")]
    Database(VaultId, #[source] sqlx::Error),

    #[error("failed to flush vault to disk, no vault with id '{0}'")]
    MissingVault(VaultId),
}

#[derive(Debug, Error)]
pub enum ListVaultsError {
    #[error("failed database op while listing vaults")]
    Database(#[from] sqlx::Error),

    #[error("invalid vault id: '{0}'")]
    InvalidVaultId(String, #[source] anyhow::Error),
}

#[derive(Debug, Error)]
pub enum LoadVaultError {
    #[error("failed to read vault from database")]
    Database(VaultId, #[source] sqlx::Error),

    #[error("failed to deserialize vault")]
    Serde(VaultId, #[source] serde_json::Error),

    #[error("failed to load vault row into memory for vault '{0}'")]
    ImpedenceMismatch(VaultId, #[source] anyhow::Error),
}

#[derive(Debug, Error)]
pub enum LockVaultError {
    #[error("failed to lock vault, no vault with id '{0}'")]
    MissingVault(VaultId),
}

#[derive(Debug, Error)]
pub enum OpenVaultError {
    #[error("failed to connect to database at '{0}'")]
    ConnectDatabase(PathBuf, #[source] sqlx::Error),

    #[error("failed to parse given database path '{0}'")]
    ParseDatabasePath(PathBuf, #[source] sqlx::Error),

    #[error("io permission error while opening vault file '{0}'")]
    IoPermission(PathBuf, #[source] std::io::Error),

    #[error("io error while writing initial vault database at path '{0}'")]
    IoWriteInitialDb(PathBuf, #[source] std::io::Error),
}

#[derive(Debug, Error)]
pub enum ReadVaultError {
    #[error("failed crypto op while reading vault '{0}'")]
    Crypto(VaultId, #[source] CryptError),

    #[error("failed to provide capability to read vault '{0}'")]
    Capability(VaultId, #[source] capsec::CapSecError),

    #[error("failed to load vault '{0}' from database")]
    Load(VaultId, #[source] LoadVaultError),

    #[error("failed to read vault '{0}', vault is locked")]
    Locked(VaultId),

    #[error(
        "failed to read decrypted symmetric key for vault '{0}'\n\
        expected 32 bytes, got {1} bytes"
    )]
    MalformedKey(VaultId, usize),

    #[error("failed to deserialize vault content while reading vault '{0}'")]
    Serde(VaultId, #[source] serde_json::Error),
}

#[derive(Debug, Error)]
pub enum ReadVaultMetadataError {
    #[error("failed crypto op while reading vault '{0}'")]
    Crypto(VaultId, #[source] CryptError),

    #[error("failed to provide capability to read vault '{0}'")]
    Capability(VaultId, #[source] capsec::CapSecError),

    #[error("failed to load vault '{0}' from database")]
    Load(VaultId, #[source] LoadVaultError),

    #[error("failed to read, vault '{0}' is missing or not loaded")]
    Missing(VaultId),

    #[error(
        "failed to read decrypted symmetric key for vault '{0}'\n\
        expected 32 bytes, got {1} bytes"
    )]
    MalformedKey(VaultId, usize),

    #[error("failed to deserialize vault content while reading vault '{0}'")]
    Serde(VaultId, #[source] serde_json::Error),
}

#[derive(Debug, Error)]
pub enum RotateKeyError {
    #[error("failed crypto op while rotating key for vault '{0}'")]
    Crypto(VaultId, #[source] CryptError),

    #[error("failed database op while rotating key for vault '{0}'")]
    Database(VaultId, #[source] sqlx::Error),

    #[error("failed to load vault from database for vault '{0}'")]
    LoadVault(VaultId, #[source] LoadVaultError),

    #[error("failed to find vault '{0}'")]
    MissingVault(VaultId),
}

#[derive(Debug, Error)]
pub enum UnlockError {
    #[error("failed crypto operation for vault '{0}'")]
    Crypto(VaultId, #[source] CryptError),

    #[error("failed to load vault for vault '{0}'")]
    LoadVault(VaultId, #[source] LoadVaultError),

    #[error("failed to find vault '{0}'")]
    MissingVault(VaultId),
}

#[derive(Debug, Error)]
pub enum UpdateVaultError {
    #[error("failed to provide capability to update vault '{0}'")]
    Capability(VaultId, #[source] capsec::CapSecError),

    #[error("failed to flush vault after update")]
    FlushVault(#[source] FlushVaultError),

    #[error("failed to load vault for update")]
    LoadVault(#[source] LoadVaultError),

    #[error("vault '{0}' is locked, no unlock session present")]
    Locked(VaultId),

    #[error("vault '{0}' is missing")]
    MissingVault(VaultId),
}
