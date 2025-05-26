use core::pin::pin;
use futures::Future;
#[allow(unused)]
use log::{info, trace};
use pin_project::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio_stream::Stream;

#[pin_project]
pub struct Collector<T, P> 
where
    P: Stream<Item = T>,
{
    #[pin]
    producer: P,
    collected_data: Vec<T>,
}

impl<T, P> Collector<T, P>
where
    P: Stream<Item = T>,
{
    pub fn new(producer: P) -> Self {
        Self { 
            producer, 
            collected_data: Vec::new(),
        }
    }
}

impl<T, P> Future for Collector<T, P> 
where
    P: Stream<Item = T> + Unpin,
{
    type Output = Vec<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        // Continuously poll the producer stream for new items
        loop {
            match this.producer.as_mut().poll_next(cx) {
                Poll::Ready(Some(item)) => {
                    this.collected_data.push(item);
                }
                Poll::Ready(None) => {
                    return Poll::Ready(std::mem::take(this.collected_data));
                }
                Poll::Pending => {
                    return Poll::Pending;
                }
            }
        }
    }
}