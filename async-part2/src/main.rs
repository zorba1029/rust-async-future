#[cfg(feature = "sync")]
use std::{thread::sleep, time::Duration};
#[cfg(not(feature = "sync"))]
use tokio::time::{sleep, Duration};

#[cfg(feature = "sync")]
const MSG: &str = "SYNCHRONOUS";
#[cfg(not(feature = "sync"))]
const MSG: &str = "ASYNC";

// ----------
// Part 2: Async in Poll
// Developing Asynchronous Components in Rust: Part 2 — Async in poll()
// https://medium.com/@alfred.weirich/developing-asynchronous-components-in-rust-part-2-async-in-poll-79edb37188af
// ----------

// ========================================================
// Experiment 1:  Don’t Block the Executor
// ========================================================
// Synchronous Mode (cargo run --features "sync"):
// Asynchronous Mode (cargo run):

// #[tokio::main(flavor = "multi_thread", worker_threads = 100)]
// #[tokio::main(flavor = "multi_thread", worker_threads = 50)]
// #[tokio::main(flavor = "multi_thread", worker_threads = 10)]
#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
async fn main() {
    
    #[allow(unused_variables)]
    let wait = Duration::from_millis(1_000);
    let mut handles = Vec::<tokio::task::JoinHandle<()>>::new();
    
    println!("{MSG} MODE");
    
    let start = std::time::Instant::now();
    for i in 0..100 {
        // println!("{}", i);
        let x = tokio::spawn(async move {
            println!("In Task: {i}, start {MSG} work");
            
            #[cfg(not(feature = "sync"))]
            sleep(wait).await;
            
            #[cfg(feature = "sync")]
            sleep(wait);
            
            println!("--------> In Task: {} after sleep", i);
        });
        handles.push(x);
    }
    
    #[allow(unused_variables)]
    for handle in handles {
        handle.await.unwrap();
    }
    println!("Time elapsed in {MSG} MODE is: {:?}", start.elapsed());
}
    
// ========================================================
// Experiment 2: What Does .async Do?
// ========================================================
// #[tokio::main]
// async fn main() {
//     // tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
//     // println!("Ready");
//     let waker: std::task::Waker = futures::task::noop_waker();
//     let mut cx: Context<'_> = Context::from_waker(&waker);
//     let mut my_sleep: tokio::time::Sleep = tokio::time::sleep(tokio::time::Duration::from_millis(100));
//
//     let mut pinned_sleep: std::pin::Pin<&mut tokio::time::Sleep> = pin!(my_sleep);
//
//     let mut i = 0;
//     loop {
//         i += 1;
//         let sleeper: Poll<()> = pinned_sleep.as_mut().poll(&mut cx);
//         match sleeper {
//             Poll::Pending => {
//                 if i % 100000 == 0 {
//                     println!("Pending: {}", i);
//                 }
//                 cx.waker().wake_by_ref();
//             }
//             Poll::Ready(_) => {
//                 println!("Ready: {}", i);
//                 break;
//             }
//         }
//     }
// }

// Pinning
// https://doc.rust-lang.org/core/pin/macro.pin.html

// With coroutines
// #![feature(coroutines)]
// #![feature(coroutine_trait)]

// use std::ops::{Coroutine, CoroutineState};
// use std::pin::Pin;

// fn coroutine_fn() -> impl Coroutine<Yield = usize, Return = ()> {
//     // Allow coroutine te be self-referential (not 'Unpin')
//     // vvvvvv so that locals can cross yield points.
//     #[coroutine] static || {
//         let foo = String::from("foo");
//         let foo_ref = &foo;
//         yield 0;
//         println!("foo_ref: {}");
//         yield foo.len();
//     }
// }
// fn coroutine_fn() -> impl Coroutine<Yield = usize, Return = ()> {
//     || {
//         let foo = String::from("foo");
//         let foo_ref = &foo;
//         yield 0;
//         println!("foo_ref: {}", foo_ref);
//         yield foo.len();
//     }
// }

// fn main() {
//     let mut coroutine = pin!(coroutine_fn());
//     match coroutine.as_mut().resume() {
//         CoroutineState::Yielded(0) => {},
//         _ => unreachable!(),
//     }
//     match coroutine.as_mut().resume(()) {
//         CoroutineState::Yielded(3) => {}
//         _ => unreachable!(),
//     }
//     match coroutine.resume(()) {
//         CoroutineState::Yielded(_) => unreachable!(),
//         CoroutineState::Complete(()) => {}
//     }
// // }
// fn main() {
//     let mut coroutine = Box::pin(coroutine_fn());
//     match coroutine.as_mut().resume(()) {
//         CoroutineState::Yielded(0) => println!("첫 번째 yield: 0"),
//         _ => unreachable!(),
//     }
//     match coroutine.as_mut().resume(()) {
//         CoroutineState::Yielded(3) => println!("두 번째 yield: 3"),
//         _ => unreachable!(),
//     }
//     match coroutine.as_mut().resume(()) {
//         CoroutineState::Complete(()) => println!("코루틴 완료"),
//         _ => unreachable!(),
//     }
// }