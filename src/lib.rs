#![feature(associated_type_defaults)]
#![feature(associated_type_bounds)]
#![feature(mutex_unlock)]

// TODO: Add ?Sized

pub use std::ops::{Deref, DerefMut};

use std::{fmt::{self}, marker::PhantomData, sync::{LockResult, Mutex, MutexGuard, PoisonError, TryLockError, TryLockResult}};

// struct SafeRef<'a, R>(&'a R);

// trait MyDeref {
//     type Target;
//     fn deref(&self) -> &R {
//         self.lock()
//     }
// }

// TODO: Requirement `R: 'mutex_guard` is superfluous.
// TODO: Do we need here both 'mutex_guard and 'base_mutex_guard?
pub struct RefMutexGuard<'mutex_guard, R: 'mutex_guard> {
    base: MutexGuard<'mutex_guard, &'mutex_guard R>,
    phantom: PhantomData<&'mutex_guard R>,
}

// TODO
// Needed?
// impl<R> !Send for RefMutexGuard<'_, R> {}

// TODO: Automatically implemented?
// unsafe impl<R: Sync, &'mutex_guard R> Sync for RefMutexGuard<'_, R> {} // FIXME
unsafe impl<'mutex, R: Sync> Sync for RefMutex<'mutex, R> {}

impl<'mutex_guard, R> RefMutexGuard<'mutex_guard, R>
{
    fn new_helper<'base_mutex_guard>(lock: MutexGuard<'base_mutex_guard, &'base_mutex_guard R>)
        -> RefMutexGuard<'mutex_guard, R>
        where 'base_mutex_guard: 'mutex_guard
    {
        Self { base: lock, phantom: PhantomData }
    }
    // TODO: pub?
    fn new<'base_mutex_guard>(lock: LockResult<MutexGuard<'base_mutex_guard, &'base_mutex_guard R>>)
        -> Result<RefMutexGuard<'mutex_guard, R>, PoisonError<RefMutexGuard<'mutex_guard, R>>>
    {
        match lock {
            Ok(lock) => Ok(Self::new_helper(lock)),
            Err(err) => {
                let e = err.into_inner();
                let e2 = Self::new_helper(e);
                Err(PoisonError::new(e2))
            },
        }
    }
    fn new2<'base_mutex_guard>(lock: TryLockResult<MutexGuard<'base_mutex_guard, &'base_mutex_guard R>>)
        -> Result<RefMutexGuard<'mutex_guard, R>, TryLockError<RefMutexGuard<'mutex_guard, R>>>
        where 'base_mutex_guard: 'mutex_guard
    {
        match lock {
            Ok(lock) => Ok(Self::new_helper(lock)),
            Err(TryLockError::WouldBlock) => Err(TryLockError::WouldBlock),
            Err(TryLockError::Poisoned(err)) => {
                let e = err.into_inner();
                let e2 = Self::new_helper(e);
                Err(TryLockError::Poisoned(PoisonError::new(e2)))
            },
        }
    }
}

impl<R> Deref for RefMutexGuard<'_, R> {
    type Target = R;

    fn deref(&self) -> &R {
        &*self.base.deref()
    }
}

impl<R> DerefMut for RefMutexGuard<'_, R> {
    fn deref_mut(&mut self) -> &mut R {
        &mut *self.base.deref_mut()
    }
}

// TODO: Make better
impl<R: fmt::Debug> fmt::Debug for RefMutexGuard<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RefMutexGuard(")?;
        self.base.fmt(f)?;
        f.write_str(")")?;
        Ok(())
    }
}

impl<R: fmt::Display> fmt::Display for RefMutexGuard<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.base.fmt(f)
    }
}

// sys::MovableMutex isn'mutex_guard public API.
// pub fn guard_lock<'a, R>(guard: &RefMutexGuard<'a, R>) -> &'a sys::MovableMutex {
//     guard_lock(guard.0)
// }

// poison::Flag isn'mutex_guard public API.
// pub fn guard_poison<'a, R>(guard: &RefMutexGuard<'a, R>) -> &'a poison::Flag {
//     guard_poison(guard.0)
// }

/// Like `Mutex` of a reference, but with `Send` trait.
pub struct RefMutex<'mutex, R: 'mutex> {
    base: Mutex<&'mutex R>,
    phantom: PhantomData<&'mutex R>,
}

unsafe impl<'mutex, R> Send for RefMutex<'mutex, R> { }

impl<'mutex, R> From<Mutex<&'mutex R>> for RefMutex<'mutex, R> {
    fn from(mutex: Mutex<&'mutex R>) -> Self {
        Self::new_helper(mutex)
    }
}

impl<'mutex, R> RefMutex<'mutex, R> {
    fn new_helper(mutex: Mutex<&'mutex R>) -> Self {
        Self { base: mutex, phantom: PhantomData }
    }
    /// Creates a new ref mutex in an unlocked state ready for use.
    ///
    /// # Examples
    ///
    /// ```
    /// pub use std::sync::{Arc, Mutex};
    /// pub use std::ops::{Deref, DerefMut};
    /// use ref_mutex::RefMutex;
    ///
    /// let holder = Arc::new(Mutex::new(0));
    /// let r = holder.lock().unwrap();
    /// let mutex = RefMutex::new(r);
    /// ```
    pub fn new(t: &'mutex R) -> Self {
        Self::new_helper(Mutex::new(t))
    }
}

impl<'mutex, R> RefMutex<'mutex, R> {
    /// Acquires a mutex, blocking the current thread until it is able to do so.
    ///
    /// This function will block the local thread until it is available to acquire
    /// the mutex. Upon returning, the thread is the only thread with the lock
    /// held. An RAII guard is returned to allow scoped unlock of the lock. When
    /// the guard goes out of scope, the mutex will be unlocked.
    ///
    /// The exact behavior on locking a mutex in the thread which already holds
    /// the lock is left unspecified. However, this function will not return on
    /// the second call (it might panic or deadlock, for example).
    ///
    /// # Errors
    ///
    /// If another user of this mutex panicked while holding the mutex, then
    /// this call will return an error once the mutex is acquired.
    ///
    /// # Panics
    ///
    /// This function might panic when called if the lock is already held by
    /// the current thread.
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate owning_ref;
    /// pub use std::ops::{Deref, DerefMut};
    /// use std::sync::{Arc, Mutex};
    /// use ref_mutex::RefMutex;
    /// use std::thread;
    ///
    /// let holder = Arc::new(Mutex::new(0));
    /// let r = holder.lock().unwrap();
    /// let mutex = Arc::new(RefMutex::new(r));
    /// let c_mutex = Arc::clone(&mutex);
    ///
    /// thread::spawn(move || {
    ///     *c_mutex.lock().unwrap() = 10;
    /// }).join().expect("thread::spawn failed");
    /// assert_eq!(*mutex.lock().unwrap(), 10);
    /// ```
    pub fn lock<'result: 'mutex>(&'result self) -> LockResult<RefMutexGuard<'result, R>> { // TODO: Are two lifetimes needed?
        RefMutexGuard::new(self.base.lock())
    }

    /// Attempts to acquire this lock.
    ///
    /// If the lock could not be acquired at this time, then [`Err`] is returned.
    /// Otherwise, an RAII guard is returned. The lock will be unlocked when the
    /// guard is dropped.
    ///
    /// This function does not block.
    ///
    /// # Errors
    ///
    /// If another user of this mutex panicked while holding the mutex, then
    /// this call will return the [`Poisoned`] error if the mutex would
    /// otherwise be acquired.
    ///
    /// If the mutex could not be acquired because it is already locked, then
    /// this call will return the [`WouldBlock`] error.
    ///
    /// [`Poisoned`]: TryLockError::Poisoned
    /// [`WouldBlock`]: TryLockError::WouldBlock
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate owning_ref;
    /// pub use std::ops::{Deref, DerefMut};
    /// use std::sync::{Arc, Mutex};
    /// use ref_mutex::RefMutex;
    /// use std::thread;
    /// let mut holder = Arc::new(Mutex::new(0));
    /// let r = holder.lock().unwrap();
    /// let mutex = Arc::new(RefMutex::new(r));
    /// let c_mutex = Arc::clone(&mutex);
    ///
    /// thread::spawn(move || {
    ///     let lock = c_mutex.try_lock();
    ///     if let Ok(guard) = lock {
    ///         *guard = 10;
    ///     } else {
    ///         println!("try_lock failed");
    ///     }
    /// }).join().expect("thread::spawn failed");
    /// assert_eq!(*mutex.lock().unwrap(), 10);
    /// ```
    // FIXME: Simplify?
    // FIXME: The output lifetime must be shorter than `self` and longer than 'mutex
    pub fn try_lock<'mutex_guard>(&/*'mutex*/ self) -> TryLockResult<RefMutexGuard<'mutex_guard, R>>
    where 'mutex: 'mutex_guard
    {
        RefMutexGuard::new2(self.base.try_lock())
    }

    /// Immediately drops the guard, and consequently unlocks the mutex.
    ///
    /// This function is equivalent to calling [`drop`] on the guard but is more self-documenting.
    /// Alternately, the guard will be automatically dropped when it goes out of scope.
    ///
    /// ```
    /// #![feature(mutex_unlock)]
    ///
    /// use std::sync::{Arc, Mutex};
    /// pub use std::ops::{Deref, DerefMut};
    /// use ref_mutex::RefMutex;
    /// let holder = Arc::new(Mutex::new(0));
    /// let r = holder.lock().unwrap();
    /// let mutex = RefMutex::new(r);
    ///
    /// let mut guard = mutex.lock().unwrap();
    /// *guard += 20;
    /// RefMutex::unlock(guard);
    /// ```
    pub fn unlock(guard: RefMutexGuard<'_, R>) {
        Mutex::unlock(guard.base)
    }

    /// Determines whether the mutex is poisoned.
    ///
    /// If another thread is active, the mutex can still become poisoned at any
    /// time. You should not trust a `false` value for program correctness
    /// without additional synchronization.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::{Arc, Mutex};
    /// use std::ops::{Deref, DerefMut};
    /// use ref_mutex::RefMutex;
    /// use std::thread;
    ///
    /// let holder = Arc::new(Mutex::new(0));
    /// let r = holder.lock().unwrap();
    /// let mutex = Arc::new(RefMutex::new(r));
    /// let c_mutex = Arc::clone(&mutex);
    ///
    /// let _ = thread::spawn(move || {
    ///     let _lock = c_mutex.lock().unwrap();
    ///     panic!(); // the mutex gets poisoned
    /// }).join();
    /// assert_eq!(mutex.is_poisoned(), true);
    /// ```
    #[inline]
    pub fn is_poisoned(&self) -> bool {
        self.base.is_poisoned()
    }

    /// Consumes this mutex, returning the underlying data.
    ///
    /// # Errors
    ///
    /// If another user of this mutex panicked while holding the mutex, then
    /// this call will return an error instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::{Arc, Mutex};
    /// pub use std::ops::{Deref, DerefMut};
    /// use  ref_mutex::RefMutex;
    ///
    /// let holder = Arc::new(Mutex::new(0));
    /// let r = holder.lock().unwrap();
    /// let mutex = RefMutex::new(r);
    /// assert_eq!(*mutex.into_inner().unwrap(), 0);
    /// ```
    pub fn into_inner(self) -> LockResult<&'mutex R>
    {
        self.base.into_inner()
    }
}

impl<'mutex, R: Copy> RefMutex<'mutex, R> {
    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this call borrows the `Mutex` mutably, no actual locking needs to
    /// take place -- the mutable borrow statically guarantees no locks exist.
    ///
    /// # Errors
    ///
    /// If another user of this mutex panicked while holding the mutex, then
    /// this call will return an error instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::{Arc, Mutex};
    /// use std::ops::{Deref, DerefMut};
    /// use ref_mutex::RefMutex;
    ///
    /// extern crate owning_ref;
    /// let mut holder = Arc::new(Mutex::new(0));
    /// let r = holder.lock().unwrap();
    /// let mutex = Arc::new(RefMutex::new(r));
    /// *mutex.lock().unwrap() = 10;
    /// assert_eq!(*mutex.lock().unwrap(), 10);
    /// ```
    pub fn get_mut(&mut self) -> LockResult<&'mutex R> {
        match self.base.get_mut() {
            Ok(r) => Ok(*r),
            Err(err) => Err(PoisonError::new(*err.into_inner())),
        }
    }
}

impl<'mutex, R: fmt::Debug> fmt::Debug for RefMutex<'mutex, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("RefMutex");
        match self.try_lock() {
            Ok(guard) => {
                d.field("data", &&*guard.base);
            }
            Err(TryLockError::Poisoned(err)) => {
                d.field("data", &&**err.get_ref());
            }
            Err(TryLockError::WouldBlock) => {
                struct LockedPlaceholder;
                impl fmt::Debug for LockedPlaceholder {
                    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str("<locked>")
                    }
                }
                d.field("data", &LockedPlaceholder);
            }
        }
        d.field("poisoned", &"TODO"/*&self.base.poison.get()*/);
        d.finish_non_exhaustive()
    }
}

// impl<R> MyDeref for Mutex<SafeRef<'_, R>> {
//     type Target = R;
//     fn deref(&self) -> &R {
//         self.lock()
//     }
// }

// impl<R> Deref for dyn MyDeref<Target = R> {
//     type Target = R;
//     fn deref(&self) -> &R {
//         self.lock()
//     }
// }
