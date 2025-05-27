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
    // The stream producer from which data is being collected
    producer: P,
    // Vector to store the collected data
    collected_data: Vec<T>,
}

impl<T, P> Collector<T, P>
where
    P: Stream<Item = T>,
{
    // Constructor for Collector
    // Takes a stream producer as input and initializes an empty collection vector
    // @param producer: The stream producer from which data is being collected
    pub fn new(producer: P) -> Self {
        Self { 
            producer, 
            collected_data: Vec::new(),
        }
    }
}

// Implement the Future trait for Collector
// This allows the Collector to behave like a future, collecting data from the producer
// and returning the collected data once the stream is complete.
impl<T, P> Future for Collector<T, P> 
where
    P: Stream<Item = T> + Unpin,
{
    // The final output is a Vec<T> containing all collected items
    type Output = Vec<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        // Continuously poll the producer stream for new items
        loop {
            match this.producer.as_mut().poll_next(cx) {
                // If the producer yields an item, add it to the collected_data vector
                Poll::Ready(Some(item)) => {
                    this.collected_data.push(item);
                }
                // If the producer stream has finished (returns None), return the collected data
                Poll::Ready(None) => {
                    // Return the vector with all the collected items
                    // use std::mem::take to efficiently take ownership of the collected data 
                    // while leaving an empty Vec in its place.
                    return Poll::Ready(std::mem::take(this.collected_data));
                }
                // If the producer stream is not ready yet, return Poll::Pending
                Poll::Pending => {
                    return Poll::Pending;
                }
            }
        }
    }
}