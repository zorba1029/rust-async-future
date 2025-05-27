use crate::data_available::DataAvailable;
use crate::has_error::HasErrorValue;
use core::pin::Pin;
use futures::Stream;
#[allow(unused)]
use log::{info, trace};
use pin_project::pin_project;
use std::task::{Context, Poll};
use tokio::sync::mpsc;
#[allow(unused)]
use tokio_stream::StreamExt;


#[pin_project]
pub struct Producer<T> 
where 
    T: HasErrorValue,
{
    #[pin]
    // The source of data (e.g., a Tokio mpsc channel)
    receiver: mpsc::Receiver<T>,    
    // A boxed function for post-processing the received data
    post_process_fn: Box<dyn Fn(T) -> T + 'static + Send>,  
    #[pin]
    // Stream controlling when data is available
    data_available: DataAvailable,
}

impl<T> Producer<T>
where
    T: HasErrorValue,
{
    /// Constructor for Producer
    /// Takes a DataAvailable stream, an mpsc receiver, and a post-processing function
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

// Implementing the Stream trait for Producer
// The Producer will produce data items of type `T` after applying the post-processing function
impl<T> Stream for Producer<T>
where
    T: HasErrorValue,
{
    type Item = T;

    /// Polling logic to produce the next item in the stream
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        // First, poll the DataAvailable stream to check if data is ready to be processed
        match this.data_available.poll_next(cx) {
            // Data is AVAILABLE on the channel
            Poll::Ready(Some(true)) => {
                // Data is available on the channel, so we can try to receive it
                match this.receiver.poll_recv(cx) {
                    Poll::Ready(Some(data)) => {
                        if data.is_error_value() {
                            Poll::Ready(None)
                        } else {
                            Poll::Ready(Some((this.post_process_fn)(data)))
                        }
                    }
                    // The mpsc channel is closed, no more data will come
                    Poll::Ready(None) => {
                        Poll::Ready(None)
                    }
                    // No data is available on the channel yet, return Poll::Pending
                    Poll::Pending => {
                        Poll::Pending
                    }
                }
            }
            // DataAvailable indicates data is NOT AVAILABLE YET, return Poll::Pending
            Poll::Ready(Some(false)) => {
                Poll::Pending
            },
            // DataAvailable is still IN PROGRESSs, waiting for the next cycle
            Poll::Pending => {
                Poll::Pending
            }
            // DataAvailable stream HAS ENDED, no more items will be produced
            Poll::Ready(None) => {
                Poll::Ready(None)
            }
        }
    }
}
