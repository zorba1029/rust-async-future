// use core::pin::Pin;
use std::{pin::Pin, time::Duration};
#[allow(unused)]
use log::{info, trace};
use pin_project::pin_project;
#[allow(unused)]
use rand::Rng;
use std::task::{Context, Poll};
use futures::Future;
use futures::Stream;

//--------------------------------------------------
// Part 5 - Tokio Streams
// Developing Asynchronous Components in Rust: Part 5 — Tokio Streams
// https://medium.com/@alfred.weirich/developing-asynchronous-components-in-rust-part-5-tokio-streams-d9ae2973f16e
//--------------------------------------------------

// Enum to represent the two states of DataAvailable
enum State {
    Init,   // State before creating the sleep future
    Sleeping, // State while waiting on the sleep future
}

#[pin_project]
pub struct DataAvailable {
    // Optional sleep future to simulate waiting
    #[pin]
    sleep_future: Option<Pin<Box<tokio::time::Sleep>>>,
    // Internal state tracking (either Init or Sleeping)
    state: State,
}

impl DataAvailable {
    pub fn new() -> Self {
        Self {
            state: State::Init,
            sleep_future: None,
        }
    }

    fn random_delay() -> Duration {
        #[allow(unused_mut)]
        #[allow(unused_variables)]
        let mut rng = rand::rng();
        // let millis = rng.gen_range(0..=1000);
        let millis = rng.random_range(0..=500);
        // let millis = 500;
        Duration::from_millis(millis)
    }
}

// Implement the Stream trait for DataAvailable
// This allows DataAvailable to be used as a Stream that yields `bool` values
impl Stream for DataAvailable {
    type Item = bool;

    // Poll function to determine the next event in the stream
    // It returns 'Poll::Ready(Some(true))' when the sleep is finished and ready to move on
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        // Handle state transitions within the loop
        loop {
            match &mut *this.state {
                // Init state: Create or reset the sleep future and transition to Sleeping
                State::Init => {
                    // Generate a random delay between 0 and 500 milliseconds
                    let delay = Self::random_delay();
                    // If sleep_future exists, reset it with the new delay
                    if let Some(ref mut sleep_future) = *this.sleep_future {
                        sleep_future.as_mut().reset(tokio::time::Instant::now() + delay);
                    } else {
                        // If no sleep_future, create a new one and set it
                        let sleep = tokio::time::sleep(delay);
                        this.sleep_future.set(Some(Box::pin(sleep)));
                    }
                    // Move to the Sleeping state
                    *this.state = State::Sleeping;
                }
                // Sleeping state: Poll the sleep future to check if it has completed
                State::Sleeping => {
                    if let Some(sleep_future) = this.sleep_future.as_mut().as_pin_mut() {
                        match sleep_future.poll(cx) {
                            // It the sleep is ready (finished), transition back to Init
                            Poll::Ready(_) => {
                                *this.state = State::Init;
                                // return true to indicate readiness
                                break Poll::Ready(Some(true));
                            }
                            // If the sleep is not yer ready, return Poll::Pending
                            Poll::Pending => {
                                break Poll::Pending;
                            }
                        }
                    } else {
                        // In case sleep_future is None, return Poll::Pending
                        // This should never happen, but it is a safety check
                        break Poll::Pending;
                    }
                }
            }
        }
    }
}



