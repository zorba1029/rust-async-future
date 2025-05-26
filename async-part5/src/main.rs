use futures::future::join_all;
// use core::pin::Pin;
#[allow(unused)]
use log::{info, trace};
#[allow(unused)]
use std::pin::Pin;
use std::time::Instant;
// use futures::StreamExt;
use tokio_stream::StreamExt;
use tokio::sync::mpsc;

mod data_available;
mod producer;
mod has_error;
mod collector;

use data_available::DataAvailable;
use producer::Producer;
use has_error::HasErrorValue;
use collector::Collector;
//--------------------------------------------------
// Part 5 - Tokio Streams
// Developing Asynchronous Components in Rust: Part 5 — Tokio Streams
// https://medium.com/@alfred.weirich/developing-asynchronous-components-in-rust-part-5-tokio-streams-d9ae2973f16e
//--------------------------------------------------

// ----------------------------------------------------------------
// test: Producer 
// ----------------------------------------------------------------

// #[tokio::main(flavor = "multi_thread", worker_threads = 1)]
// async fn main() {
//     env_logger::init();

//     // Create a new DataAvailable stream
//     let data_available = DataAvailable::new();  

//     // Create an mpsc channel to send and receive data
//     let (tx, rx) = mpsc::channel(1);

//     // Create a new Producer stream
//     let mut producer = Producer::new(data_available, rx, Box::new(|x: i32| {
//         info!("  -- Processing data: {}", x);
//         x + 1
//     }));

//     // Spawn a task to send data into the channel
//     tokio::spawn(async  move {
//         for i in 0..=10 {
//             tx.send(i).await.unwrap();
//         }
//     });

//     // Consume the stream and print the results
//     let mut i = 0;
//     while let Some(_data) = producer.next().await {
//         info!("Data available : {}", i);
//         i += 1;
//     }
// }

async fn test_producer() {
    let data_available = DataAvailable::new();  

    // Create an mpsc channel to send and receive data
    let (tx, rx) = mpsc::channel(1);

    // Create a new Producer stream
    let mut producer = Producer::new(data_available, rx, Box::new(|x: i32| {
        info!("  -- Processing data: {}", x);
        x + 1
    }));

    // Spawn a task to send data into the channel
    tokio::spawn(async move {
        for i in 0..=10 {
            tx.send(i).await.unwrap();
        }
    });

    // Consume the stream and print the results
    let mut i = 0;
    while let Some(data) = producer.next().await {
        info!("Produced data[{}] : {}", i, data);
        i += 1;
    }
}

// ----------------------------------------------------------------
//  Test: Collector
// ----------------------------------------------------------------

// #[tokio::main(flavor = "multi_thread", worker_threads = 1)]
// async fn main() {
//     env_logger::init();

//     // Create a new DataAvailable stream
//     let data_available = DataAvailable::new();  

//     // Create an mpsc channel to send and receive data
//     let (tx, rx) = mpsc::channel(1);

//     // Create a new Producer stream
//     let producer = Producer::new(data_available, rx, Box::new(|x: i32| {
//         info!("  -- Processing data: {}", x);
//         x + 1
//     }));

//     // Spawn a task to send data into the channel and send an error
//     tokio::spawn(async  move {
//         for i in 0i32..900 {
//             if i == 11 {
//                 let err = i.into_error_value();
//                 tx.send(err).await.unwrap();
//             } else  if let Err(_e) = tx.send(i).await {
//                 break;  // Stop sending if the channel is closes
//             }
//         }
//     });

//     // Create a new Collector stream
//     let collector = Collector::new(producer);

//     // Wait for the Collector to finish and return the collected Vector
//     let collected_data = collector.await;

//     info!("Collected Data : {:?}", collected_data);
// }

async fn test_collector() {
    // Create a new DataAvailable stream
    let data_available = DataAvailable::new();  

    // Create an mpsc channel to send and receive data
    let (tx, rx) = mpsc::channel(1);

    // Create a new Producer stream
    let producer = Producer::new(data_available, rx, Box::new(|x: i32| {
        info!("  -- Processing data: {}", x);
        x + 1
    }));

    // Spawn a task to send data into the channel and send an error
    tokio::spawn(async  move {
        for i in 0i32..900 {
            if i == 11 {
                let err = i.into_error_value();
                tx.send(err).await.unwrap();
            } else if let Err(_e) = tx.send(i).await {
                break;  // Stop sending if the channel is closes
            }
        }
    });

    // Create a new Collector stream
    let collector = Collector::new(producer);

    // Wait for the Collector to finish and return the collected Vector
    let collected_data = collector.await;

    info!("Collected Data : {:?}", collected_data);
}

// ----------------------------------------------------------------
// Test: 10,1000 Collectors
// ----------------------------------------------------------------
#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
async fn main() {
    env_logger::init();

    info!("--- Test: Producer ---");
    test_producer().await;

    info!("--- Test: Collector ---");
    test_collector().await;

    //-------------------------------------------------------------
    info!("--- Test: 10,1000 Collectors ---");
    let start_time = Instant::now();

    // Create a list of tasks
    let mut tasks = Vec::new();

    for _ in 0..10_000 {
    // for _ in 0..10 {
        let (tx, rx) = mpsc::channel(3);

        // Create a new DataAvailable stream
        let data_available = DataAvailable::new();  


        // Create a new Producer stream
        let producer = Producer::new(data_available, rx, Box::new(|x: i32| {
            // info!("  -- Processing data: {}", x);
            x + 1
        }));

        // Spawn a task to send data into the channel and send an error
        tokio::spawn(async  move {
            for i in 0i32..900 {
                if i == 10 {
                    let err = i.into_error_value();
                    tx.send(err).await.unwrap();
                } else  if let Err(_e) = tx.send(i).await {
                    break;  // Stop sending if the channel is closes
                }
            }
        });

        // Create a new Collector stream
        let collector = Collector::new(producer);

        // Add the collector to the list of tasks
        tasks.push(tokio::spawn(async move {
            // Wait for the Collector to finish and return the collected Vector
            let collected_data = collector.await;
            trace!("Collected Data : {:?}", collected_data);
        }));
    }

    let tasks_len = tasks.len();

    // Wait for all tasks to complete
    let _ = join_all(tasks).await;    

    info!("Number of tasks: {}", tasks_len);

    let elapsed_time = start_time.elapsed();
    info!("Total Elapsed Time : {:?}", elapsed_time);
}
