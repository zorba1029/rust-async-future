use crate::data_available::DataAvailable;
// use crate::has_error::HasErrorValue;
use crate::messages::{Message, DeserializeError, SensorMessage};
use std::fmt::Debug;

use futures::stream::Stream;
use serde::Deserialize;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

#[pin_project::pin_project]
pub struct Producer<S, T> {
    #[pin]
    receiver: S,
    post_process_fn: Box<dyn Fn(T) -> T + Send + 'static>,
    #[pin]
    data_available: DataAvailable,
    buffer: Vec<u8>,
}

impl<S, T> Producer<S, T>
where
    S: Stream<Item = Vec<u8>> + Send + 'static,
{
    pub fn new(data_available: DataAvailable, 
                receiver: S, 
                post_process_fn: Box<dyn Fn(T) -> T + 'static + Send>) -> Self {
        Self { 
            data_available,
            receiver, 
            post_process_fn, 
            buffer: Vec::new(),
        }
    }
}

impl<S, T> Stream for Producer<S, T>
where
    S: Stream<Item = Vec<u8>> + Send,
    T: for<'a> Deserialize<'a> + Copy + Debug,
{
    type Item = Message<T>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        match this.data_available.poll_next(cx) {
            Poll::Ready(Some(true)) => {
                match this.receiver.poll_next(cx) {
                    Poll::Ready(Some(data)) => {
                        this.buffer.extend_from_slice(&data);
                        process_buffer::<T>(
                            this.buffer,
                            this.post_process_fn,
                            false,
                            cx.waker(),
                        )
                    },
                    Poll::Ready(None) => {
                        process_buffer::<T>(
                            this.buffer,
                            this.post_process_fn,
                            true,
                            cx.waker(),
                        )
                    },
                    Poll::Pending => {
                        process_buffer::<T>(
                            this.buffer,
                            this.post_process_fn,
                            false,
                            cx.waker(),
                        )
                    }
                }
            },
            // No data available yet
            Poll::Ready(Some(false)) | Poll::Pending => {
                Poll::Pending
            },
            // DataAvailable stream has ended, never happens
            Poll::Ready(None) => {
                Poll::Ready(None)
            }
        }
    }
}

fn process_buffer<T>(buffer: &mut Vec<u8>, 
                    post_process_fn: &mut Box<dyn Fn(T) -> T + Send + 'static>, 
                    stream_closed: bool, 
                    waker: &Waker) -> Poll<Option<Message<T>>>
where 
    T: for<'de> Deserialize<'de> + Copy + Debug,
{
    if buffer.is_empty() && stream_closed {
        return Poll::Ready(None);
    }

    match Message::<T>::from_vec(buffer) {
        Ok(mut message) => {
            if message.is_error() {
                buffer.clear();
                return Poll::Ready(None);
            }

            if message.is_unusable() {
                let length = u32::from_be_bytes(buffer[0..4].try_into().unwrap()) as usize;
                buffer.drain(0..4+length);
                waker.wake_by_ref();
                return Poll::Pending;
            }

            if let Some(value) = message.get_value() {
                let post_processed =  (post_process_fn)(value);
                message.set_value(post_processed);
            } 

            let length = u32::from_be_bytes(buffer[0..4].try_into().unwrap()) as usize;
            buffer.drain(0..4+length);
            return Poll::Ready(Some(message));
        },

        Err(DeserializeError::InsufficientData) => {
            waker.wake_by_ref();
            return Poll::Pending;
        },

        #[cfg(feature = "compression")]
        Err(DeserializeError::DecompressionFailed) => { 
            log::error!("Failed to deserialize the message");
            buffer.clear();
            return Poll::Ready(None);
        },

        Err(DeserializeError::DeserializationFailed) => {
            log::error!("Failed to deserialize the message");
            buffer.clear();
            return Poll::Ready(None);
        }
    }
}
