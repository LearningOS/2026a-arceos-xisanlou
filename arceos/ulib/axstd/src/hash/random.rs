//! This module exists to isolate [`RandomState`] and [`DefaultHasher`] outside of the
//! [`collections`] module without actually publicly exporting them, so that parts of that
//! implementation can more easily be moved to the [`alloc`] crate.
//!
//! Although its items are public and contain stability attributes, they can't actually be accessed
//! outside this crate.
//!
//! [`collections`]: crate::collections


// ****** START xisanlou add at  250507 No.1
//use crate::cell::Cell;
//use crate::fmt;
//use crate::sys;
use super::{BuildHasher, Hasher, SipHasher};
use core::fmt;
use arceos_api::modules::axhal::misc::random;
// ****** END   xisanlou add at 250507 No.1

/// `RandomState` is the default state for [`HashMap`] types.
///
/// A particular instance `RandomState` will create the same instances of
/// [`Hasher`], but the hashers created by two different `RandomState`
/// instances are unlikely to produce the same result for the same values.
///
/// [`HashMap`]: crate::collections::HashMap
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
/// use std::hash::RandomState;
///
/// let s = RandomState::new();
/// let mut map = HashMap::with_hasher(s);
/// map.insert(1, 2);
/// ```
#[derive(Clone)]
pub struct RandomState {
    k0: u64,
    k1: u64,
}

impl RandomState {
    /// Constructs a new `RandomState` that is initialized with random keys.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::hash::RandomState;
    ///
    /// let s = RandomState::new();
    /// ```
    #[inline]
    #[allow(deprecated)]
    // rand
    #[must_use]
    pub fn new() -> RandomState {
        // ****** START xisanlou add at  250507 No.2
        /*
        thread_local!(static KEYS: Cell<(u64, u64)> = {
            Cell::new(sys::hashmap_random_keys())
        });

        KEYS.with(|keys| {
            let (k0, k1) = keys.get();
            keys.set((k0.wrapping_add(1), k1));
            RandomState { k0, k1 }
        })
        */
        let key = random();
        let k0 = key as u64;
        let k1 = (key >> 64) as u64;
        
        RandomState { k0, k1 }
        // ****** END   xisanlou add at 250507 No.2

    }
}

impl BuildHasher for RandomState {
    type Hasher = DefaultHasher;
    #[inline]
    #[allow(deprecated)]
    fn build_hasher(&self) -> DefaultHasher {
        DefaultHasher(SipHasher::new_with_keys(self.k0, self.k1))
    }
}

/// The default [`Hasher`] used by [`RandomState`].
///
/// The internal algorithm is not specified, and so it and its hashes should
/// not be relied upon over releases.
#[allow(deprecated)]
#[derive(Clone, Debug)]
pub struct DefaultHasher(SipHasher);

impl DefaultHasher {
    /// Creates a new `DefaultHasher`.
    ///
    /// This hasher is not guaranteed to be the same as all other
    /// `DefaultHasher` instances, but is the same as all other `DefaultHasher`
    /// instances created through `new` or `default`.
    #[inline]
    #[allow(deprecated)]
    #[must_use]
    pub fn new() -> DefaultHasher {
        DefaultHasher(SipHasher::new_with_keys(0, 0))
    }
}

impl Default for DefaultHasher {
    /// Creates a new `DefaultHasher` using [`new`].
    /// See its documentation for more.
    ///
    /// [`new`]: DefaultHasher::new
    #[inline]
    fn default() -> DefaultHasher {
        DefaultHasher::new()
    }
}

impl Hasher for DefaultHasher {
    // The underlying `SipHasher` doesn't override the other
    // `write_*` methods, so it's ok not to forward them here.

    #[inline]
    fn write(&mut self, msg: &[u8]) {
        self.0.write(msg)
    }

    //#[inline]
    //fn write_str(&mut self, s: &str) {
    //    self.0.write_str(s);
    //}

    #[inline]
    fn finish(&self) -> u64 {
        self.0.finish()
    }
}

impl Default for RandomState {
    /// Constructs a new `RandomState`.
    #[inline]
    fn default() -> RandomState {
        RandomState::new()
    }
}


impl fmt::Debug for RandomState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RandomState").finish_non_exhaustive()
    }
}
