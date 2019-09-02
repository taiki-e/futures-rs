#![feature(test)]
#![cfg(feature = "bilock")]

extern crate test;

use futures::executor::LocalPool;
use futures::future::FutureExt;
use futures::lock::{BiLock, BiLockAcquire, BiLockGuard};
use futures::stream::{Stream, StreamExt};
use futures::task::{ArcWake, Context, Waker};
use futures_test::task::noop_context;
use std::mem;
use std::sync::Arc;
use test::Bencher;

/// Pseudo-stream which simply calls `lock.poll()` on `poll`
struct LockStream<'a> {
    lock: BiLockAcquire<'a, u32>,
}

impl<'a> LockStream<'a> {
    fn new(lock: &'a BiLock<u32>) -> Self {
        Self {
            lock: lock.lock()
        }
    }

    /// Release a lock after it was acquired in `poll`,
    /// so `poll` could be called again.
    fn release_lock(&mut self, guard: BiLockGuard<'a, u32>) {
        // guard.bilock.unlock();
        // self.lock = guard.bilock.lock();
        // mem::forget(guard)
    }
}

impl<'a> Stream for LockStream<'a> {
    type Item = BiLockGuard<'a, u32>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.lock.poll_unpin(cx).map(Some)
    }
}

#[bench]
fn contended(b: &mut Bencher) {
    let cx = &mut noop_context();

    b.iter(|| {
        let (x, y) = BiLock::new(1);

        let mut x_lock = LockStream::new(&x);
        let mut y_lock = LockStream::new(&y);

        for _ in 0..1000 {
            let x_guard = match x_lock.poll_next_unpin(cx) {
                Poll::Ready(Some(guard)) => guard,
                _ => panic!(),
            };

            // Try poll second lock while first lock still holds the lock
            match y_lock.poll_next_unpin(cx) {
                Poll::Pending => (),
                _ => panic!(),
            };

            x_lock.release_lock(x_guard);

            let y_guard = match y_lock.poll_next_unpin(cx) {
                Poll::Ready(Some(guard)) => guard,
                _ => panic!(),
            };

            y_lock.release_lock(y_guard);
        }
        (x, y)
    });
}

#[bench]
fn lock_unlock(b: &mut Bencher) {
    let cx = &mut noop_context();

    b.iter(|| {
        let (x, y) = BiLock::new(1);

        let mut x_lock = LockStream::new(&x);
        let mut y_lock = LockStream::new(&y);

        for _ in 0..1000 {
            let x_guard = match x_lock.poll_next_unpin(cx) {
                Poll::Ready(Some(guard)) => guard,
                _ => panic!(),
            };

            x_lock.release_lock(x_guard);

            let y_guard = match y_lock.poll_next_unpin(cx) {
                Poll::Ready(Some(guard)) => guard,
                _ => panic!(),
            };

            y_lock.release_lock(y_guard);
        }
        (x, y)
    })
}
