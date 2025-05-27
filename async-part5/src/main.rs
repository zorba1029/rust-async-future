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
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

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
// Test: DataAvailable
// ----------------------------------------------------------------
async fn test_data_available() {
    // Create a new DataAvailable stream
    let mut data_available = DataAvailable::new();

    // Consume the stream and print the results
    let mut i = 0;
    while let Some(data) = data_available.next().await {
        info!("Data Available[{}] : {}", i, data);
        i += 1;
        if i > 10 {
            break;
        }
    }
}

// ----------------------------------------------------------------
// test: Producer 
// ----------------------------------------------------------------
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
        for i in 0..900i32 {
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
// Test: 10,000 Collectors
// ----------------------------------------------------------------
async fn test_10_1000_collectors() {
    let start_time = Instant::now();

    // Create a list of tasks
    let mut tasks = Vec::new();
    let task_counter = Arc::new(AtomicUsize::new(0));

    // for _ in 0..10_000 {
    for _ in 0..100 {
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
        // let counter_clone = Arc::clone(&task_counter);
        tasks.push(tokio::spawn(async move {
            // Wait for the Collector to finish and return the collected Vector
            let collected_data = collector.await;
            trace!("Collected data: {:?}", collected_data);
            // let current_i = counter_clone.fetch_add(1, Ordering::SeqCst);
            // trace!("Collected Data[{}] : {:?}", current_i, collected_data);
        }));
    }

    let tasks_len = tasks.len();

    // Wait for all tasks to complete
    let _ = join_all(tasks).await;    

    info!("Number of tasks: {}", tasks_len);

    let elapsed_time = start_time.elapsed();
    info!("Total Elapsed Time : {:?}", elapsed_time);
}


#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
async fn main() {
    env_logger::init();

    info!("--- Test: DataAvailable -----------------------------");
    test_data_available().await;

    info!("--- Test: Producer --------------------------------");
    test_producer().await;

    info!("--- Test: Collector --------------------------------");
    test_collector().await;

    //-------------------------------------------------------------
    info!("--- Test: 10,1000 Collectors ------------------------");
    test_10_1000_collectors().await;
}
