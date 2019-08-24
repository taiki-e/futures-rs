use super::BufWriter;
use futures_core::task::{Context, Poll};
use futures_io::AsyncWrite;
use pin_utils::{unsafe_pinned, unsafe_unpinned};
use std::fmt;
use std::io;
use std::mem;
use std::pin::Pin;

/// Wraps a writer and buffers output to it, flushing whenever a newline
/// (`0x0a`, `'\n'`) is detected.
///
/// The [`BufWriter`][bufwriter] struct wraps a writer and buffers its output.
/// But it only does this batched write when it goes out of scope, or when the
/// internal buffer is full. Sometimes, you'd prefer to write each line as it's
/// completed, rather than the entire buffer at once. Enter `LineWriter`. It
/// does exactly that.
///
/// Like [`BufWriter`][bufwriter], a `LineWriter`’s buffer will also be flushed when the
/// `LineWriter` goes out of scope or when its internal buffer is full.
///
/// [bufwriter]: struct.BufWriter.html
///
/// If there's still a partial line in the buffer when the `LineWriter` is
/// dropped, it will flush those contents.
///
/// # Examples
///
/// We can use `LineWriter` to write one line at a time, significantly
/// reducing the number of actual writes to the file.
///
/// ```no_run
/// use std::fs::{self, File};
/// use std::io::prelude::*;
/// use std::io::LineWriter;
///
/// fn main() -> std::io::Result<()> {
///     let road_not_taken = b"I shall be telling this with a sigh
/// Somewhere ages and ages hence:
/// Two roads diverged in a wood, and I -
/// I took the one less traveled by,
/// And that has made all the difference.";
///
///     let file = File::create("poem.txt")?;
///     let mut file = LineWriter::new(file);
///
///     file.write_all(b"I shall be telling this with a sigh")?;
///
///     // No bytes are written until a newline is encountered (or
///     // the internal buffer is filled).
///     assert_eq!(fs::read_to_string("poem.txt")?, "");
///     file.write_all(b"\n")?;
///     assert_eq!(
///         fs::read_to_string("poem.txt")?,
///         "I shall be telling this with a sigh\n",
///     );
///
///     // Write the rest of the poem.
///     file.write_all(b"Somewhere ages and ages hence:
/// Two roads diverged in a wood, and I -
/// I took the one less traveled by,
/// And that has made all the difference.")?;
///
///     // The last line of the poem doesn't end in a newline, so
///     // we have to flush or drop the `LineWriter` to finish
///     // writing.
///     file.flush()?;
///
///     // Confirm the whole poem was written.
///     assert_eq!(fs::read("poem.txt")?, &road_not_taken[..]);
///     Ok(())
/// }
/// ```
pub struct LineWriter<W> {
    inner: BufWriter<W>,
    need_flush: bool,
    written: usize,
}

impl<W: AsyncWrite> LineWriter<W> {
    unsafe_pinned!(inner: BufWriter<W>);
    unsafe_unpinned!(need_flush: bool);
    unsafe_unpinned!(written: usize);

    /// Creates a new `LineWriter`.
    pub fn new(inner: W) -> Self {
        // Lines typically aren't that long, don't use a giant buffer
        Self::with_capacity(1024, inner)
    }

    /// Creates a new `LineWriter` with a specified capacity for the internal
    /// buffer.
    pub fn with_capacity(capacity: usize, inner: W) -> Self {
        Self {
            inner: BufWriter::with_capacity(capacity, inner),
            need_flush: false,
            written: 0,
        }
    }

    /// Gets a reference to the underlying writer.
    pub fn get_ref(&self) -> &W {
        self.inner.get_ref()
    }

    /// Gets a mutable reference to the underlying writer.
    ///
    /// Caution must be taken when calling methods on the mutable reference
    /// returned as extra writes could corrupt the output stream.
    pub fn get_mut(&mut self) -> &mut W {
        self.inner.get_mut()
    }

    /// Gets a pinned mutable reference to the underlying writer.
    ///
    /// Caution must be taken when calling methods on the mutable reference
    /// returned as extra writes could corrupt the output stream.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut W> {
        self.inner().get_pin_mut()
    }

    /// Unwraps this `LineWriter`, returning the underlying writer.
    ///
    /// Note that any leftover data in the internal buffer is lost.
    pub fn into_inner(self) -> W {
        self.inner.into_inner()
    }

    /// Returns a reference to the internally buffered data.
    pub fn buffer(&self) -> &[u8] {
        self.inner.buffer()
    }
}

impl<W: AsyncWrite> AsyncWrite for LineWriter<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let Self {
            inner,
            need_flush,
            written,
        } = unsafe { self.get_unchecked_mut() };
        let mut inner = unsafe { Pin::new_unchecked(inner) };

        if *need_flush {
            ready!(inner.as_mut().poll_flush(cx))?;
        }

        // Find the last newline character in the buffer provided. If found then
        // we're going to write all the data up to that point and then flush,
        // otherwise we just write the whole block to the underlying writer.
        let i = match memchr::memrchr(b'\n', buf) {
            Some(i) => i,
            None => {*written = ready!(inner.poll_write(cx, &buf[*written..])?);
            return Poll::Ready(Ok(mem::replace(written, 0)));
            },
        };

        // Ok, we're going to write a partial amount of the data given first
        // followed by flushing the newline. After we've successfully written
        // some data then we *must* report that we wrote that data, so future
        // errors are ignored. We set our internal `need_flush` flag, though, in
        // case flushing fails and we need to try it first next time.
        dbg!(&written);
        *written += ready!(inner.as_mut().poll_write(cx, &buf[*written..=i]))?;
        *need_flush = true;
        let res = {
            inner
                .as_mut()
                .poll_flush(cx)
                .map(|res| res.map(|()| *need_flush = false))
        };
        dbg!(buf);
        dbg!(&written);
        dbg!(res.is_pending());
        if ready!(res.map(|res| res.is_err())) || *written != i + 1 {
            return Poll::Ready(Ok(mem::replace(written, 0)));
        }

        // At this point we successfully wrote `i + 1` bytes and flushed it out,
        // meaning that the entire line is now flushed out on the screen. While
        // we can attempt to finish writing the rest of the data provided.
        // Remember though that we ignore errors here as we've successfully
        // written data, so we need to report that.
        match inner.as_mut().poll_write(cx, &buf[i + 1..]) {
            Poll::Ready(Ok(i)) => Poll::Ready(Ok(mem::replace(written, 0) + i)),
            Poll::Ready(Err(_)) | Poll::Pending => Poll::Ready(Ok(mem::replace(written, 0))),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        ready!(self.as_mut().inner().poll_flush(cx))?;
        *self.need_flush() = false;
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        ready!(self.as_mut().poll_flush(cx))?;
        self.inner().poll_close(cx)
    }
}

impl<W: AsyncWrite + fmt::Debug> fmt::Debug for LineWriter<W> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("LineWriter")
            .field("writer", &self.inner.inner)
            .field(
                "buffer",
                &format_args!("{}/{}", self.inner.buf.len(), self.inner.buf.capacity()),
            )
            .finish()
    }
}
