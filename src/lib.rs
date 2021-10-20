#![feature(associated_type_defaults)]
#![feature(associated_type_bounds)]
#![feature(mutex_unlock)]
#![feature(negative_impls)]

// TODO: Add ?Sized

pub use std::ops::{Deref, DerefMut};

use std::{fmt::{self}, marker::PhantomData, sync::{Arc, LockResult, Mutex, MutexGuard, PoisonError, TryLockError, TryLockResult}};
// struct SafeRef<'a, T>(&'a T);

// trait MyDeref {
//     type Target;
//     fn deref(&self) -> &T {
//         self.lock()
//     }
// }

pub struct RefMutexGuard<'r, 'v, T> {
    // Having the same lifetime 'r of the reference, we may have different lifetimes 'v of the underlyng type T.
    base: MutexGuard<'r, &'v T>,
    phantom: PhantomData<&'r T>,
}

impl<T> !Send for RefMutexGuard<'_, '_, T> {}

// The below test shows it is automatically implemented.
// unsafe impl<T: Sync, &'mutex_guard T> Sync for RefMutexGuard<'_, T> {} // FIXME

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use crate::RefMutexGuard;

    #[test]
    fn test_sync_guard() {
        let mutex = Mutex::new(&0);
        let lock = mutex.lock().unwrap();
        let _: &dyn Sync = &RefMutexGuard::new_helper(lock);
    }
}

unsafe impl<'mutex, T: Sync> Sync for RefMutex<'mutex, T> {}

impl<'r, 'v, T> RefMutexGuard<'r, 'v, T>
{
    fn new_helper(lock: MutexGuard<'r, &'v T>) -> Self
    {
        Self { base: lock, phantom: PhantomData }
    }
    // TODO: pub?
    fn new(lock: LockResult<MutexGuard<'r, &'v T>>) -> Result<Self, PoisonError<Self>>
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
    fn new2(lock: TryLockResult<MutexGuard<'r, &'v T>>) -> Result<Self, TryLockError<Self>>
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

impl<'v, T> Deref for RefMutexGuard<'_, 'v, T> {
    type Target = &'v T;

    fn deref(&self) -> &&'v T {
        &*self.base.deref()
    }
}

// It's impossible: If two threads obtained mutable references to T and then copy them,
// they would be able later both modify the value of T.
impl<'v, T> DerefMut for RefMutexGuard<'_, 'v, T> {
    fn deref_mut(&mut self) -> &mut &'v T {
        &mut *self.base.deref_mut()
    }
}

// TODO: Make better
impl<T: fmt::Debug> fmt::Debug for RefMutexGuard<'_, '_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RefMutexGuard(")?;
        self.base.fmt(f)?;
        f.write_str(")")?;
        Ok(())
    }
}

impl<T: fmt::Display> fmt::Display for RefMutexGuard<'_, '_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.base.fmt(f)
    }
}

// sys::MovableMutex isn'mutex_guard public API.
// pub fn guard_lock<'a, T>(guard: &RefMutexGuard<'a, T>) -> &'a sys::MovableMutex {
//     guard_lock(guard.0)
// }

// poison::Flag isn'mutex_guard public API.
// pub fn guard_poison<'a, T>(guard: &RefMutexGuard<'a, T>) -> &'a poison::Flag {
//     guard_poison(guard.0)
// }

/// Like `Mutex` of a reference, but with `Send` trait, even if T isn't `Send`.
pub struct RefMutex<'mutex, T> {
    base: Mutex<&'mutex T>,
    phantom: PhantomData<&'mutex T>,
}

unsafe impl<'mutex, T> Send for RefMutex<'mutex, T> { }

#[cfg(test)]
mod tests2 {
    use crate::RefMutex;

    #[test]
    fn test_sync_guard() {
        #[derive(Debug)]
        struct NotSend {}
        impl !Send for NotSend {}

        let mutex = RefMutex::new(&NotSend{}); // RefMutex should be `Send` even if the argument is `!Send`.
        let _: &dyn Sync = &mutex;
        let _: &dyn Send = &mutex;
    }
}

impl<'mutex, T: fmt::Debug> From<Mutex<&'mutex T>> for RefMutex<'mutex, T> {
    fn from(mutex: Mutex<&'mutex T>) -> Self {
        Self::new_helper(mutex)
    }
}

// TODO: pub here and in other places.
impl<'mutex, T: fmt::Debug> RefMutex<'mutex, T> {
    fn new_helper(mutex: Mutex<&'mutex T>) -> Self {
        Self { base: mutex, phantom: PhantomData }
    }
    pub fn move_mutex(r: Arc<Mutex<&'mutex T>>) -> Arc<Self> { // TODO: Make this a method of `Mutex`?
        let mutex = Arc::try_unwrap(r).unwrap();
        Arc::new(Self::new_helper(mutex))
    }
    // fn clone_double_mutex(r: Arc<Mutex<Arc<Mutex<&'mutex T>>>>) -> Self { // needed?
    //     let borrowed = r.clone();
    //     let mut inner = *borrowed.lock().unwrap();
    //     let mutex = Arc::try_unwrap(inner).unwrap();
    //     Self::new_helper(mutex)
    // }
    /// Creates a new ref mutex in an unlocked state ready for use.
    ///
    /// # Examples
    ///
    /// ```
    /// pub use std::sync::{Arc, Mutex};
    /// pub use std::ops::{Deref, DerefMut};
    /// use ref_mutex::RefMutex;
    ///
    /// let holder = Arc::new(Mutex::new(&10));
    /// let mutex = RefMutex::move_mutex(holder);
    /// ```
    pub fn new(t: &'mutex T) -> Self {
        Self::new_helper(Mutex::new(t))
    }
}

impl<'mutex, T> RefMutex<'mutex, T> {
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
    /// let holder = Arc::new(Mutex::new(&10)); // TODO: A method to create RefMutex directly from value.
    /// let mutex = RefMutex::move_mutex(holder);
    /// let c_mutex = Arc::clone(&mutex);
    ///
    /// thread::spawn(move || {
    ///     *c_mutex.lock().unwrap() = &20;
    ///     assert_eq!(**mutex.lock().unwrap(), 20);
    /// }).join().expect("thread::spawn failed");
    /// ```
    /// API note: The lifetime of T can be only 'mutex because the lifetime of the result of `self.base.lock()` is such.
    pub fn lock(&self) -> LockResult<RefMutexGuard<'_, 'mutex, T>> { // TODO: 'mutex -> '_
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
    /// use std::borrow::Borrow;
    /// let mut holder = Arc::new(Mutex::new(&10));
    /// let mutex = RefMutex::move_mutex(holder);
    /// let c_mutex = Arc::clone(&mutex);
    ///
    /// thread::spawn(move || {
    ///     let mut lock = c_mutex.try_lock();
    ///     if let Ok(ref mut mutex) = lock {
    ///         **mutex = &20;
    ///         assert_eq!(***mutex, 20);
    ///     } else {
    ///         println!("try_lock failed");
    ///     }
    /// }).join().expect("thread::spawn failed");
    /// ```
    /// API note: The lifetime of T can be only 'mutex because the lifetime of the result of `self.base.lock()` is such.
    pub fn try_lock(&self) -> TryLockResult<RefMutexGuard<'_, 'mutex, T>>
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
    /// let holder = Arc::new(Mutex::new(&10));
    /// let mutex = RefMutex::move_mutex(holder);
    ///
    /// let mut guard = mutex.lock().unwrap();
    /// *guard = *guard; // TODO: Better example.
    /// RefMutex::unlock(guard);
    /// ```
    pub fn unlock(guard: RefMutexGuard<'_, '_, T>) {
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
    /// use std::borrow::Borrow;
    ///
    /// let holder = Arc::new(Mutex::new(&10));
    /// let mutex = RefMutex::move_mutex(holder);
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
    /// let holder = Arc::new(Mutex::new(&10));
    /// let mutex = RefMutex::new(&holder);
    /// assert_eq!(**(**mutex.into_inner().unwrap()).lock().unwrap(), 10);
    /// ```
    pub fn into_inner(self) -> LockResult<&'mutex T> // TODO: 'mutex -> '_
    {
        self.base.into_inner()
    }
}

impl<'mutex, T: Copy> RefMutex<'mutex, T> {
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
    /// use std::borrow::Borrow;
    ///
    /// extern crate owning_ref;
    /// let mut holder = Arc::new(Mutex::new(&10)); // TODO: simply 0
    /// let mutex = RefMutex::move_mutex(holder);
    /// *mutex.lock().unwrap() = &20;
    /// assert_eq!(**mutex.lock().unwrap(), 20);
    /// ```
    pub fn get_mut(&mut self) -> LockResult<&'mutex T> { // TODO: 'mutex -> '_
        match self.base.get_mut() {
            Ok(r) => Ok(*r),
            Err(err) => Err(PoisonError::new(*err.into_inner())),
        }
    }
}

impl<'mutex, T: fmt::Debug> fmt::Debug for RefMutex<'mutex, T> {
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

// impl<T> MyDeref for Mutex<SafeRef<'_, T>> {
//     type Target = T;
//     fn deref(&self) -> &T {
//         self.lock()
//     }
// }

// impl<T> Deref for dyn MyDeref<Target = T> {
//     type Target = T;
//     fn deref(&self) -> &T {
//         self.lock()
//     }
// }
