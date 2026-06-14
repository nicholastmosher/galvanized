//! Strict capability management.
//!
//! This is a combination of all of `capsec`'s runtime capability features:
//!
//! - Revokable capabilities
//! - Timed capabilities
//! - Attenuated capabilities
//!   - Scoped to particular target instances

use std::{
    marker::PhantomData,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use capsec::{Cap, CapProvider, CapSecError, Permission, Scope, SendCap};

/// A revocable, timed, scoped capability token proving the holder has permission `P`.
///
/// Created via [`StrictCap::new`], which consumes a [`Cap<P>`] as proof of
/// possession and returns a `(StrictCap<P>, StrictRevoker)` pair.
///
/// `!Send + !Sync` by default — use [`make_send`](StrictCap::make_send) for
/// cross-thread transfer. Cloning shares the same revocation state: revoking
/// one clone revokes all of them.
pub struct StrictCap<P, S>
where
    P: Permission,
    S: Scope + Clone,
{
    _phantom: PhantomData<P>,
    // PhantomData<*const ()> makes RuntimeCap !Send + !Sync
    _not_send: PhantomData<*const ()>,
    cap: Cap<P>,

    revoked: Arc<AtomicBool>,
    scope: S,
    expires_at: Instant,
}

impl<P, S> CapProvider<P> for StrictCap<P, S>
where
    P: Permission,
    S: Scope + Clone,
{
    fn provide_cap(&self, target: &str) -> Result<Cap<P>, CapSecError> {
        self.try_cap(target)
    }
}

impl<P, S> StrictCap<P, S>
where
    P: Permission,
    S: Scope + Clone,
{
    /// Creates a revocable capability by consuming a [`Cap<P>`] as proof of possession.
    ///
    /// Returns a `(StrictCap<P>, StrictRevoker)` pair. The `Revoker` can invalidate
    /// this capability (and all its clones) from any thread.
    pub fn new(cap: Cap<P>, ttl: Duration, scope: S) -> (Self, StrictRevoker) {
        let revoked = Arc::new(AtomicBool::new(false));
        let revoker = StrictRevoker {
            revoked: Arc::clone(&revoked),
        };
        let secrets_cap = Self {
            _phantom: PhantomData,
            _not_send: PhantomData,
            cap,

            revoked,
            scope,
            expires_at: Instant::now() + ttl,
        };
        (secrets_cap, revoker)
    }

    /// Attempts to obtain a [`Cap<P>`] from this strict capability.
    ///
    /// Must pass three checks to obtain the capability:
    ///
    /// - The target must be within this capability's scope.
    /// - The capability must not have expired.
    /// - The capability must not have been revoked.
    pub fn try_cap(&self, target: &str) -> Result<Cap<P>, CapSecError> {
        self.scope.check(target)?;

        if Instant::now() >= self.expires_at {
            return Err(CapSecError::Expired);
        }

        if self.revoked.load(Ordering::Acquire) {
            return Err(CapSecError::Revoked);
        }

        Ok(self.cap.clone())
    }

    /// Checks whether `target` is within this capability's scope.
    #[must_use = "ignoring a scope check silently discards scope violations"]
    pub fn is_in_scope(&self, target: &str) -> Result<(), CapSecError> {
        self.scope.check(target)
    }

    /// Advisory check — returns `true` if the capability has been revoked.
    ///
    /// The result is immediately stale; do not use for control flow.
    /// Always use [`try_cap`](StrictCap::try_cap) for actual access.
    pub fn is_revoked(&self) -> bool {
        self.revoked.load(Ordering::Acquire)
    }

    /// Advisory check — returns `true` if the capability has expired.
    ///
    /// The result is immediately stale; do not use for control flow.
    /// Always use [`try_cap`](TimedCap::try_cap) for actual access.
    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }

    /// Returns the remaining duration before expiry.
    ///
    /// Returns [`Duration::ZERO`] if the capability has already expired.
    pub fn remaining(&self) -> Duration {
        self.expires_at.saturating_duration_since(Instant::now())
    }

    /// Converts this capability into a [`StrictSendCap`] that can cross thread boundaries.
    ///
    /// This is an explicit opt-in — you're acknowledging that this capability
    /// will be used in a multi-threaded context.
    pub fn make_send(self) -> StrictSendCap<P, S> {
        StrictSendCap {
            _phantom: PhantomData,
            cap: self.cap,

            revoked: self.revoked,
            scope: self.scope,
            expires_at: self.expires_at,
        }
    }
}

impl<P, S> Clone for StrictCap<P, S>
where
    P: Permission,
    S: Scope + Clone,
{
    fn clone(&self) -> Self {
        Self {
            _phantom: PhantomData,
            _not_send: PhantomData,
            cap: self.cap.clone(),

            revoked: Arc::clone(&self.revoked),
            scope: self.scope.clone(),
            expires_at: self.expires_at.clone(),
        }
    }
}

/// A handle that can revoke its associated [`StrictCap`] (and all clones).
///
/// `Revoker` is `Send + Sync` and `Clone` — multiple owners can hold revokers
/// to the same capability, and any of them can revoke it from any thread.
/// Revocation is idempotent: calling [`revoke`](Revoker::revoke) multiple times
/// is safe and has no additional effect.
pub struct StrictRevoker {
    revoked: Arc<AtomicBool>,
}

impl StrictRevoker {
    /// Revokes the associated capability. All subsequent calls to
    /// [`RuntimeCap::try_cap`] (and clones) will return `Err(CapSecError::Revoked)`.
    ///
    /// Idempotent — calling multiple times is safe.
    pub fn revoke(&self) {
        self.revoked.store(true, Ordering::Release);
    }

    /// Returns `true` if the capability has been revoked.
    pub fn is_revoked(&self) -> bool {
        self.revoked.load(Ordering::Acquire)
    }
}

impl Clone for StrictRevoker {
    fn clone(&self) -> Self {
        Self {
            revoked: Arc::clone(&self.revoked),
        }
    }
}

/// A thread-safe, revocable, timed, scoped capability token proving the holder
/// has permission `P`.
///
/// Created via [`StrictCap::make_send`]. Unlike [`StrictCap`], this implements
/// `Send + Sync`, making it usable with `std::thread::spawn`, `tokio::spawn`, etc.
pub struct StrictSendCap<P, S>
where
    P: Permission,
    S: Scope + Clone,
{
    _phantom: PhantomData<P>,
    cap: Cap<P>,

    revoked: Arc<AtomicBool>,
    scope: S,
    expires_at: Instant,
}

// SAFETY: StrictSendCap is explicitly opted into cross-thread transfer via make_send().
// The inner Arc<AtomicBool> is already Send+Sync; PhantomData<P> is Send+Sync when P is.
// Permission types are marker traits (ZSTs) that are always Send+Sync.
unsafe impl<P: Permission, S: Scope + Clone> Send for StrictSendCap<P, S> {}
unsafe impl<P: Permission, S: Scope + Clone> Sync for StrictSendCap<P, S> {}

impl<P, S> CapProvider<P> for StrictSendCap<P, S>
where
    P: Permission,
    S: Scope + Clone,
{
    fn provide_cap(&self, target: &str) -> Result<Cap<P>, CapSecError> {
        self.try_cap(target)
    }
}

impl<P, S> StrictSendCap<P, S>
where
    P: Permission,
    S: Scope + Clone,
{
    /// Creates a revocable capability by consuming a [`Cap<P>`] as proof of possession.
    ///
    /// Returns a `(StrictCap<P>, StrictRevoker)` pair. The `Revoker` can invalidate
    /// this capability (and all its clones) from any thread.
    pub fn new(cap: SendCap<P>, ttl: Duration, scope: S) -> (Self, StrictRevoker) {
        let revoked = Arc::new(AtomicBool::new(false));
        let revoker = StrictRevoker {
            revoked: Arc::clone(&revoked),
        };
        let secrets_cap = Self {
            _phantom: PhantomData,
            cap: cap.as_cap(),

            revoked,
            scope,
            expires_at: Instant::now() + ttl,
        };
        (secrets_cap, revoker)
    }

    /// Attempts to obtain a [`Cap<P>`] from this revocable capability.
    ///
    /// Returns `Ok(Cap<P>)` if still active, or `Err(CapSecError::Revoked)` if
    /// the associated [`Revoker`] has been invoked.
    pub fn try_cap(&self, target: &str) -> Result<Cap<P>, CapSecError> {
        self.scope.check(target)?;

        if Instant::now() >= self.expires_at {
            return Err(CapSecError::Expired);
        }

        if self.revoked.load(Ordering::Acquire) {
            return Err(CapSecError::Revoked);
        }

        Ok(self.cap.clone())
    }

    /// Advisory check — returns `true` if the capability has been revoked.
    ///
    /// The result is immediately stale; do not use for control flow.
    /// Always use [`try_cap`](StrictCap::try_cap) for actual access.
    pub fn is_revoked(&self) -> bool {
        self.revoked.load(Ordering::Acquire)
    }

    /// Advisory check — returns `true` if the capability has expired.
    ///
    /// The result is immediately stale; do not use for control flow.
    /// Always use [`try_cap`](TimedCap::try_cap) for actual access.
    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }

    /// Returns the remaining duration before expiry.
    ///
    /// Returns [`Duration::ZERO`] if the capability has already expired.
    pub fn remaining(&self) -> Duration {
        self.expires_at.saturating_duration_since(Instant::now())
    }
}

impl<P, S> Clone for StrictSendCap<P, S>
where
    P: Permission,
    S: Scope + Clone,
{
    fn clone(&self) -> Self {
        Self {
            _phantom: PhantomData,
            cap: self.cap.clone(),

            revoked: Arc::clone(&self.revoked),
            scope: self.scope.clone(),
            expires_at: self.expires_at.clone(),
        }
    }
}
