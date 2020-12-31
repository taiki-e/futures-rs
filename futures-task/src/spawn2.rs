use crate::{FutureObj, LocalFutureObj};
use core::fmt;
use std::{future::Future, pin::Pin};

/// The `Spawn` trait allows for pushing futures onto an executor that will
/// run them to completion.
pub trait Spawn {
    type Handle;
    /// An error that occurred during spawning.
    type Error;

    /// Spawns a future that will be run to completion.
    ///
    /// # Errors
    ///
    /// The executor may be unable to spawn tasks. Spawn errors should
    /// represent relatively rare scenarios, such as the executor
    /// having been shut down so that it is no longer able to accept
    /// tasks.
    fn spawn_boxed(
        &self,
        future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    ) -> Result<Self::Handle, Self::Error>;

    /// Determines whether the executor is able to spawn new tasks.
    ///
    /// This method will return `Ok` when the executor is *likely*
    /// (but not guaranteed) to accept a subsequent spawn attempt.
    /// Likewise, an `Err` return means that `spawn` is likely, but
    /// not guaranteed, to yield an error.
    #[inline]
    fn status(&self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// The `LocalSpawn` is similar to [`Spawn`], but allows spawning futures
/// that don't implement `Send`.
pub trait LocalSpawn {
    type Handle;
    /// An error that occurred during spawning.
    type Error;

    /// Spawns a future that will be run to completion.
    ///
    /// # Errors
    ///
    /// The executor may be unable to spawn tasks. Spawn errors should
    /// represent relatively rare scenarios, such as the executor
    /// having been shut down so that it is no longer able to accept
    /// tasks.
    fn spawn_local_boxed(
        &self,
        future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    ) -> Result<Self::Handle, Self::Error>;

    /// Determines whether the executor is able to spawn new tasks.
    ///
    /// This method will return `Ok` when the executor is *likely*
    /// (but not guaranteed) to accept a subsequent spawn attempt.
    /// Likewise, an `Err` return means that `spawn` is likely, but
    /// not guaranteed, to yield an error.
    #[inline]
    fn status_local(&self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl<Sp: ?Sized + Spawn> Spawn for &Sp {
    fn spawn_boxed(
        &self,
        future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    ) -> Result<Self::Handle, Self::Error> {
        Sp::spawn_obj(self, future)
    }

    fn status(&self) -> Result<(), Self::Error> {
        Sp::status(self)
    }
}

impl<Sp: ?Sized + Spawn> Spawn for &mut Sp {
    fn spawn_boxed(
        &self,
        future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    ) -> Result<Self::Handle, Self::Error> {
        Sp::spawn_obj(self, future)
    }

    fn status(&self) -> Result<(), Self::Error> {
        Sp::status(self)
    }
}

impl<Sp: ?Sized + LocalSpawn> LocalSpawn for &Sp {
    fn spawn_local_boxed(
        &self,
        future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    ) -> Result<Self::Handle, Self::Error> {
        Sp::spawn_local_boxed(self, future)
    }

    fn status_local(&self) -> Result<(), Self::Error> {
        Sp::status_local(self)
    }
}

impl<Sp: ?Sized + LocalSpawn> LocalSpawn for &mut Sp {
    fn spawn_local_boxed(
        &self,
        future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    ) -> Result<Self::Handle, Self::Error> {
        Sp::spawn_local_boxed(self, future)
    }

    fn status_local(&self) -> Result<(), Self::Error> {
        Sp::status_local(self)
    }
}

#[cfg(feature = "alloc")]
mod if_alloc {
    use super::*;
    use alloc::{boxed::Box, rc::Rc};

    impl<Sp: ?Sized + Spawn> Spawn for Box<Sp> {
        fn spawn_boxed(
            &self,
            future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
        ) -> Result<Self::Handle, Self::Error> {
            (**self).spawn_obj(future)
        }

        fn status(&self) -> Result<(), Self::Error> {
            (**self).status()
        }
    }

    impl<Sp: ?Sized + LocalSpawn> LocalSpawn for Box<Sp> {
        fn spawn_local_boxed(
            &self,
            future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
        ) -> Result<Self::Handle, Self::Error> {
            (**self).spawn_local_boxed(future)
        }

        fn status_local(&self) -> Result<(), Self::Error> {
            (**self).status_local()
        }
    }

    impl<Sp: ?Sized + Spawn> Spawn for Rc<Sp> {
        fn spawn_boxed(
            &self,
            future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
        ) -> Result<Self::Handle, Self::Error> {
            (**self).spawn_obj(future)
        }

        fn status(&self) -> Result<(), Self::Error> {
            (**self).status()
        }
    }

    impl<Sp: ?Sized + LocalSpawn> LocalSpawn for Rc<Sp> {
        fn spawn_local_boxed(
            &self,
            future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
        ) -> Result<Self::Handle, Self::Error> {
            (**self).spawn_local_boxed(future)
        }

        fn status_local(&self) -> Result<(), Self::Error> {
            (**self).status_local()
        }
    }

    cfg_target_has_atomic! {
        use alloc::sync::Arc;

        impl<Sp: ?Sized + Spawn> Spawn for Arc<Sp> {
            fn spawn_boxed(
                &self,
                future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
            ) -> Result<Self::Handle, Self::Error> {
                (**self).spawn_obj(future)
            }

            fn status(&self) -> Result<(), Self::Error> {
                (**self).status()
            }
        }

        impl<Sp: ?Sized + LocalSpawn> LocalSpawn for Arc<Sp> {
            fn spawn_local_boxed(
                &self,
                future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
            ) -> Result<Self::Handle, Self::Error> {
                (**self).spawn_local_boxed(future)
            }

            fn status_local(&self) -> Result<(), Self::Error> {
                (**self).status_local()
            }
        }
    }
}
