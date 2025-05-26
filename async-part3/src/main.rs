use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use pin_project::pin_project;
use log::{info, trace};
use futures::future::join_all;

//--------------------------------------------------
// Part 3 - Pinning
// Developing Asynchronous Components in Rust: Part 3 — Implementing Futures
// https://medium.com/@alfred.weirich/developing-asynchronous-components-in-rust-part-3-implementing-futures-ab799efffb86
//--------------------------------------------------

/// The 'DataAvailable' structure simulates a future that waits for a specified
/// number of milliseconds.
/// It internally wraps 'tokio::time::Sleep' (an async sleep) and provides
/// a state machine to manage its difference states of execution.

// 5-26-2025 ADDED
#[allow(non_camel_case_types)]
enum DataAvailableState {
    STATE_0,
    STATE_1,
    STATE_2,
}

#[pin_project]
struct DataAvailableFuture {
    seq_no: i32,    // Sequence number for identification
    // state: i32,     // Tracks the state of the function
    state: DataAvailableState,     // Tracks the state of the function
    #[pin]
    sleep_future: Option<tokio::time::Sleep>,  // The future for tokio::sleep, wrapped in Option
    sleep_millis: u64,  // Duration in milliseconds for which the future should wait
}

impl DataAvailableFuture {
    /// Creates a new 'DataAvailableFuture' instance
    /// @param sleep_millis: The number of milliseconds to sleep (delay) before the future resolves
    pub fn new(seq_no: i32, millis: u64) -> Self {
        Self {
            seq_no,         // Sequence number for identification (can be set later)
            // state: 0,       // Initial state
            state: DataAvailableState::STATE_0,       // Initial state
            sleep_future: None,     // Initially, there's no future for Lazy Initialization
            sleep_millis: millis,   // Sleep duration in milliseconds
        }
    }
}   

impl Future for DataAvailableFuture {
    type Output = bool;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // ** Pinned projection of the fields **
        let mut this = self.project();
        loop {
            match *this.state {
                // state 0: Initialize the timer
                DataAvailableState::STATE_0 => {
                    trace!("    |- [{}] DataAvailableFuture - State 0: Init <Sleep Future>", *this.seq_no);
                    if this.sleep_future.is_none() {
                        let sleep = tokio::time::sleep(Duration::from_millis(*this.sleep_millis));
                        this.sleep_future.set(Some(sleep));
                    }
                    *this.state = DataAvailableState::STATE_1;  // Transition to state 1 (waiting)
                    continue;
                }
                // state 1: Wait for the sleep to complete
                DataAvailableState::STATE_1 => {
                    if let Some(sleep_future) = this.sleep_future.as_mut().as_pin_mut() {
                        trace!("    |- [{}] DataAvailableFuture - State 1: -->> POLL <Sleep Future>", *this.seq_no);
                        match sleep_future.poll(cx) {
                            Poll::Ready(()) => {
                                trace!("    |- [{}] DataAvailableFuture - State 1: READY - Sleep completed", *this.seq_no);
                                this.sleep_future.set(None);  // Clear the sleep future (de-initialize the future)
                                *this.state = DataAvailableState::STATE_2;  // Transition to state 2 (data available)
                                continue;
                            }
                            Poll::Pending => {
                                trace!("    |- [{}] DataAvailableFuture - State 1: PENDING - Sleep is pending", *this.seq_no);
                                return Poll::Pending;  // Sleep not yet done, return Pending and stay in waiting state
                            }
                        }   
                    }
                }
                // state 2: Future is Done (Data is available)
                DataAvailableState::STATE_2 => {
                    trace!("    |- [{}] DataAvailableFuture - State 2 [*]: Data is available (Sleep Ready)", *this.seq_no);
                    return Poll::Ready(true);  // Sleep Completed, return true
                }
            }
        }
    }
}

///---------------------------------------------
/// The 'GetDataFuture' structure represents an asynchronous task that waits for the data
/// availability and then returns a fixed result (42).
/// It internally wraps 'DataAvailableFuture' (an async sleep) and provides
/// a state machine to manage its difference states of execution.

// 5-26-2025 ADDED
#[allow(non_camel_case_types)]
enum GetDataState {
    STATE_0,
    STATE_1,
    STATE_2,
}

#[pin_project]
struct GetDataFuture {
    seq_no: i32,            // Sequence number for identification
    state: GetDataState,    // Tracks the current state of the future
    #[pin]
    data_available_future: Option<DataAvailableFuture>,  // Wraps 'DataAvailableFuture' which waits for data
}

impl GetDataFuture {
    pub fn new(seq_no: i32) -> Self {
        Self {
            seq_no,     // Sequence number for identification
            state: GetDataState::STATE_0,   // Initial state of the future
            data_available_future: None,    // Lazy initialization of the data availability future
        }
    }
}

impl Future for GetDataFuture {
    type Output = u64;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Pinned projection of the fields
        let mut this = self.project();
        loop {
            match *this.state {
                // state 0: Initialize the data availability future
                GetDataState::STATE_0 => {
                    trace!("[{}] GetDataFuture - State 0: START =================", *this.seq_no);
                    trace!("[{}] GetDataFuture - State 0: Init <DataAvailableFuture>", *this.seq_no);
                    if this.data_available_future.is_none() {
                        let data_available = DataAvailableFuture::new(*this.seq_no, 5000);
                        this.data_available_future.set(Some(data_available)); // Init the data_available future
                    }
                    *this.state = GetDataState::STATE_1;  // Transition to state 1 (waiting)
                    continue;
                }
                // state 1: Wait for the data availability future to complete
                GetDataState::STATE_1 => {
                    if let Some(data_available_future) = this.data_available_future.as_mut().as_pin_mut() {
                        trace!("[{}] GetDataFuture - State 1: -->> POLL <DataAvailableFuture>", *this.seq_no);
                        match data_available_future.poll(cx) {
                            Poll::Ready(_) => {
                                trace!("[{}] GetDataFuture - State 1: READY - <DataAvailableFuture> completed", *this.seq_no);
                                this.data_available_future.set(None);  // Clear the data availability future
                                *this.state = GetDataState::STATE_2;  // Transition to state 2 (data available)
                                continue;
                            }
                            Poll::Pending => {
                                trace!("[{}] GetDataFuture - State 1: PENDING - <DataAvailableFuture> is pending", *this.seq_no);
                                return Poll::Pending;  // Data not yet available, return Pending and stay in waiting state
                            }
                        }       
                    }
                }
                // state 2: Data is available, return the result
                GetDataState::STATE_2 => {
                    trace!("[{}] GetDataFuture - State 2 [*]: Data is available (return final reult 42)", *this.seq_no);
                    trace!("[{}] GetDataFuture - poll(cx) END ----------------------------", *this.seq_no);
                    return Poll::Ready(42);  // Return the fixed result (42)
                }
            }
        }
    }   
    
}

#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
async fn main() {
    env_logger::init();
    let start = Instant::now();
    let mut tasks = Vec::new();
    for i in 0..3 {
        tasks.push(tokio::spawn(async move {
            let get_data = GetDataFuture::new(i);
            get_data.await
        }));
    }
    // 모든 태스크를 동시에 기다림
    info!("|--------- Starting tasks...");
    let results = join_all(tasks).await;
    let elapsed = start.elapsed();
    info!("It took: {:#?} seconds", elapsed);
    info!("Results: {:?}", results.iter().filter_map(|r| r.as_ref().ok()).collect::<Vec<&u64>>());
}
