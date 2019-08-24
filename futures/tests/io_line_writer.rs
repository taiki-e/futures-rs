use futures::executor::block_on;
use futures::future::{Future, FutureExt};
use futures::io::{AsyncWrite, AsyncWriteExt, LineWriter};
use futures::task::{Context, Poll};
use futures_test::task::noop_context;
use std::io;
use std::pin::Pin;

#[test]
fn test_line_buffer() {
    let mut writer = LineWriter::new(Vec::new());
    block_on(writer.write(&[0])).unwrap();
    assert_eq!(*writer.get_ref(), []);
    block_on(writer.write(&[1])).unwrap();
    assert_eq!(*writer.get_ref(), []);
    block_on(writer.flush()).unwrap();
    assert_eq!(*writer.get_ref(), [0, 1]);
    block_on(writer.write(&[0, b'\n', 1, b'\n', 2])).unwrap();
    assert_eq!(*writer.get_ref(), [0, 1, 0, b'\n', 1, b'\n']);
    block_on(writer.flush()).unwrap();
    assert_eq!(*writer.get_ref(), [0, 1, 0, b'\n', 1, b'\n', 2]);
    block_on(writer.write(&[3, b'\n'])).unwrap();
    assert_eq!(*writer.get_ref(), [0, 1, 0, b'\n', 1, b'\n', 2, 3, b'\n']);
}

// https://github.com/rust-lang/rust/issues/32085
#[test]
fn test_line_buffer_fail_flush() {
    struct FailFlushWriter<'a>(&'a mut Vec<u8>);

    impl AsyncWrite for FailFlushWriter<'_> {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            self.0.extend_from_slice(buf);
            Poll::Ready(Ok(buf.len()))
        }
        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, "flush failed")))
        }
        fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            unimplemented!()
        }
    }

    let mut buf = Vec::new();
    {
        let mut writer = LineWriter::new(FailFlushWriter(&mut buf));
        let to_write = b"abc\ndef";
        if let Ok(written) = block_on(writer.write(to_write)) {
            assert!(written < to_write.len(), "didn't flush on new line");
            // PASS
            return;
        }
    }
    assert!(buf.is_empty(), "write returned an error but wrote data");
}

struct MaybePending {
    inner: Vec<u8>,
    ready: bool,
}

impl MaybePending {
    fn new(inner: Vec<u8>) -> Self {
        Self { inner, ready: false }
    }
}

impl AsyncWrite for MaybePending {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.ready {
            self.ready = false;
            Pin::new(&mut self.inner).poll_write(cx, buf)
        } else {
            self.ready = true;
            Poll::Pending
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_close(cx)
    }
}

fn run<F: Future + Unpin>(mut f: F) -> F::Output {
    let mut cx = noop_context();
    loop {
        if let Poll::Ready(x) = f.poll_unpin(&mut cx) {
            return x;
        }
    }
}

#[test]
fn maybe_pending_line_buffer() {
    let mut writer = LineWriter::new(MaybePending::new(Vec::new()));
    run(writer.write(&[0])).unwrap();
    assert_eq!(writer.get_ref().inner, []);
    run(writer.write(&[1])).unwrap();
    assert_eq!(writer.get_ref().inner, []);
    run(writer.flush()).unwrap();
    assert_eq!(writer.get_ref().inner, [0, 1]);
    run(writer.write(&[0, b'\n', 1, b'\n', 2])).unwrap();
    assert_eq!(writer.get_ref().inner, [0, 1, 0, b'\n', 1, b'\n']);
    run(writer.flush()).unwrap();
    assert_eq!(writer.get_ref().inner, [0, 1, 0, b'\n', 1, b'\n', 2]);
    run(writer.write(&[3, b'\n'])).unwrap();
    assert_eq!(writer.get_ref().inner, [0, 1, 0, b'\n', 1, b'\n', 2, 3, b'\n']);
}
