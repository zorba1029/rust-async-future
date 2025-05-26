use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use pin_project::pin_project;
use log::{info, trace};
use futures::future::join_all;
use std::task::Waker;
use std::task::{RawWaker, RawWakerVTable};
//--------------------------------------------------
// Part 4 - Waking
// Developing Asynchronous Components in Rust: Part 4 — Waking
// https://medium.com/@alfred.weirich/developing-asynchronous-components-in-rust-part-4-waking-57760d3b630b
//--------------------------------------------------

// async fn data_available(millis: u64) -> bool {
//     tokio::time::sleep(Duration::from_millis(millis)).await;
//     true
// }

/// The 'DataAvailable' structure simulates a future that waits for a specified
/// number of milliseconds.
/// It internally wraps 'tokio::time::Sleep' (an async sleep) and provides
/// a state machine to manage its difference states of execution.
#[pin_project]
struct DataAvailableFuture {
    state: i32,  // Tracks the state of the function
    #[pin]
    sleep_future: Option<tokio::time::Sleep>,  // The future for tokio::sleep, wrapped in Option
    sleep_millis: u64,  // Duration in milliseconds for whiich the future should wait
}

impl DataAvailableFuture {
    /// Creates a new 'DataAvailableFuture' instance
    /// @param sleep_millis: The number of milliseconds to sleep (delay) before the future resolves
    pub fn new(millis: u64) -> Self {
        Self {
            state: 0,   // Initial state
            sleep_future: None,  // Initially, there's no future for Laxy Initialization
            sleep_millis: millis,  // Sleep duration in milliseconds
        }
    }
}   


// 2. Waking Up via a Blocking Task
// impl Future for DataAvailableFuture {
//     type Output = bool;

//     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
//         // Pinned projection of the fields
//         let mut this = self.project();

//         loop {
//             match *this.state {
//                 // state 0: Initialize the timer
//                 0 => {
//                     trace!("    |- DataAvailableFuture - State 0: Initialize sleep function");
//                     if this.sleep_future.is_none() {
//                         let sleep = tokio::time::sleep(Duration::from_millis(*this.sleep_millis));
//                         this.sleep_future.set(Some(sleep));
//                     }
//                     *this.state = 1;  // Transition to state 1 (waiting)
//                     // start a polling task to wake us
//                     // 🔻🔻🔻🔻🔻🔻
//                     let waker = cx.waker().clone();
//                     let _ = tokio::task::spawn_blocking(move || {
//                         std::thread::sleep(std::time::Duration::from_millis(7000));
//                         info!("🔺🔺🔺🔺🔺🔺 Now the blocking tasks wakes us");
//                         waker.wake_by_ref();
//                     });
//                     // 🔺🔺🔺🔺🔺🔺
//                     continue;
//                 }
//                 // state 1: Wait for the sleep to complete
//                 1 => {
//                     if let Some(sleep_future) = this.sleep_future.as_mut().as_pin_mut() {
//                         trace!("    |- DataAvailableFuture - State 1: -->> sleep_future.poll(cx)");
//                         // Create a noop-waker and a context
//                         // 🔻🔻🔻🔻🔻🔻
//                         let waker =  futures::task::noop_waker();
//                         let mut sleep_cx = Context::from_waker(&waker);
//                         //                      🔻🔻🔻🔻🔻🔻
//                         match sleep_future.poll(&mut sleep_cx) {
//                         // 🔺🔺🔺🔺🔺🔺
//                             Poll::Ready(()) => {
//                                 trace!("    |- DataAvailableFuture - State 1: Sleep completed");
//                                 this.sleep_future.set(None);  // Clear the sleep future (de-initialize the future)
//                                 *this.state = 2;  // Transition to state 2 (data available)
//                                 continue;
//                             }
//                             Poll::Pending => {
//                                 trace!("    |- DataAvailableFuture - State 1: Sleep is pending");
//                                 return Poll::Pending;  // Sleep not yet done, return Pending and stay in waiting state
//                             }
//                         }   
//                     }
//                 }
//                 // state 2: Future is Done (Data is available
//                 2 => {
//                     trace!("    |- DataAvailableFuture - State 2 [*]: Data is available (Sleep Ready)");
//                     return Poll::Ready(true);  // Sleep Completed, return true
//                 }
//                 _ => {
//                     panic!("    |- DataAvailableFuture - Invalid state");
//                 }
//             }
//         }
//     }
// }

//--------------------------------------------------
// Custom waker
//--------------------------------------------------
// Struct to hold the original waker
struct CustomSleepWaker {
    original_waker: Waker,
}

// Custom Waker creation function using the inner waker
fn custom_sleep_waker(original_waker: Waker) -> Waker {
    let custom_sleep_waker = CustomSleepWaker { original_waker };

    // Create a RawWaker
    unsafe {
        Waker::from_raw(RawWaker::new(
            Box::into_raw(Box::new(custom_sleep_waker)) as *const (),
            &RAW_WAKER_VTABLE,
        ))
    }
}

// RawWakerVTable 
const RAW_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    custom_waker_clone,
    custom_waker_wake,
    custom_waker_wake_by_ref,
    custom_waker_drop,
);

unsafe fn custom_waker_clone(ptr: *const ()) -> RawWaker {
    let my_waker = &*(ptr as *const CustomSleepWaker);
    let inner_waker = my_waker.original_waker.clone();
    RawWaker::new(
        Box::into_raw(Box::new(CustomSleepWaker { 
            original_waker: inner_waker 
        })) as *const (), 
        &RAW_WAKER_VTABLE,
    )
}

unsafe fn custom_waker_wake(ptr: *const ()) {
    // Cast the pointer to a mutable one for Box::from_raw()
    let my_waker = Box::from_raw(ptr as *mut CustomSleepWaker);
    info!("CustomSleepWaker: wake() called");
    my_waker.original_waker.wake();  // Delegate to the wake-up to the inner waker
}

unsafe fn custom_waker_wake_by_ref(ptr: *const ()) {
    let my_waker = &*(ptr as *const CustomSleepWaker);
    info!("CustomSleepWaker: wake_by_ref() called");
    my_waker.original_waker.wake_by_ref(); // Delegate to the wake-up to the inner waker    
}

unsafe fn custom_waker_drop(ptr: *const ()) {
    // Cast the pointer to a mutable one for Box::from_raw()
    // This will automatically drop the the box and the inner waker
    let _ = Box::from_raw(ptr as *mut CustomSleepWaker);
}


//--------------------------------------------------    
// modified to use custom waker
//--------------------------------------------------
impl Future for DataAvailableFuture {
    type Output = bool;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Pinned projection of the fields
        let mut this = self.project();

        loop {
            match *this.state {
                // state 0: Initialize the timer
                0 => {
                    trace!("    |- DataAvailableFuture - State 0: Initialize sleep function");
                    if this.sleep_future.is_none() {
                        let sleep = tokio::time::sleep(Duration::from_millis(*this.sleep_millis));
                        this.sleep_future.set(Some(sleep));
                    }
                    *this.state = 1;  // Transition to state 1 (waiting)
                    continue;
                }
                // state 1: Wait for the sleep to complete
                1 => {
                    if let Some(sleep_future) = this.sleep_future.as_mut().as_pin_mut() {
                        trace!("    |- DataAvailableFuture - State 1: -->> sleep_future.poll(cx)");
                        // Create a custom sleep waker
                        // 🔻🔻🔻🔻🔻🔻
                        let sleep_waker = cx.waker().clone();
                        let custom_sleep_waker = custom_sleep_waker(sleep_waker);
                        let mut sleep_cx = Context::from_waker(&custom_sleep_waker);
                        // let waker =  futures::task::noop_waker();
                        // let mut sleep_cx = Context::from_waker(&waker);
                        match sleep_future.poll(&mut sleep_cx) {
                        // 🔺🔺🔺🔺🔺🔺
                            Poll::Ready(()) => {
                                trace!("    |- DataAvailableFuture - State 1: Sleep completed");
                                this.sleep_future.set(None);  // Clear the sleep future (de-initialize the future)
                                *this.state = 2;  // Transition to state 2 (data available)
                                continue;
                            }
                            Poll::Pending => {
                                trace!("    |- DataAvailableFuture - State 1: Sleep is pending");
                                return Poll::Pending;  // Sleep not yet done, return Pending and stay in waiting state
                            }
                        }   
                    }
                }
                // state 2: Future is Done (Data is available
                2 => {
                    trace!("    |- DataAvailableFuture - State 2 [*]: Data is available (Sleep Ready)");
                    return Poll::Ready(true);  // Sleep Completed, return true
                }
                _ => {
                    panic!("    |- DataAvailableFuture - Invalid state");
                }
            }
        }
    }
}

///---------------------------------------------
/// 
// async fn get_data() -> u64 {
//     data_available(3000).await;
//     42
// }

/// The 'GetDataFuture' structure represents an asynchronous task that waits for the data
/// availability and then returns a fixed result (42).
#[pin_project]
struct GetDataFuture {
    state: i32,  // Tracks the current state of the future
    #[pin]
    data_available_future: Option<DataAvailableFuture>,  // Wraps 'DataAvailableFuture' which waits for data
}

impl GetDataFuture {
    pub fn new() -> Self {
        Self {
            state: 0,  // Initial state of the future
            data_available_future: None,  // Lazy initialization of the data availability future
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
                //-- state 0: Initialize the data availability future
                0 => {
                    trace!("GetDataFuture       - State 0: START =================");
                    trace!("GetDataFuture       - State 0: Initialize data availability future");
                    if this.data_available_future.is_none() {
                        let data_available = DataAvailableFuture::new(3000);
                        this.data_available_future.set(Some(data_available)); // Init the data_available future
                    }
                    *this.state = 1;  // Transition to state 1 (waiting)
                    continue;
                }
                //-- state 1: Wait for the data availability future to complete
                1 => {
                    if let Some(data_available_future) = this.data_available_future.as_mut().as_pin_mut() {
                        trace!("GetDataFuture       - State 1: -->> poll(cx)");
                        match data_available_future.poll(cx) {
                            Poll::Ready(_) => {
                                trace!("GetDataFuture       - State 1: data_available complete");
                                this.data_available_future.set(None);  // Clear the data availability future
                                *this.state = 2;  // Transition to state 2 (data available)
                                continue;
                            }
                            Poll::Pending => {
                                trace!("GetDataFuture       - State 1: data_available pending");
                                return Poll::Pending;  // Data not yet available, return Pending and stay in waiting state
                            }
                        }       
                    }
                }
                // state 2: Data is available, return the result
                2 => {
                    trace!("GetDataFuture       - State 2 [*]: Data is available (return final reult 42)");
                    trace!("GetDataFuture       - poll(cx) END ----------------------------");
                    return Poll::Ready(42);  // Return the fixed result (42)
                }
                _ => {
                    panic!("GetDataFuture      -Invalid state");
                }
            }
        }
    }   
}


//--------------------------------------------------
// Main
//--------------------------------------------------

#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
async fn main() {
    // let result: u64 = get_data().await;
    // println!("Result: {}", result);
    env_logger::init();
    let start = Instant::now();
    let mut tasks = Vec::new();
    for _i in 0..1 {
        tasks.push(tokio::spawn(async move {
            let get_data = GetDataFuture::new();
            get_data.await
        }));
    }
    // 모든 태스크를 동시에 기다림
    let results = join_all(tasks).await;
    let elapsed = start.elapsed();
    info!("It took: {:#?} seconds", elapsed);
    info!("Results: {:?}", results.iter().filter_map(|r| r.as_ref().ok()).collect::<Vec<&u64>>());
}
