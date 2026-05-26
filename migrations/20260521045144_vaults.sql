CREATE TABLE vaults (
    vault_id TEXT PRIMARY KEY NOT NULL,
    metadata BLOB NOT NULL,
    encrypted_vault BLOB NOT NULL,
    encrypted_vault_encryption_key BLOB NOT NULL,
    vault_encryption_key_salt BLOB NOT NULL
)
