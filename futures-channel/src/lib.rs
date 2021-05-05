//! Asynchronous channels.
//!
//! Like threads, concurrent tasks sometimes need to communicate with each
//! other. This module contains two basic abstractions for doing so:
//!
//! - [oneshot], a way of sending a single value from one task to another.
//! - [mpsc], a multi-producer, single-consumer channel for sending values
//!   between tasks, analogous to the similarly-named structure in the standard
//!   library.
//!
//! All items are only available when the `std` or `alloc` feature of this
//! library is activated, and it is activated by default.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms, unreachable_pub)]
// It cannot be included in the published code because this lints have false positives in the minimum required version.
#![cfg_attr(test, warn(single_use_lifetimes))]
#![warn(clippy::all)]
#![doc(test(attr(deny(warnings), allow(dead_code, unused_assignments, unused_variables))))]

macro_rules! cfg_target_has_atomic {
    ($($item:item)*) => {$(
        #[cfg(not(futures_no_atomic_cas))]
        $item
    )*};
}

cfg_target_has_atomic! {
    #[cfg(feature = "alloc")]
    extern crate alloc;

    #[cfg(feature = "alloc")]
    mod lock;
    #[cfg(feature = "std")]
    pub mod mpsc;
    #[cfg(feature = "alloc")]
    pub mod oneshot;
}

#[cfg(futures_loom)]
#[allow(unused_imports)]
mod primitive {
    pub(crate) mod cell {
        pub(crate) use loom::cell::UnsafeCell;
    }
    pub(crate) mod sync {
        pub(crate) mod atomic {
            use core::sync::atomic::Ordering;
            pub(crate) use loom::sync::atomic::AtomicUsize;
            pub(crate) fn fence(ord: Ordering) {
                if let Ordering::Acquire = ord {
                } else {
                    // FIXME: loom only supports acquire fences at the moment.
                    // https://github.com/tokio-rs/loom/issues/117
                    // let's at least not panic...
                    // this may generate some false positives (`SeqCst` is stronger than `Acquire`
                    // for example), and some false negatives (`Relaxed` is weaker than `Acquire`),
                    // but it's the best we can do for the time being.
                }
                loom::sync::atomic::fence(Ordering::Acquire)
            }

            // FIXME: loom does not support compiler_fence at the moment.
            // https://github.com/tokio-rs/loom/issues/117
            // we use fence as a stand-in for compiler_fence for the time being.
            // this may miss some races since fence is stronger than compiler_fence,
            // but it's the best we can do for the time being.
            pub(crate) use self::fence as compiler_fence;
        }
        pub(crate) use loom::sync::{Arc, Mutex, Weak};
    }
    pub(crate) use loom::thread_local;
}
#[cfg(not(futures_loom))]
#[allow(unused_imports, dead_code)]
mod primitive {
    #[cfg(feature = "alloc")]
    pub(crate) mod cell {
        #[derive(Debug)]
        #[repr(transparent)]
        pub(crate) struct UnsafeCell<T>(::core::cell::UnsafeCell<T>);

        // loom's UnsafeCell has a slightly different API than the standard library UnsafeCell.
        // Since we want the rest of the code to be agnostic to whether it's running under loom or
        // not, we write this small wrapper that provides the loom-supported API for the standard
        // library UnsafeCell. This is also what the loom documentation recommends:
        // https://github.com/tokio-rs/loom#handling-loom-api-differences
        impl<T> UnsafeCell<T> {
            #[inline]
            pub(crate) fn new(data: T) -> UnsafeCell<T> {
                UnsafeCell(::core::cell::UnsafeCell::new(data))
            }

            #[inline]
            pub(crate) fn with<R>(&self, f: impl FnOnce(*const T) -> R) -> R {
                f(self.0.get())
            }

            #[inline]
            pub(crate) fn with_mut<R>(&self, f: impl FnOnce(*mut T) -> R) -> R {
                f(self.0.get())
            }
        }
    }
    #[cfg(feature = "alloc")]
    pub(crate) mod sync {
        pub(crate) mod atomic {
            pub(crate) use core::sync::atomic::{
                compiler_fence, fence, AtomicBool, AtomicPtr, AtomicUsize,
            };
        }
        cfg_target_has_atomic! {
            pub(crate) use alloc::sync::{Arc, Weak};
        }
        #[cfg(feature = "std")]
        pub(crate) use std::sync::Mutex;
    }
    #[cfg(feature = "std")]
    pub(crate) use std::thread_local;
}
