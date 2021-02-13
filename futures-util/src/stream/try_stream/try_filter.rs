use crate::stream::{Stream, TryStream, FusedStream};
use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::ready;
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`try_filter`](super::TryStreamExt::try_filter)
    /// method.
    #[must_use = "streams do nothing unless polled"]
    pub struct TryFilter<St, Fut, F>
        where St: TryStream
    {
        #[pin]
        stream: St,
        f: F,
        #[pin]
        pending_fut: Option<Fut>,
        pending_item: Option<St::Ok>,
    }
}

impl<St, Fut, F> fmt::Debug for TryFilter<St, Fut, F>
where
    St: TryStream + fmt::Debug,
    St::Ok: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TryFilter")
            .field("stream", &self.stream)
            .field("pending_fut", &self.pending_fut)
            .field("pending_item", &self.pending_item)
            .finish()
    }
}

impl<St, Fut, F> TryFilter<St, Fut, F>
    where St: TryStream
{
    pub(super) fn new(stream: St, f: F) -> Self {
        Self {
            stream,
            f,
            pending_fut: None,
            pending_item: None,
        }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St, Fut, F> FusedStream for TryFilter<St, Fut, F>
    where St: TryStream + FusedStream,
          F: FnMut(&St::Ok) -> Fut,
          Fut: Future<Output = bool>,
{
    fn is_terminated(&self) -> bool {
        self.pending_fut.is_none() && self.stream.is_terminated()
    }
}

impl<St, Fut, F> Stream for TryFilter<St, Fut, F>
    where St: TryStream,
          Fut: Future<Output = bool>,
          F: FnMut(&St::Ok) -> Fut,
{
    type Item = Result<St::Ok, St::Error>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        Poll::Ready(loop {
            if let Some(fut) = this.pending_fut.as_mut().as_pin_mut() {
                let res = ready!(fut.poll(cx));
                this.pending_fut.set(None);
                if res {
                    break this.pending_item.take().map(Ok);
                }
                *this.pending_item = None;
            } else if let Some(item) = ready!(this.stream.as_mut().try_poll_next(cx)?) {
                this.pending_fut.set(Some((this.f)(&item)));
                *this.pending_item = Some(item);
            } else {
                break None;
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let pending_len = if self.pending_fut.is_some() { 1 } else { 0 };
        let (_, upper) = self.stream.size_hint();
        let upper = match upper {
            Some(x) => x.checked_add(pending_len),
            None => None,
        };
        (0, upper) // can't know a lower bound, due to the predicate
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Fut, F, Item, E> Sink<Item> for TryFilter<S, Fut, F>
    where S: TryStream + Sink<Item, Error = E>,
{
    type Error = E;

    delegate_sink!(stream, Item);
}
