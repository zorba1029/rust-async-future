use crate::data_available::DataAvailable;
use crate::has_error::HasErrorValue;
use core::pin::Pin;
use futures::Stream;
#[allow(unused)]
use log::{info, trace};
use pin_project::pin_project;
use std::task::{Context, Poll};
use tokio::sync::mpsc;
// use tokio_stream::StreamExt;

#[pin_project]
pub struct Producer<T> 
where 
    T: HasErrorValue,
{
    #[pin]
    receiver: mpsc::Receiver<T>,
    post_process_fn: Box<dyn Fn(T) -> T + 'static + Send>,
    #[pin]
    data_available: DataAvailable,
}

impl<T> Producer<T>
where
    T: HasErrorValue,
{
    pub fn new(data_available: DataAvailable, 
                receiver: mpsc::Receiver<T>, 
                post_process_fn: Box<dyn Fn(T) -> T + 'static + Send>) -> Self {
        Self { 
            data_available,
            receiver, 
            post_process_fn, 
        }
    }
}

impl<T> Stream for Producer<T>
where
    T: HasErrorValue,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        match this.data_available.poll_next(cx) {
            Poll::Ready(Some(true)) => {
                match this.receiver.poll_recv(cx) {
                    Poll::Ready(Some(data)) => {
                        if data.is_error_value() {
                            Poll::Ready(None)
                        } else {
                            Poll::Ready(Some((this.post_process_fn)(data)))
                        }
                    }
                    Poll::Ready(None) => {
                        Poll::Ready(None)
                    }
                    Poll::Pending => {
                        Poll::Pending
                    }
                }
            }
            Poll::Ready(Some(false)) => {
                Poll::Pending
            },
            Poll::Pending => {
                Poll::Pending
            }
            Poll::Ready(None) => {
                Poll::Ready(None)
            }
        }
    }
}
