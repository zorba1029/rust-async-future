mod data_available;
mod producer;
mod has_error;
mod collector;
mod messages;
mod tcp_adapter;

use futures::future::join_all;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

#[allow(unused)]
use log::{info, trace};
#[allow(unused)]
use std::pin::Pin;

use collector::CollectorDyn;
use data_available::DataAvailable;
use producer::Producer;
use messages::{Message, SensorMessage};

#[cfg(feature = "with_tcp")]
use {
    tcp_adapter::TcpAdapter,
    std::time::Duration,
    tokio::{
        // io::AsyncReadExt,
        io::AsyncWriteExt,
        net::{TcpListener, TcpStream},
    },
};

#[cfg(feature = "with_tcp")]
const TCPADDR: &str = "127.0.0.1";
type DataFormat = i32;


//--------------------------------------------------
// Part 6 - Tokio Streams
// Developing Asynchronous Components in Rust: Part 6 — Tokio Streams
// https://medium.com/@alfred.weirich/developing-asynchronous-components-in-rust-part-6-tokio-streams-42366e470e7b
//--------------------------------------------------

fn main() {
    env_logger::init();

    for num_workers in [1, 4, 8, 16, 32, 64] {
        println!("Running with {} workers", num_workers);
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(num_workers)
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                #[cfg(not(feature = "with_tcp"))]
                let collectors = [
                    1_000, 2_000, 5_000, 10_000, 20_000, 50_000, 100_000, 200_000,
                    350_000, 500_000, 750_000, 1_000_000, 1_500_000,
                  ];
                #[cfg(feature = "with_tcp")]
                let collectors = [1_000, 2_000, 5_000, 10_000];

                for num_collectors in collectors {
                    #[cfg(feature = "with_tcp")]
                    {
                        for i in 0..num_collectors {
                            let tcp_addr = format!("{}:{}", TCPADDR, 7800 + i);
                            tokio::spawn(start_tcp_sender(tcp_addr.clone()));
                        }
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }

                    let start_time = Instant::now();

                    const BUFSIZE: usize = 3;
                    // 
                    let mut tasks = Vec::new();

                    for _i in 0..num_collectors {
                        let (tx1, rx1) = mpsc::channel(BUFSIZE); 
                        let (tx2, rx2) = mpsc::channel(BUFSIZE);
                        let (tx3, rx3) = mpsc::channel(BUFSIZE);
                        #[cfg(not(feature = "with_tcp"))]
                        let (tx4, rx4) = mpsc::channel(BUFSIZE);

                        let producer1 = create_producer(ReceiverStream::new(rx1), |x: i32| x + 1);
                        let producer2 = create_producer(ReceiverStream::new(rx2), |x: i32| x + 10);
                        let producer3 = create_producer(ReceiverStream::new(rx3), |x: i32| x + 100);
                        let producer4;
                        #[cfg(not(feature = "with_tcp"))] 
                        {
                            producer4 = create_producer(ReceiverStream::new(rx4), |x: i32| x + 1_000);
                        }

                        #[cfg(feature = "with_tcp")]
                        {
                            let tcp_addr = format!("{}:{}", TCPADDR, 7800 + _i);
                            let tcp_stream = TcpStream::connect(tcp_addr.clone()).await.unwrap();
                            
                            // instead of TcpAdaptor we can use
                            // use tokio_util::io::ReaderStream;
                            // use futures::StreamExt;
                            // ReaderStream::new(tcp_stream).map(|result| {
                            //     match result {
                            //         Ok(bytes) => {
                            //             bytes.to_vec()
                            //         } // Convert Bytes to Vec<u8>
                            //         Err(_) => vec![], 
                            //     }
                            // });
                            producer4 = create_producer(TcpAdapter::new(tcp_stream), |x: i32| x + 1_000);
                        }

                        tokio::spawn(async move {
                            for i in 0i32..900 {
                                let mut msg = Message::new(i as DataFormat, "T1".to_string());
                                if i == 5 {
                                    msg.set_error();
                                }
                                let msg = msg.to_vec();

                                if let Err(_e) = tx1.send(msg).await {
                                    break;
                                }
                            }
                        });

                        tokio::spawn(async move {
                            for i in -1..7 {
                                let mut msg = Message::new(i as DataFormat, "T2".to_string());
                                if i == -1 {
                                    let stream = msg.set_unusable(-1 as DataFormat).to_vec();
                                    tx2.send(stream).await.unwrap();
                                } else if let Err(_e) = tx2.send(msg.to_vec()).await {
                                    break;
                                }
                            }
                        });

                        tokio::spawn(async move {
                            for i in 0..9 {
                                let msg = Message::new(i as DataFormat, "T3".to_string());
                                tx3.send(msg.to_vec()).await.unwrap();
                            }
                        });

                        #[cfg(not(feature = "with_tcp"))]
                        tokio::spawn(async move {
                            for i in 0..3 {
                                let msg = Message::new(i as DataFormat, "T4".to_string());
                                tx4.send(msg.to_vec()).await.unwrap();
                            }
                        });

                        let collector = CollectorDyn::new(vec![
                            Box::new(producer1), 
                            Box::new(producer2), 
                            Box::new(producer3), 
                            Box::new(producer4),
                        ]);

                        tasks.push(tokio::spawn(async move {
                            #[allow(unused_mut, unused)]
                            let mut collected_data = collector.await;
                            let mut collected_data = Message::to_values(collected_data);
                            collected_data.sort();
                            // log::trace!("^^^ --| collected_data: {:?}", collected_data);
                            assert!(
                                collected_data == [
                                    1, 2, 3, 4, 5, 10, 11, 12, 13, 14, 15, 16, 100, 101, 102,
                                    103, 104, 105, 106, 107, 108, 1000, 1001, 1002
                                ]
                            );
                        }));
                    }

                    let _ = join_all(tasks).await;

                    let duration = start_time.elapsed();
                    log::info!(
                        "Total execution time with {} Workers, {} Collectors: {:.2?}",
                        num_workers, num_collectors, duration
                    );
                }
            });
    }
}

#[cfg(feature = "with_tcp")]
async fn start_tcp_sender(addr: String) {
    let listener = TcpListener::bind(&addr).await.unwrap();
    log::info!("TCP Sender started at {}", addr);
    
    let (socket, _) = listener.accept().await.unwrap();

    let messages: Vec<Message<DataFormat>> = (-1..30)
        .map(|i| {
            let mut msg = Message::new(i as DataFormat, "T4".to_string());
            if i == -1 || i == 3 {
                msg.set_unusable(-1);
            } else if i == 4 {
                msg.set_error();
            }
            msg
        })
        .collect();

    send_tcp_messages(socket, messages).await;
}

#[cfg(feature = "with_tcp")]
async fn send_tcp_messages(mut socket: TcpStream, messages: Vec<Message<DataFormat>>) {
    for msg in messages {
        if socket.write_all(&msg.to_vec()).await.is_err() {
            break;
        }
    }
}

use tokio_stream::Stream;

fn create_producer<T, F>(rx: T, func: F) -> Producer<impl Stream<Item = Vec<u8>>, DataFormat> 
where 
    F: Fn(i32) -> i32 + Send + 'static,
    T: Stream<Item = Vec<u8>> + Send + 'static,
{
    Producer::new(DataAvailable::new(), rx, Box::new(func))
}

// Running with 1 workers
// [2025-05-27T02:30:26Z INFO  async_part6] Total execution time with 1 Workers, 1000 Collectors: 5.08s
// [2025-05-27T02:30:31Z INFO  async_part6] Total execution time with 1 Workers, 2000 Collectors: 5.11s
// [2025-05-27T02:30:36Z INFO  async_part6] Total execution time with 1 Workers, 5000 Collectors: 5.24s
// [2025-05-27T02:30:41Z INFO  async_part6] Total execution time with 1 Workers, 10000 Collectors: 5.34s
// [2025-05-27T02:30:47Z INFO  async_part6] Total execution time with 1 Workers, 20000 Collectors: 5.43s
// [2025-05-27T02:30:57Z INFO  async_part6] Total execution time with 1 Workers, 50000 Collectors: 10.29s
// [2025-05-27T02:31:15Z INFO  async_part6] Total execution time with 1 Workers, 100000 Collectors: 17.90s
// [2025-05-27T02:31:49Z INFO  async_part6] Total execution time with 1 Workers, 200000 Collectors: 33.65s
// [2025-05-27T02:32:47Z INFO  async_part6] Total execution time with 1 Workers, 350000 Collectors: 58.49s
// [2025-05-27T02:34:12Z INFO  async_part6] Total execution time with 1 Workers, 500000 Collectors: 84.34s
// [2025-05-27T02:36:20Z INFO  async_part6] Total execution time with 1 Workers, 750000 Collectors: 128.51s
// [2025-05-27T02:39:25Z INFO  async_part6] Total execution time with 1 Workers, 1000000 Collectors: 185.37s
// [2025-05-27T02:43:51Z INFO  async_part6] Total execution time with 1 Workers, 1500000 Collectors: 265.97s
// Running with 4 workers
// [2025-05-27T02:43:56Z INFO  async_part6] Total execution time with 4 Workers, 1000 Collectors: 5.08s
// [2025-05-27T02:44:02Z INFO  async_part6] Total execution time with 4 Workers, 2000 Collectors: 5.11s
// [2025-05-27T02:44:07Z INFO  async_part6] Total execution time with 4 Workers, 5000 Collectors: 5.19s
// [2025-05-27T02:44:12Z INFO  async_part6] Total execution time with 4 Workers, 10000 Collectors: 5.19s
// [2025-05-27T02:44:17Z INFO  async_part6] Total execution time with 4 Workers, 20000 Collectors: 5.29s
// [2025-05-27T02:44:23Z INFO  async_part6] Total execution time with 4 Workers, 50000 Collectors: 5.41s
// [2025-05-27T02:44:29Z INFO  async_part6] Total execution time with 4 Workers, 100000 Collectors: 6.51s
// [2025-05-27T02:44:41Z INFO  async_part6] Total execution time with 4 Workers, 200000 Collectors: 12.27s
// [2025-05-27T02:45:02Z INFO  async_part6] Total execution time with 4 Workers, 350000 Collectors: 20.61s
// [2025-05-27T02:45:31Z INFO  async_part6] Total execution time with 4 Workers, 500000 Collectors: 29.28s
// [2025-05-27T02:46:34Z INFO  async_part6] Total execution time with 4 Workers, 750000 Collectors: 63.19s
// [2025-05-27T02:48:17Z INFO  async_part6] Total execution time with 4 Workers, 1000000 Collectors: 102.33s
// [2025-05-27T02:51:25Z INFO  async_part6] Total execution time with 4 Workers, 1500000 Collectors: 188.59s
// Running with 8 workers
// [2025-05-27T02:51:30Z INFO  async_part6] Total execution time with 8 Workers, 1000 Collectors: 5.07s
// [2025-05-27T02:51:36Z INFO  async_part6] Total execution time with 8 Workers, 2000 Collectors: 5.11s
// [2025-05-27T02:51:41Z INFO  async_part6] Total execution time with 8 Workers, 5000 Collectors: 5.18s
// [2025-05-27T02:51:46Z INFO  async_part6] Total execution time with 8 Workers, 10000 Collectors: 5.20s
// [2025-05-27T02:51:51Z INFO  async_part6] Total execution time with 8 Workers, 20000 Collectors: 5.25s
// [2025-05-27T02:51:57Z INFO  async_part6] Total execution time with 8 Workers, 50000 Collectors: 5.46s
// [2025-05-27T02:52:03Z INFO  async_part6] Total execution time with 8 Workers, 100000 Collectors: 6.72s
// [2025-05-27T02:52:16Z INFO  async_part6] Total execution time with 8 Workers, 200000 Collectors: 12.42s
// [2025-05-27T02:52:37Z INFO  async_part6] Total execution time with 8 Workers, 350000 Collectors: 20.81s
// [2025-05-27T02:53:06Z INFO  async_part6] Total execution time with 8 Workers, 500000 Collectors: 29.41s
// [2025-05-27T02:54:14Z INFO  async_part6] Total execution time with 8 Workers, 750000 Collectors: 67.77s
// [2025-05-27T02:56:01Z INFO  async_part6] Total execution time with 8 Workers, 1000000 Collectors: 107.21s
// [2025-05-27T02:59:22Z INFO  async_part6] Total execution time with 8 Workers, 1500000 Collectors: 200.99s
// Running with 16 workers
// [2025-05-27T02:59:27Z INFO  async_part6] Total execution time with 16 Workers, 1000 Collectors: 5.07s
// [2025-05-27T02:59:32Z INFO  async_part6] Total execution time with 16 Workers, 2000 Collectors: 5.11s
// [2025-05-27T02:59:37Z INFO  async_part6] Total execution time with 16 Workers, 5000 Collectors: 5.17s
// [2025-05-27T02:59:43Z INFO  async_part6] Total execution time with 16 Workers, 10000 Collectors: 5.21s
// [2025-05-27T02:59:48Z INFO  async_part6] Total execution time with 16 Workers, 20000 Collectors: 5.25s
// [2025-05-27T02:59:53Z INFO  async_part6] Total execution time with 16 Workers, 50000 Collectors: 5.43s
// [2025-05-27T03:00:00Z INFO  async_part6] Total execution time with 16 Workers, 100000 Collectors: 6.64s
// [2025-05-27T03:00:12Z INFO  async_part6] Total execution time with 16 Workers, 200000 Collectors: 12.45s
// [2025-05-27T03:00:33Z INFO  async_part6] Total execution time with 16 Workers, 350000 Collectors: 20.87s
// [2025-05-27T03:01:03Z INFO  async_part6] Total execution time with 16 Workers, 500000 Collectors: 29.37s
// [2025-05-27T03:02:07Z INFO  async_part6] Total execution time with 16 Workers, 750000 Collectors: 64.61s
// [2025-05-27T03:03:53Z INFO  async_part6] Total execution time with 16 Workers, 1000000 Collectors: 105.59s
// [2025-05-27T03:07:30Z INFO  async_part6] Total execution time with 16 Workers, 1500000 Collectors: 216.85s
// Running with 32 workers
// [2025-05-27T03:07:35Z INFO  async_part6] Total execution time with 32 Workers, 1000 Collectors: 5.07s
// [2025-05-27T03:07:40Z INFO  async_part6] Total execution time with 32 Workers, 2000 Collectors: 5.11s
// [2025-05-27T03:07:45Z INFO  async_part6] Total execution time with 32 Workers, 5000 Collectors: 5.16s
// [2025-05-27T03:07:50Z INFO  async_part6] Total execution time with 32 Workers, 10000 Collectors: 5.19s
// [2025-05-27T03:07:55Z INFO  async_part6] Total execution time with 32 Workers, 20000 Collectors: 5.25s
// [2025-05-27T03:08:01Z INFO  async_part6] Total execution time with 32 Workers, 50000 Collectors: 5.43s
// [2025-05-27T03:08:07Z INFO  async_part6] Total execution time with 32 Workers, 100000 Collectors: 6.59s
// [2025-05-27T03:08:19Z INFO  async_part6] Total execution time with 32 Workers, 200000 Collectors: 11.56s
// [2025-05-27T03:08:39Z INFO  async_part6] Total execution time with 32 Workers, 350000 Collectors: 19.62s
// [2025-05-27T03:09:07Z INFO  async_part6] Total execution time with 32 Workers, 500000 Collectors: 28.27s
// [2025-05-27T03:10:15Z INFO  async_part6] Total execution time with 32 Workers, 750000 Collectors: 67.73s
// [2025-05-27T03:12:10Z INFO  async_part6] Total execution time with 32 Workers, 1000000 Collectors: 115.31s
// [2025-05-27T03:16:06Z INFO  async_part6] Total execution time with 32 Workers, 1500000 Collectors: 235.61s
// Running with 64 workers
// [2025-05-27T03:16:11Z INFO  async_part6] Total execution time with 64 Workers, 1000 Collectors: 5.09s
// [2025-05-27T03:16:16Z INFO  async_part6] Total execution time with 64 Workers, 2000 Collectors: 5.10s
// [2025-05-27T03:16:21Z INFO  async_part6] Total execution time with 64 Workers, 5000 Collectors: 5.14s
// [2025-05-27T03:16:26Z INFO  async_part6] Total execution time with 64 Workers, 10000 Collectors: 5.15s
// [2025-05-27T03:16:31Z INFO  async_part6] Total execution time with 64 Workers, 20000 Collectors: 5.23s
// [2025-05-27T03:16:37Z INFO  async_part6] Total execution time with 64 Workers, 50000 Collectors: 5.44s
// [2025-05-27T03:16:44Z INFO  async_part6] Total execution time with 64 Workers, 100000 Collectors: 7.11s
// [2025-05-27T03:16:56Z INFO  async_part6] Total execution time with 64 Workers, 200000 Collectors: 11.97s
// [2025-05-27T03:17:16Z INFO  async_part6] Total execution time with 64 Workers, 350000 Collectors: 19.86s
// [2025-05-27T03:17:44Z INFO  async_part6] Total execution time with 64 Workers, 500000 Collectors: 28.05s
// [2025-05-27T03:18:51Z INFO  async_part6] Total execution time with 64 Workers, 750000 Collectors: 67.01s
// [2025-05-27T03:21:36Z INFO  async_part6] Total execution time with 64 Workers, 1000000 Collectors: 165.29s
// [2025-05-27T03:24:30Z INFO  async_part6] Total execution time with 64 Workers, 1500000 Collectors: 174.17s