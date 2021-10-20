## `ref_mutex` Rust library

This library implement `RefMutex<T>` that is similar to `Mutex<T>` but is `Sync` and `Send`
even if `T` isn't `Send`.

TODO: It seems this can be instead be done as a patch to standard library:

```rust
unsafe impl<'mutex, T: ?Sized + Sync> Sync for Mutex<'mutex, T> {}
unsafe impl<'mutex, T: ?Sized> Send for Mutex<'mutex, T> { }
```