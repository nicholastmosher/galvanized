//! Strict capability management.
//!
//! This is a combination of all of `capsec`'s runtime capability features:
//!
//! - Revokable capabilities
//! - Timed capabilities
//! - Attenuated capabilities
//!   - Scoped to particular target instances
//! - A caching wrapper ([`CapCell`]) that gates in-memory content behind a
//!   capability, blanking it when the capability is revoked or expires.
//!
//! ## Sliding-window TTL
//!
//! [`StrictCap::try_cap`] (and [`StrictSendCap::try_cap`]) refresh the
//! capability's expiry on every *successful* check: a capability in active use
//! does not expire mid-operation. The original TTL is retained for the refresh,
//! so a cap left idle still expires at its original deadline. This is the
//! "sliding window" policy ratified for Galvanized's session capabilities
//! (T1, design question §3).

use std::{
    marker::PhantomData,
    sync::{
        Arc, Mutex,
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
    ttl: Duration,
    // Interior-mutable so a successful `try_cap` can refresh the deadline via
    // `&self` (sliding-window TTL).
    expires_at: Mutex<Instant>,
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
            ttl,
            expires_at: Mutex::new(Instant::now() + ttl),
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
    ///
    /// On success, the expiry deadline is refreshed to `now + ttl` (sliding
    /// window), so a capability in active use does not expire mid-operation.
    pub fn try_cap(&self, target: &str) -> Result<Cap<P>, CapSecError> {
        self.scope.check(target)?;

        let now = Instant::now();
        let mut expires_at = self
            .expires_at
            .lock()
            .expect("StrictCap expires_at mutex poisoned");
        if now >= *expires_at {
            return Err(CapSecError::Expired);
        }

        if self.revoked.load(Ordering::Acquire) {
            return Err(CapSecError::Revoked);
        }

        // Sliding-window refresh: a successful check extends the deadline.
        *expires_at = now + self.ttl;
        drop(expires_at);

        Ok(self.cap.clone())
    }

    /// Checks whether `target` is within this capability's scope.
    #[must_use = "ignoring a scope check silently discards scope violations"]
    pub fn is_in_scope(&self, target: &str) -> Result<(), CapSecError> {
        self.scope.check(target)
    }

    /// Checks liveness only — expiry and revocation — without a scope check,
    /// and refreshes the TTL on success (sliding window).
    ///
    /// Use this when the capability authorizes an operation that is not
    /// naturally described by a target string (e.g. "sign as this Profile"),
    /// but the caller performs its own structured scope check. For ordinary
    /// scoped access, prefer [`try_cap`](StrictCap::try_cap), which checks the
    /// scope as well.
    pub fn is_active(&self) -> Result<(), CapSecError> {
        let now = Instant::now();
        let mut expires_at = self
            .expires_at
            .lock()
            .expect("StrictCap expires_at mutex poisoned");
        if now >= *expires_at {
            return Err(CapSecError::Expired);
        }
        if self.revoked.load(Ordering::Acquire) {
            return Err(CapSecError::Revoked);
        }
        *expires_at = now + self.ttl;
        Ok(())
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
    /// Always use [`try_cap`](StrictCap::try_cap) for actual access.
    pub fn is_expired(&self) -> bool {
        let expires_at = self
            .expires_at
            .lock()
            .expect("StrictCap expires_at mutex poisoned");
        Instant::now() >= *expires_at
    }

    /// Returns the remaining duration before expiry.
    ///
    /// Returns [`Duration::ZERO`] if the capability has already expired.
    pub fn remaining(&self) -> Duration {
        let expires_at = self
            .expires_at
            .lock()
            .expect("StrictCap expires_at mutex poisoned");
        expires_at.saturating_duration_since(Instant::now())
    }

    /// Returns the TTL used to refresh this capability on successful access.
    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Returns the scope this capability is attenuated to.
    pub fn scope(&self) -> &S {
        &self.scope
    }

    /// Converts this capability into a [`StrictSendCap`] that can cross thread boundaries.
    ///
    /// This is an explicit opt-in — you're acknowledging that this capability
    /// will be used in a multi-threaded context.
    pub fn make_send(self) -> StrictSendCap<P, S> {
        let expires_at = self
            .expires_at
            .into_inner()
            .expect("StrictCap expires_at mutex poisoned");
        StrictSendCap {
            _phantom: PhantomData,
            cap: self.cap,

            revoked: self.revoked,
            scope: self.scope,
            ttl: self.ttl,
            expires_at: Mutex::new(expires_at),
        }
    }
}

impl<P, S> Clone for StrictCap<P, S>
where
    P: Permission,
    S: Scope + Clone,
{
    fn clone(&self) -> Self {
        let expires_at = self
            .expires_at
            .lock()
            .expect("StrictCap expires_at mutex poisoned");
        Self {
            _phantom: PhantomData,
            _not_send: PhantomData,
            cap: self.cap.clone(),

            revoked: Arc::clone(&self.revoked),
            scope: self.scope.clone(),
            ttl: self.ttl,
            expires_at: Mutex::new(*expires_at),
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
/// Created via [`StrictCap::make_send`] or [`StrictSendCap::new`]. Unlike
/// [`StrictCap`], this implements `Send + Sync`, making it usable with
/// `std::thread::spawn`, `tokio::spawn`, etc.
pub struct StrictSendCap<P, S>
where
    P: Permission,
    S: Scope + Clone,
{
    _phantom: PhantomData<P>,
    cap: Cap<P>,

    revoked: Arc<AtomicBool>,
    scope: S,
    ttl: Duration,
    expires_at: Mutex<Instant>,
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
    /// Creates a revocable capability by consuming a [`SendCap<P>`] as proof of possession.
    ///
    /// Returns a `(StrictSendCap<P>, StrictRevoker)` pair. The `Revoker` can
    /// invalidate this capability (and all clones) from any thread.
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
            ttl,
            expires_at: Mutex::new(Instant::now() + ttl),
        };
        (secrets_cap, revoker)
    }

    /// Attempts to obtain a [`Cap<P>`] from this revocable capability.
    ///
    /// Returns `Ok(Cap<P>)` if the target is in scope, the capability has not
    /// expired, and has not been revoked. On success the expiry deadline is
    /// refreshed to `now + ttl` (sliding window).
    pub fn try_cap(&self, target: &str) -> Result<Cap<P>, CapSecError> {
        self.scope.check(target)?;

        let now = Instant::now();
        let mut expires_at = self
            .expires_at
            .lock()
            .expect("StrictSendCap expires_at mutex poisoned");
        if now >= *expires_at {
            return Err(CapSecError::Expired);
        }

        if self.revoked.load(Ordering::Acquire) {
            return Err(CapSecError::Revoked);
        }

        *expires_at = now + self.ttl;
        drop(expires_at);

        Ok(self.cap.clone())
    }

    /// Advisory check — returns `true` if the capability has been revoked.
    ///
    /// The result is immediately stale; do not use for control flow.
    /// Always use [`try_cap`](StrictCap::try_cap) for actual access.
    pub fn is_revoked(&self) -> bool {
        self.revoked.load(Ordering::Acquire)
    }

    /// Checks liveness only — expiry and revocation — without a scope check,
    /// and refreshes the TTL on success (sliding window). See
    /// [`StrictCap::is_active`] for the rationale.
    pub fn is_active(&self) -> Result<(), CapSecError> {
        let now = Instant::now();
        let mut expires_at = self
            .expires_at
            .lock()
            .expect("StrictSendCap expires_at mutex poisoned");
        if now >= *expires_at {
            return Err(CapSecError::Expired);
        }
        if self.revoked.load(Ordering::Acquire) {
            return Err(CapSecError::Revoked);
        }
        *expires_at = now + self.ttl;
        Ok(())
    }

    /// Advisory check — returns `true` if the capability has expired.
    ///
    /// The result is immediately stale; do not use for control flow.
    pub fn is_expired(&self) -> bool {
        let expires_at = self
            .expires_at
            .lock()
            .expect("StrictSendCap expires_at mutex poisoned");
        Instant::now() >= *expires_at
    }

    /// Returns the remaining duration before expiry.
    ///
    /// Returns [`Duration::ZERO`] if the capability has already expired.
    pub fn remaining(&self) -> Duration {
        let expires_at = self
            .expires_at
            .lock()
            .expect("StrictSendCap expires_at mutex poisoned");
        expires_at.saturating_duration_since(Instant::now())
    }

    /// Returns the TTL used to refresh this capability on successful access.
    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Returns the scope this capability is attenuated to.
    pub fn scope(&self) -> &S {
        &self.scope
    }
}

impl<P, S> Clone for StrictSendCap<P, S>
where
    P: Permission,
    S: Scope + Clone,
{
    fn clone(&self) -> Self {
        let expires_at = self
            .expires_at
            .lock()
            .expect("StrictSendCap expires_at mutex poisoned");
        Self {
            _phantom: PhantomData,
            cap: self.cap.clone(),

            revoked: Arc::clone(&self.revoked),
            scope: self.scope.clone(),
            ttl: self.ttl,
            expires_at: Mutex::new(*expires_at),
        }
    }
}

/// A caching wrapper that gates an in-memory value behind a capability.
///
/// `CapCell<T, C, P>` holds an inner `T` and only yields `Some(&T)` iff the
/// capability `C` (a [`CapProvider<P>`]) is currently valid for `target` — not
/// expired, not revoked, and in scope. On lock/timeout/revoke the inner value
/// becomes inaccessible through the wrapper.
///
/// This is the mechanism that gates *content held in memory* behind a
/// capability — distinct from [`StrictCap::try_cap`], which gates the *right to
/// perform an operation*. Revoking the underlying capability (via its
/// [`StrictRevoker`]) blanks every `CapCell` holding a clone of it: this is how
/// locking the application makes gated content inaccessible.
///
/// `C` is typically a [`StrictCap`] or [`StrictSendCap`]. Accessing the inner
/// value through [`get`](CapCell::get) / [`get_mut`](CapCell::get_mut) refreshes
/// the capability's TTL (sliding window), so content in active use does not
/// expire mid-operation.
pub struct CapCell<T, C, P>
where
    C: CapProvider<P>,
    P: Permission,
{
    inner: T,
    cap: C,
    target: String,
    _perm: PhantomData<P>,
}

impl<T, C, P> CapCell<T, C, P>
where
    C: CapProvider<P>,
    P: Permission,
{
    /// Wraps `inner` so it is only accessible while `cap` is valid for `target`.
    ///
    /// The caller retains the [`StrictRevoker`] paired with `cap` (or a clone of
    /// it); revoking it blanks this cell. `cap` is typically a clone of a
    /// session capability, so revoking the session revokes the cell too.
    pub fn new(cap: C, target: impl Into<String>, inner: T) -> Self {
        Self {
            inner,
            cap,
            target: target.into(),
            _perm: PhantomData,
        }
    }

    /// Returns the inner value iff the capability is currently valid for the
    /// target. Refreshes the capability's TTL on a successful check.
    pub fn get(&self) -> Option<&T> {
        self.cap.provide_cap(&self.target).ok().map(|_| &self.inner)
    }

    /// Returns a mutable reference to the inner value iff the capability is
    /// currently valid for the target. Refreshes the capability's TTL on a
    /// successful check.
    pub fn get_mut(&mut self) -> Option<&mut T> {
        if self.cap.provide_cap(&self.target).is_ok() {
            Some(&mut self.inner)
        } else {
            None
        }
    }

    /// Returns `true` iff the capability is currently valid for the target.
    /// Refreshes the TTL on success.
    pub fn is_valid(&self) -> bool {
        self.cap.provide_cap(&self.target).is_ok()
    }

    /// Returns the capability backing this cell.
    pub fn cap(&self) -> &C {
        &self.cap
    }

    /// Returns the target string this cell checks the capability against.
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Replaces the inner value, keeping the same capability and target. This
    /// is a management operation (not capability-gated): use it to refresh the
    /// cached content while the capability is still valid (e.g. reloading from
    /// the vault after an update). Returns the previous inner value.
    pub fn replace_inner(&mut self, inner: T) -> T {
        std::mem::replace(&mut self.inner, inner)
    }

    /// Drops the cell and returns the inner value, regardless of capability
    /// state. Use this only when the caller is independently authoritative
    /// (e.g. tearing down); prefer [`get`](CapCell::get) for gated access.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use capsec::{CapSecError, FsRead, Scope};

    /// A scope that matches every target — used to isolate the
    /// revocation/TTL/refresh behaviour from scope checking.
    #[derive(Clone)]
    struct AnyScope;
    impl Scope for AnyScope {
        fn check(&self, _target: &str) -> Result<(), CapSecError> {
            Ok(())
        }
    }

    fn fresh_cap(ttl: Duration) -> (StrictCap<FsRead, AnyScope>, StrictRevoker) {
        let root = capsec::test_root();
        StrictCap::new(root.grant::<FsRead>(), ttl, AnyScope)
    }

    /// Extracts the error variant from a `try_cap` result, panicking on success.
    fn err_of(r: Result<capsec::Cap<FsRead>, CapSecError>) -> CapSecError {
        match r {
            Err(e) => e,
            Ok(_) => panic!("expected a capability error, got Ok"),
        }
    }

    #[test]
    fn try_cap_succeeds_when_valid() {
        let (cap, _revoker) = fresh_cap(Duration::from_secs(60));
        assert!(cap.try_cap("any").is_ok());
    }

    #[test]
    fn revoke_blanks_try_cap() {
        let (cap, revoker) = fresh_cap(Duration::from_secs(60));
        assert!(cap.try_cap("any").is_ok());
        revoker.revoke();
        assert!(matches!(err_of(cap.try_cap("any")), CapSecError::Revoked));
    }

    #[test]
    fn expiry_blanks_try_cap() {
        let (cap, _revoker) = fresh_cap(Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(5));
        assert!(matches!(err_of(cap.try_cap("any")), CapSecError::Expired));
    }

    #[test]
    fn sliding_window_refreshes_expiry() {
        let (cap, _revoker) = fresh_cap(Duration::from_millis(20));
        // Repeatedly access within the window; the deadline should keep moving
        // forward and the cap should remain usable well past a single TTL.
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(5));
            assert!(
                cap.try_cap("any").is_ok(),
                "cap should remain valid under active use"
            );
        }
    }

    #[test]
    fn idle_cap_still_expires() {
        let (cap, _revoker) = fresh_cap(Duration::from_millis(5));
        // Don't touch it; it should expire on its own.
        std::thread::sleep(Duration::from_millis(15));
        assert!(matches!(err_of(cap.try_cap("any")), CapSecError::Expired));
    }

    #[test]
    fn cap_cell_yields_some_while_valid() {
        let (cap, _revoker) = fresh_cap(Duration::from_secs(60));
        let cell = CapCell::new(cap, "any", 42_u32);
        assert_eq!(cell.get(), Some(&42));
        assert!(cell.is_valid());
    }

    #[test]
    fn cap_cell_yields_none_after_revoke() {
        let (cap, revoker) = fresh_cap(Duration::from_secs(60));
        let cell = CapCell::new(cap, "any", 42_u32);
        assert_eq!(cell.get(), Some(&42));
        revoker.revoke();
        assert_eq!(cell.get(), None);
        assert!(!cell.is_valid());
    }

    #[test]
    fn cap_cell_yields_none_after_expiry() {
        let (cap, _revoker) = fresh_cap(Duration::from_millis(1));
        let cell = CapCell::new(cap, "any", 42_u32);
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(cell.get(), None);
    }

    #[test]
    fn cap_cell_get_mut_gated() {
        let (cap, revoker) = fresh_cap(Duration::from_secs(60));
        let mut cell = CapCell::new(cap, "any", 42_u32);
        *cell.get_mut().unwrap() = 7;
        assert_eq!(cell.get(), Some(&7));
        revoker.revoke();
        assert!(cell.get_mut().is_none());
    }

    #[test]
    fn send_cap_crosses_thread_and_revokes() {
        let root = capsec::test_root();
        let (send, revoker) = StrictSendCap::new(
            root.grant::<FsRead>().make_send(),
            Duration::from_secs(60),
            AnyScope,
        );
        let send2 = send.clone();
        let handle = std::thread::spawn(move || send2.try_cap("any").is_ok());
        assert!(handle.join().unwrap());
        revoker.revoke();
        assert!(matches!(err_of(send.try_cap("any")), CapSecError::Revoked));
    }
}
