use plugin_vault::vault_db::VaultId;

pub struct Subspace {
    /// Rather than store the subspace key directly in memory, hold a
    /// [`VaultId`] which may be used to retrieve the key from the vault.
    vault_id: VaultId,
}
