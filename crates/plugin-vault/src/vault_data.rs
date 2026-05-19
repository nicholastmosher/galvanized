use crate::secret_repository::encrypted_repository::encryption::{
    CryptError, decrypt, encrypt, generate_salt, hash_password,
};
use crate::vault_cap::{VaultAccess, VaultRevoker, VaultSendCap};
use anyhow::{Context as _, Result};
use capsec::CapSecError;
use capsec::Scope;
use derive_more::Display;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use serde_json::json;
use thiserror::Error;
use uuid::Uuid;

/// Errors that can occur when interacting with a [`Vault`].
#[derive(Debug, Error)]
pub enum VaultError {
    #[error("failed to create vault: duplicate vault ID")]
    DuplicateVaultId(VaultId),
    #[error("failed vault encryption or decryption")]
    CryptoError(#[from] CryptError),
    #[error(transparent)]
    Other(anyhow::Error),
}

/// A wrapper around a [`Vault`] and its corresponding [`VaultHandle`].
///
/// This is used to revoke the capabilities of clones of the [`VaultHandle`]
/// when the [`Vault`] is locked.
pub struct VaultState<S = LockedSecretVaultContent> {
    handle: VaultHandle,
    vault: Box<Vault<S>>,
}

impl<S> VaultState<S> {
    pub fn new(handle: VaultHandle, vault: Box<Vault<S>>) -> Self {
        Self { handle, vault }
    }
}

impl VaultState<UnlockedSecretVaultContent> {
    /// Consumes the Unlocked vault and returns a new [`VaultState`] with the locked vault.
    pub fn lock(self) -> anyhow::Result<VaultState<LockedSecretVaultContent>> {
        self.handle.revoke();
        let locked = self.vault.lock()?;
        let locked_state = VaultState::new(self.handle, Box::new(locked));
        Ok(locked_state)
    }
}

/// A handle granting access to a particular [`Vault`]
#[derive(Clone)]
pub struct VaultHandle {
    id: VaultId,
    cap: VaultSendCap<VaultAccess, VaultId>,
    revoker: VaultRevoker,
}

impl VaultHandle {
    /// Creates a new [`VaultHandle`] with the given ID, capability, and revoker.
    pub fn new(
        id: VaultId,
        cap: VaultSendCap<VaultAccess, VaultId>,
        revoker: VaultRevoker,
    ) -> Self {
        Self { id, cap, revoker }
    }

    /// Returns the ID of the vault this handle grants access to
    pub fn id(&self) -> VaultId {
        self.id.clone()
    }

    /// Returns a reference to the vault's send capability
    pub fn cap(&self) -> &VaultSendCap<VaultAccess, VaultId> {
        &self.cap
    }

    /// Revokes the capability associated with this handle.
    ///
    /// Any subsequent attempt to use any capability returned by
    /// [`VaultHandle::cap`] will return `Err(CapSecError::Revoked)`.
    pub fn revoke(&self) {
        self.revoker.revoke();
    }
}

/// A unique ID for a [`Vault`]
///
/// This is also used as a [`Scope`] for narrowing capabilities
/// such that they only apply to a specific vault
#[derive(Debug, Display, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VaultId(Uuid);
impl VaultId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Scope for VaultId {
    fn check(&self, target: &str) -> Result<(), CapSecError> {
        let target_uuid =
            target
                .parse::<Uuid>()
                .map_err(|_parse_error| CapSecError::OutOfScope {
                    target: target.to_string(),
                    scope: self.0.to_string(),
                })?;

        // VaultId matches iff the target is exactly the same as the VaultId
        if self.0 == target_uuid {
            return Ok(());
        }

        Err(CapSecError::OutOfScope {
            target: target.to_string(),
            scope: self.0.to_string(),
        })
    }
}

/// Top-level Vault object, containing both secret and public portions.
///
/// A `Vault` may be in a locked or unlocked state.
///
/// - In the locked state, the secret content is serialized, encrypted, and
/// base64-encoded.
/// - In the unlocked state, the secret content is a deserialized in-memory
/// object.
/// - In both locked and unlocked states, the public content is a deserialized
/// in-memory object.
///
/// In both locked and unlocked states, there is a distinction between "vault"
/// content and "user" content. The vault content may include machinery data to
/// help the locking and unlocking process (such as holding the salt or password
/// hash), whereas the "user" content is the actual data the user requests to
/// store in the vault.
#[derive(Debug, Serialize, Deserialize)]
pub struct Vault<S = LockedSecretVaultContent> {
    /// The public / unencrypted portion of the vault's content
    ///
    /// This may be used to store display data about the object, such as the
    /// name of a vault
    public: PublicVaultContent,

    /// The secret / encrypted portion of the vault's content
    ///
    /// This may exist in a locked or unlocked state.
    ///
    /// - When locked, the secret is serialized, encrypted, and base64-encoded.
    /// - When unlocked, the secret is a deserialized in-memory object.
    secret: S,
}

impl Vault<LockedSecretVaultContent> {
    /// Construct a new locked vault with the given password.
    pub fn new(password: &str) -> Result<Vault<LockedSecretVaultContent>> {
        let salt = generate_salt();
        Self::with_salt(password, &salt)
    }

    /// Construct a new locked vault with the given password and salt.
    pub fn with_salt(password: &str, salt: &str) -> Result<Vault<LockedSecretVaultContent>> {
        let password_hash = hash_password(password, &salt)
            .context("failed to hash password while creating vault")?;

        // Create empty user content
        let secret_user_content = SecretUserContent::new();

        // Create unlocked vault content, holding the password hash in order to
        // allow locking without prompting for the password again
        let vault = Vault {
            public: PublicVaultContent::new(salt.to_string()),
            secret: UnlockedSecretVaultContent::new(password_hash, secret_user_content),
        };

        // Newly constructed vaults should be locked by default
        let vault = vault.lock()?;
        Ok(vault)
    }
}

impl<S> Vault<S> {
    pub fn id(&self) -> VaultId {
        self.public.id.clone()
    }
}

impl Vault<LockedSecretVaultContent> {
    /// Attempt to unlock the vault using the given password
    ///
    /// Consider this a blocking operation which should be executed within the
    /// scope of a `spawn_blocking` call or similar, due to the serialization
    /// and decryption work done here.
    pub fn unlock(self, password: &str) -> Result<Vault<UnlockedSecretVaultContent>> {
        let salt = &self.public.salt;
        let password_hash = hash_password(password, salt).context("failed to hash password")?;
        let encrypted_text = self.secret.0;
        let decrypted_string =
            decrypt(encrypted_text, password_hash).context("failed to decrypt vault")?;
        let secret_user_content = serde_json::from_str::<SecretUserContent>(&decrypted_string)
            .context("failed to deserialize unlocked vault content")?;
        let unlocked_vault_content = UnlockedSecretVaultContent {
            password_hash,
            user_content: secret_user_content,
        };

        Ok(Vault {
            secret: unlocked_vault_content,
            public: self.public,
        })
    }
}

impl Vault<UnlockedSecretVaultContent> {
    /// Lock the vault, returning it to the locked state which must be unlocked
    /// with the correct password to access the user's secret content
    ///
    /// Consider this a blocking operation which should be executed within the
    /// scope of a `spawn_blocking` call or similar, due to the serialization
    /// and encryption work done here.
    pub fn lock(self) -> Result<Vault<LockedSecretVaultContent>> {
        let password_hash = self.secret.password_hash;
        let secret_content = self.secret.user_content;
        let serialized_secret_content = serde_json::to_string(&secret_content)
            .context("failed to serialize user secret content")?;
        let encrypted_base64 = encrypt(serialized_secret_content, password_hash)
            .context("failed to encrypt user secret content")?;

        Ok(Vault {
            secret: LockedSecretVaultContent(encrypted_base64),
            public: self.public,
        })
    }

    /// Returns a reference to the vault user's public content
    pub fn public_content(&self) -> &PublicUserContent {
        &self.public.user_content
    }

    /// Returns a mutable reference to the vault user's public content
    pub fn public_content_mut(&mut self) -> &mut PublicUserContent {
        &mut self.public.user_content
    }

    /// Returns a reference to the vault user's secret content
    pub fn secret_content(&self) -> &SecretUserContent {
        &self.secret.user_content
    }

    /// Returns a mutable reference to the vault user's secret content
    pub fn secret_content_mut(&mut self) -> &mut SecretUserContent {
        &mut self.secret.user_content
    }
}

/// The serialized, encrypted, base64-encoded form of a vault's content.
#[derive(Debug, Serialize, Deserialize)]
pub struct LockedSecretVaultContent(String);

/// The deserialized, in-memory form of a vault's content.
///
/// This includes both vault machinery (password hash) and the user's actual
/// secret content.
// DO NOT IMPLEMENT SERIALIZE / DESERIALIZE, ONLY LOCKED STATE SHOULD SERIALIZE
#[derive(Debug)]
pub struct UnlockedSecretVaultContent {
    /// Hold the password hash while unlocked to allow auto-locking without
    /// prompting for the password again
    password_hash: [u8; 32],

    /// The user's actual secret stored content, without any vault machinery
    /// included
    user_content: SecretUserContent,
}

impl UnlockedSecretVaultContent {
    /// Create a new unlocked vault content with the given password hash and
    /// user content
    pub fn new(password_hash: [u8; 32], user_content: SecretUserContent) -> Self {
        Self {
            password_hash,
            user_content,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PublicVaultContent {
    /// The unique ID of this vault
    id: VaultId,

    /// The salt that was used to hash this vault's password
    salt: String,

    /// The user's public content, without any vault machinery included
    ///
    /// This may include items such as a display name or a vault avater
    user_content: PublicUserContent,
}

impl PublicVaultContent {
    /// Create a new public vault content, including the salt used to hash the
    /// vault's password.
    pub fn new(salt: String) -> Self {
        Self {
            id: VaultId::generate(),
            salt,
            user_content: PublicUserContent::new(),
        }
    }
}

/// The user's public content, without any vault machinery included
///
/// This may include items such as a display name or a vault avatar
#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PublicUserContent(JsonValue);
impl PublicUserContent {
    /// Create a new empty public user content object
    pub fn new() -> Self {
        Self(json!({}))
    }
}

/// The user's secret content, without any vault machinery included
///
/// This content will be encrypted and stored securely in the vault
#[derive(Debug, Serialize, Deserialize)]
pub struct SecretUserContent(JsonValue);
impl SecretUserContent {
    /// Create a new empty secret user content object
    pub fn new() -> Self {
        Self(json!({}))
    }
}
