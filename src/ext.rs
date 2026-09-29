#![allow(dead_code, reason = "The types here are not used for all features, but they could be")]

use futures::stream::FusedStream;
use futures::Stream;
use pin_project::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};

#[pin_project]
pub struct StreamWithOnCloseHook<S, F> {
    #[pin]
    stream: S,
    on_close: Option<F>,
}

impl<S, F> StreamWithOnCloseHook<S, F>
where
    S: Stream,
    F: FnOnce(),
{
    pub const fn new(stream: S, on_close: F) -> Self {
        Self { stream, on_close: Some(on_close) }
    }
}

impl<S, F> Stream for StreamWithOnCloseHook<S, F>
where
    S: Stream,
    F: FnOnce(),
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        match this.stream.poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(item)) => Poll::Ready(Some(item)),
            Poll::Ready(None) => {
                if let Some(on_close) = this.on_close.take() {
                    on_close();
                }
                Poll::Ready(None)
            },
        }
    }
}


impl<S, F> FusedStream for StreamWithOnCloseHook<S, F>
where
    S: FusedStream,
    F: FnOnce(),
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}
