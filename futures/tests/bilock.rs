#![cfg(feature = "bilock")]

use futures::executor::block_on;
use futures::future::{self, FutureExt};
use futures::lock::BiLock;
use futures::stream::{self, StreamExt};
use futures::task;
use futures_test::task::noop_context;
use std::thread;

#[test]
fn smoke() {
    let mut future = future::lazy(|cx| {
        let (a, b) = BiLock::new(1);

        {
            let mut lock = match a.poll_lock(cx) {
                Poll::Ready(l) => l,
                Poll::Pending => panic!("poll not ready"),
            };
            assert_eq!(*lock, 1);
            *lock = 2;

            assert!(b.poll_lock(cx).is_pending());
            assert!(a.poll_lock(cx).is_pending());
        }

        assert!(b.poll_lock(cx).is_ready());
        assert!(a.poll_lock(cx).is_ready());

        {
            let lock = match b.poll_lock(cx) {
                Poll::Ready(l) => l,
                Poll::Pending => panic!("poll not ready"),
            };
            assert_eq!(*lock, 2);
        }

        assert_eq!(a.reunite(b).expect("bilock/smoke: reunite error"), 2);
    });

    let cx = &mut noop_context();
    assert!(future.poll_unpin(cx).is_ready());
}

#[test]
fn concurrent() {
    const N: usize = 10000;
    let cx = &mut noop_context();
    let (a, b) = BiLock::new(0);

    let a = Increment {
        a: Some(a),
        remaining: N,
    };
    let b2 = stream::iter(0..N).for_each(|_n| {
        b.lock().map(|mut b| {
            *b += 1;
        })
    });

    let t1 = thread::spawn(move || block_on(a));
    block_on(b2);
    let a = t1.join().unwrap();

    match a.poll_lock(cx) {
        Poll::Ready(l) => assert_eq!(*l, 2 * N),
        Poll::Pending => panic!("poll not ready"),
    }
    match b.poll_lock(cx) {
        Poll::Ready(l) => assert_eq!(*l, 2 * N),
        Poll::Pending => panic!("poll not ready"),
    }

    assert_eq!(a.reunite(b).expect("bilock/concurrent: reunite error"), 2 * N);

    struct Increment {
        remaining: usize,
        a: Option<BiLock<usize>>,
    }

    impl Future for Increment {
        type Output = BiLock<usize>;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let Self { remaining, a } = self.get_mut();
            loop {
                if *remaining == 0 {
                    return Poll::Ready(a.take().unwrap());
                }

                let a = a.as_ref().unwrap();
                let mut a = ready!(a.poll_lock(cx));
                *remaining -= 1;
                *a += 1;
            }
        }
    }
}
