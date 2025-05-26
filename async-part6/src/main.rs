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
