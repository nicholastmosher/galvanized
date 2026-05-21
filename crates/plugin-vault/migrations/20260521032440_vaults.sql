CREATE TABLE vaults (
    id TEXT PRIMARY KEY NOT NULL,
    encrypted_vault BLOB NOT NULL,
    encrypted_vault_encryption_key BLOB NOT NULL,
    vault_encryption_key_salt TEXT NOT NULL,
)
