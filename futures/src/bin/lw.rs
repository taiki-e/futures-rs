use futures::future::{Future, FutureExt};
use futures::io::{AsyncWrite, AsyncWriteExt, LineWriter};
use futures::task::{Context, Poll};
use futures_test::task::noop_context;
use futures_test::io::AsyncWriteTestExt;

fn run<F: Future + Unpin>(mut f: F) -> F::Output {
    let mut cx = noop_context();
    loop {
        if let Poll::Ready(x) = f.poll_unpin(&mut cx) {
            return x;
        }
    }
}

fn main() {
    let mut writer = LineWriter::new(Vec::new().interleave_pending_write());
    run(writer.write(&[0])).unwrap();
    assert_eq!(*writer.get_ref().get_ref(), []);
    run(writer.write(&[1])).unwrap();
    assert_eq!(*writer.get_ref().get_ref(), []);
    run(writer.flush()).unwrap();
    assert_eq!(*writer.get_ref().get_ref(), [0, 1]);
    run(writer.write(&[0, b'\n', 1, b'\n', 2])).unwrap();
    assert_eq!(*writer.get_ref().get_ref(), [0, 1, 0, b'\n', 1, b'\n']);
    // run(writer.flush()).unwrap();
    // assert_eq!(*writer.get_ref().get_ref(), [0, 1, 0, b'\n', 1, b'\n', 2]);
    // run(writer.write(&[3, b'\n'])).unwrap();
    // assert_eq!(*writer.get_ref().get_ref(), [0, 1, 0, b'\n', 1, b'\n', 2, 3, b'\n']);
}
