## `ref_mutex` Rust library

Please instead use `Mutex<send_cell::Ref<T>>` (https://docs.rs/send-cell/).

---

**My code is erroneous!** Don't use.

This library implement `RefMutex<T>` that is similar to `Mutex<T>` but is `Sync` and `Send`
even if `T` isn't `Send`.