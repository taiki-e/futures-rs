use futures::executor::{block_on, block_on_stream};
use futures::future::{ok, err, ready, Future, TryFutureExt};
use futures::stream::{self, iter, empty, poll_fn, Peekable, Stream, StreamExt, TryStreamExt};
use futures::sink::SinkExt;
use futures::channel::{mpsc, oneshot};
use futures_test::task::noop_context;
use std::fmt::Debug;
use std::pin::Pin;
use std::task::{Context, Poll};

fn assert_done<T, F, Fut>(actual_fut: F, expected: T)
where
    T: PartialEq + Debug,
    F: FnOnce() -> Fut,
    Fut: Future<Output = T> + Unpin,
{
    let output = block_on(actual_fut());
    assert_eq!(output, expected);
}

fn sassert_empty<S: Stream + Unpin>(s: &mut S) {
    match poll_next_unpin(&mut noop_context()) {
        Poll::Ready(None) => panic!("stream is at its end"),
        Poll::Ready(Some(_)) => panic!("stream had more elements"),
        Poll::Pending => {}
    }
}

fn list() -> impl Stream<Item = i32> {
    // let (mut tx, rx) = mpsc::channel(1);
    // block_on(tx.send(1)).unwrap();
    // block_on(tx.send(2)).unwrap();
    // block_on(tx.send(3)).unwrap();
    // drop(tx);
    // rx
    iter(1..=3)
}

fn err_list() -> impl Stream<Item = Result<i32, u32>> {
    // let (mut tx, rx) = mpsc::channel(1);
    // block_on(tx.send(Ok(1))).unwrap();
    // block_on(tx.send(Ok(2))).unwrap();
    // block_on(tx.send(Err(3))).unwrap();
    // drop(tx);
    // rx
    iter(vec![Ok(1), Ok(2), Err(3)])
}

#[test]
fn map() {
    assert_done(|| list().map(|a| a + 1).collect(), vec![2, 3, 4]);
}

#[test]
fn map_err() {
    assert_done(|| err_list().map_err(|a| a + 1).try_collect::<Vec<_>>(), Err(4));
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct FromErrTest(u32);

impl From<u32> for FromErrTest {
    fn from(i: u32) -> Self {
        Self(i)
    }
}

#[test]
fn from_err() {
    assert_done(|| err_list().err_into().try_collect::<Vec<_>>(), Err(FromErrTest(3)));
}

#[test]
fn fold() {
    assert_done(|| list().fold(0, |a, b| ready(a + b)), 6);
    assert_done(|| err_list().try_fold(0, |a, b| ok(a + b)), Err(3));
}

#[test]
fn filter() {
    assert_done(|| list().filter(|a| ready(*a % 2 == 0)).collect(), vec![2]);
}

#[test]
fn filter_map() {
    assert_done(|| list().filter_map(|x| {
        ready(if x % 2 == 0 {
            Some(x + 10)
        } else {
            None
        })
    }).collect(), vec![12]);
}

#[test]
fn then() {
    assert_done(|| list().then(|e| ready(e + 1)).collect(), vec![2, 3, 4]);
}

#[test]
fn and_then() {
    assert_done(|| list().map(Ok).and_then(|a| ok(a + 1)).try_collect(), Ok::<_, ()>(vec![2, 3, 4]));
    assert_done(|| list().map(Ok).and_then(|a| err::<i32, u32>(a as u32)).try_collect::<Vec<_>>(),
                Err(1));
}

#[test]
fn or_else() {
    assert_done(|| err_list().or_else(|a| {
        ok::<i32, u32>(a as i32)
    }).try_collect(), Ok(vec![1, 2, 3]));
}

#[test]
fn flatten() {
    assert_done(|| list().map(|_| list()).flatten().collect(),
                vec![1, 2, 3, 1, 2, 3, 1, 2, 3]);

}

#[test]
fn skip() {
    assert_done(|| list().skip(2).collect(), vec![3]);
}

#[test]
fn skip_passes_errors_through() {
    // Note that the behavior is different from 0.1.
    // In 0.1, the following values are returned:
    // ```
    // assert_eq!(s.next(), Some(Err(1)));
    // assert_eq!(s.next(), Some(Err(2)));
    // assert_eq!(s.next(), Some(Ok(4)));
    // assert_eq!(s.next(), Some(Ok(5)));
    // assert_eq!(s.next(), None);
    // ```
    let mut s = block_on_stream(
        iter(vec![Err(1), Err(2), Ok(3), Ok(4), Ok(5)]).skip(1)
    );
    assert_eq!(s.next(), Some(Err(2)));
    assert_eq!(s.next(), Some(Ok(3)));
    assert_eq!(s.next(), Some(Ok(4)));
    assert_eq!(s.next(), Some(Ok(5)));
    assert_eq!(s.next(), None);
}

#[test]
fn skip_while() {
    assert_done(|| list().skip_while(|e| ready(*e % 2 == 1)).collect(),
                vec![2, 3]);
}

#[test]
fn take() {
    assert_done(|| list().take(2).collect(), vec![1, 2]);
}

#[test]
fn take_while() {
    assert_done(|| list().take_while(|e| ready(*e < 3)).collect(),
                vec![1, 2]);
}

#[test]
fn take_passes_errors_through() {
    // Note that the behavior is different from 0.1.
    // In 0.1, the following values are returned:
    // ```
    // assert_eq!(s.next(), Some(Err(1)));
    // assert_eq!(s.next(), Some(Err(2)));
    // assert_eq!(s.next(), Some(Ok(3)));
    // assert_eq!(s.next(), None);
    // ```
    let mut s = block_on_stream(iter(vec![Err(1), Err(2), Ok(3), Ok(4), Err(5)]).take(1));
    assert_eq!(s.next(), Some(Err(1)));
    assert_eq!(s.next(), None);

    let mut s = block_on_stream(iter(vec![Ok(1), Err(2)]).take(1));
    assert_eq!(s.next(), Some(Ok(1)));
    assert_eq!(s.next(), None);
}

#[test]
fn peekable() {
    assert_done(|| list().peekable().collect(), vec![1, 2, 3]);
}

#[test]
fn fuse() {
    let mut stream = block_on_stream(list().fuse());
    assert_eq!(stream.next(), Some(1));
    assert_eq!(stream.next(), Some(2));
    assert_eq!(stream.next(), Some(3));
    assert_eq!(stream.next(), None);
    assert_eq!(stream.next(), None);
    assert_eq!(stream.next(), None);
}

#[test]
fn buffered() {
    let (mut tx, rx) = mpsc::channel(1);
    let (a, b) = oneshot::channel::<u32>();
    let (c, d) = oneshot::channel::<u32>();

    block_on(tx.send(Box::new(b.map_err(|_| panic!())) as Box<Future<Output=_> + Unpin + Send>)).unwrap();
    block_on(tx.send(Box::new(d.map_err(|_| panic!())))).unwrap();

    let mut rx = rx.buffered(2);
    sassert_empty(&mut rx);
    c.send(3).unwrap();
    sassert_empty(&mut rx);
    a.send(5).unwrap();
    let mut rx = block_on_stream(rx);
    assert_eq!(rx.next(), Some(Ok(5)));
    assert_eq!(rx.next(), Some(Ok(3)));
    assert_eq!(rx.next(), None);

    let (mut tx, rx) = mpsc::channel(1);
    let (a, b) = oneshot::channel::<u32>();
    let (c, d) = oneshot::channel::<u32>();

    block_on(tx.send(Box::new(b.map_err(|_| panic!())) as Box<Future<Output=_> + Unpin + Send>)).unwrap();
    block_on(tx.send(Box::new(d.map_err(|_| panic!())))).unwrap();

    let mut rx = rx.buffered(1);
    sassert_empty(&mut rx);
    c.send(3).unwrap();
    sassert_empty(&mut rx);
    a.send(5).unwrap();
    let mut rx = block_on_stream(rx);
    assert_eq!(rx.next(), Some(Ok(5)));
    assert_eq!(rx.next(), Some(Ok(3)));
    assert_eq!(rx.next(), None);
}

#[test]
fn unordered() {
    let (mut tx, rx) = mpsc::channel(1);
    let (a, b) = oneshot::channel::<u32>();
    let (c, d) = oneshot::channel::<u32>();

    block_on(tx.send(Box::new(b.map_err(|_| panic!())) as Box<Future<Output=_> + Unpin + Send>)).unwrap();
    block_on(tx.send(Box::new(d.map_err(|_| panic!())))).unwrap();

    let mut rx = rx.buffer_unordered(2);
    sassert_empty(&mut rx);
    let mut rx = block_on_stream(rx);
    c.send(3).unwrap();
    assert_eq!(rx.next(), Some(Ok(3)));
    a.send(5).unwrap();
    assert_eq!(rx.next(), Some(Ok(5)));
    assert_eq!(rx.next(), None);

    let (mut tx, rx) = mpsc::channel(1);
    let (a, b) = oneshot::channel::<u32>();
    let (c, d) = oneshot::channel::<u32>();

    block_on(tx.send(Box::new(b.map_err(|_| panic!())) as Box<Future<Output=_> + Unpin + Send>)).unwrap();
    block_on(tx.send(Box::new(d.map_err(|_| panic!())))).unwrap();

    // We don't even get to see `c` until `a` completes.
    let mut rx = rx.buffer_unordered(1);
    sassert_empty(&mut rx);
    c.send(3).unwrap();
    sassert_empty(&mut rx);
    a.send(5).unwrap();
    let mut rx = block_on_stream(rx);
    assert_eq!(rx.next(), Some(Ok(5)));
    assert_eq!(rx.next(), Some(Ok(3)));
    assert_eq!(rx.next(), None);
}

#[test]
fn zip() {
    assert_done(|| list().zip(list()).collect(),
                vec![(1, 1), (2, 2), (3, 3)]);
    assert_done(|| list().zip(list().take(2)).collect(),
                vec![(1, 1), (2, 2)]);
    assert_done(|| list().take(2).zip(list()).collect(),
                vec![(1, 1), (2, 2)]);
    assert_done(|| list().zip(list().map(|x| x + 1)).collect(),
                vec![(1, 2), (2, 3), (3, 4)]);
}

#[test]
fn peek() {
    struct Peek<S: Stream<Item = i32> + Unpin> {
        inner: Peekable<S>
    }

    impl<S: Stream<Item = i32> + Unpin> Future for Peek<S> {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            {
                let res = futures::ready!(Pin::new(&mut self.inner).peek(cx));
                assert_eq!(res, Some(&1));
            }
            assert_eq!(futures::ready!(Pin::new(&mut self.inner).peek(cx)), Some(&1));
            assert_eq!(futures::ready!(Pin::new(&mut self.inner).poll_next(cx)), Some(1));
            Poll::Ready(())
        }
    }

    block_on(Peek {
        inner: list().peekable(),
    })
}

#[test]
fn wait() {
    assert_eq!(block_on_stream(list()).collect::<Vec<_>>(), vec![1, 2, 3]);
}

#[test]
fn chunks() {
    assert_done(|| list().chunks(3).collect(), vec![vec![1, 2, 3]]);
    assert_done(|| list().chunks(1).collect(), vec![vec![1], vec![2], vec![3]]);
    assert_done(|| list().chunks(2).collect(), vec![vec![1, 2], vec![3]]);
    let mut list = block_on_stream(err_list().chunks(3));
    let i = list.next().unwrap();
    assert_eq!(i, vec![Ok(1), Ok(2)]);
    let i = list.next().unwrap();
    assert_eq!(i, vec![Err(3)]);
}

#[test]
#[should_panic]
fn chunks_panic_on_cap_zero() {
    let _ = list().chunks(0);
}

#[test]
fn forward() {
    let mut v = Vec::new();
    block_on(iter(vec![Ok(0), Ok(1)]).forward(&mut v)).unwrap();
    assert_eq!(v, vec![0, 1]);

    block_on(iter(vec![Ok(2), Ok(3)]).forward(&mut v)).unwrap();
    assert_eq!(v, vec![0, 1, 2, 3]);

    block_on(iter(vec![Ok(4), Ok(5)]).forward(&mut v)).unwrap();
    assert_eq!(v, vec![0, 1, 2, 3, 4, 5]);
}

#[test]
#[allow(deprecated)]
fn concat() {
    let a = iter(vec![vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9]]);
    assert_done(move || a.concat(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);

    let b = iter(vec![Ok::<_, ()>(vec![1, 2, 3]), Err(()), Ok(vec![7, 8, 9])]);
    assert_done(move || b.try_concat(), Err(()));
}

#[test]
fn concat2() {
    let a = iter(vec![vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9]]);
    assert_done(move || a.concat(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);

    let b = iter(vec![Ok::<_, ()>(vec![1, 2, 3]), Err(()), Ok(vec![7, 8, 9])]);
    assert_done(move || b.try_concat(), Err(()));

    let c = empty::<Vec<()>>();
    assert_done(move || c.concat(), vec![])
}

#[test]
fn stream_poll_fn() {
    let mut counter = 5usize;

    let read_stream = poll_fn(move |_| -> Poll<Option<usize>> {
        if counter == 0 {
            return Poll::Ready(None);
        }
        counter -= 1;
        Poll::Ready(Some(counter))
    });

    assert_eq!(block_on_stream(read_stream).count(), 5);
}

#[test]
fn inspect() {
    let mut seen = vec![];
    assert_done(|| list().inspect(|&a| seen.push(a)).collect(), vec![1, 2, 3]);
    assert_eq!(seen, [1, 2, 3]);
}

#[test]
fn inspect_err() {
    let mut seen = vec![];
    assert_done(|| err_list().inspect_err(|&a| seen.push(a)).collect::<Vec<_>>(), Err(3));
    assert_eq!(seen, [3]);
}

#[test]
fn select() {
    fn select_and_compare(a: Vec<u32>, b: Vec<u32>, expected: Vec<u32>) {
        let a = stream::iter(a);
        let b = stream::iter(b);
        let vec = block_on(stream::select(a, b).collect::<Vec<_>>());
        assert_eq!(vec, expected);
    }

    select_and_compare(vec![1, 2, 3], vec![4, 5, 6], vec![1, 4, 2, 5, 3, 6]);
    select_and_compare(vec![1, 2, 3], vec![4, 5], vec![1, 4, 2, 5, 3]);
    select_and_compare(vec![1, 2], vec![4, 5, 6], vec![1, 4, 2, 5, 6]);
}
