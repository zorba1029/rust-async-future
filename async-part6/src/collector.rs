use core::pin::pin;
use std::collections::VecDeque;
// use futures::Future;
#[allow(unused)]
use log::{info, trace};
use pin_project::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio_stream::Stream;

#[pin_project]
pub struct CollectorDyn<T>
{
    #[pin]
    producers: Vec<Box<dyn Stream<Item = T> + Unpin + Send>>,
    collected_data: Vec<T>,
}

impl<T> CollectorDyn<T>
{
    #[allow(unused)]
    pub fn new(producers: Vec<Box<dyn Stream<Item = T> + Unpin + Send>>) -> Self {
        Self { 
            producers, 
            collected_data: Vec::new(),
        }
    }
}

impl<T> futures::Future for CollectorDyn<T> 
where
    T: Unpin,
{
    type Output = Vec<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        let mut to_remove = VecDeque::new();
        let mut made_progress = false;

        for (i, producer) in this.producers.iter_mut().enumerate() {
            let mut producer = Pin::new(producer.as_mut());

            match producer.as_mut().poll_next(cx) {
                Poll::Ready(Some(item)) => {
                    this.collected_data.push(item);
                    made_progress = true;
                }
                Poll::Ready(None) => {
                    to_remove.push_back(i);
                }
                Poll::Pending => {

                }
            }
        }

        // Remove completed streams
        while let Some(i) = to_remove.pop_back() {
            let _ = this.producers.remove(i);
        }

        if this.producers.is_empty() {
            return Poll::Ready(std::mem::take(this.collected_data));
        } else if made_progress {
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Pending
        }
    }
}