
// Developing Asynchronous Components in Rust: Part 1 — Basics
// article source
// https://medium.com/@alfred.weirich/developing-asynchronous-components-in-rust-bea1db85c720
//
use pin_project::pin_project;
use rand::{distributions::uniform::SampleUniform, thread_rng, Rng};
use std::fmt::Debug;
use std::future::Future;
use std::io::{Read, Result, Write};
use std::net::{TcpListener, TcpStream};
use std::pin::Pin;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::task::{Context, Poll, Waker};
use std::thread::{sleep, spawn};
use std::time::Duration;

// perform the next step in polling
macro_rules! poll_step {
    ($self:ident) => {
        // no core functionality of future
        // check is working thread changed
        // can get removed
        if let Some(new_id) = check_thread_id!($self) {
            *$self.thread_id = Some(new_id);
        }
        // get new data and check if we are ready
        if new_data_and_check_ready!($self) {
            println!("MATCH {} Steps: {}", $self.num, $self.status);
            return Poll::Ready($self.result.clone()); // Return the result
        }
        return Poll::Pending // Continue polling if condition not met
    };
}

// get new data and check if read_condition is reached
macro_rules! new_data_and_check_ready {
    ($self:ident) => {{
        if !$self.producer.data_available() {
            false // Return false immediately if no data is available
        } else {
            let data = $self.producer.produce(); // Produce new data
            $self.result.push(data); // Store the produced data
            *$self.sum += data; // Update the future's sum
            // Increment future's status
            *$self.status += 1; 

            // check ready-condition
            if ready_condition!($self) {
                $self.producer.stop(); // Stop if the ready condition is met
                true // Return true if the condition is met
            } else {
                false // Return false if the condition is not met
            }
        }
    }};
}

// logic to check when the collector's ready condition is met
macro_rules! ready_condition {
    ($self:ident) => {
        *$self.sum % P::from(17) == P::from(0)
    };
}

// Check if the future changes its working thread
// nothing to do with core functionality
// can be removed
macro_rules! check_thread_id {
    ($self:ident) => {{
        let new_thread_id = std::thread::current().id();
        if *$self.thread_id != Some(new_thread_id) {
            if let Some(stored_thread_id) = *$self.thread_id {
                println!(
                    "---------------->Thread ID Changed: {} from/to {:?}/{:?}",
                    $self.num, stored_thread_id, new_thread_id
                );
            }
            Some(new_thread_id) // return the updated thread ID
        } else {
            *$self.thread_id // No change, return the original thread ID
        }
    }};
}

//----------------------------------------------
// Producer trait
//----------------------------------------------

// Trait definition for a Producer that can produce data and manage a waker
trait Producer<T> {
    // get new data element
    fn produce(&mut self) -> T;
    // check if producer has new data
    fn data_available(&self) -> bool;
    // store the futures current waker
    fn store_waker(&mut self, waker: &Waker) {
        match self.get_waker() {
            None => {
                self.set_waker(Some(waker.clone()));
            }
            Some(old_waker) => {
                if !waker.will_wake(old_waker) {
                    println!("---------------->Waker Changed");
                    self.set_waker(Some(waker.clone()));
                }
            }
        }
    }
    // send stop signal to producer
    fn stop(&mut self) {}
    // support methods
    fn set_waker(&mut self, waker: Option<Waker>);
    fn get_waker(&self) -> Option<&Waker>;
}
macro_rules! impl_waker_methods {
    () => {
        fn get_waker(&self) -> Option<&Waker> {
            self.waker.as_ref()
        }

        fn set_waker(&mut self, waker: Option<Waker>) {
            self.waker = waker;
        }
    };
}

//----------------------------------------------
// RandProducer
//----------------------------------------------

// Define a random data producer for async collection
#[derive(Default, Debug)]
struct RandProducer<T> {
    waker: Option<Waker>,                 // Waker for waking up async tasks
    _marker: std::marker::PhantomData<T>, // Marker for generic type T
}

impl<T> RandProducer<T>
where
    T: Default,
{
    #[allow(dead_code)]
    fn new() -> Self {
        Self::default() // Constructor for RandProducer using default values
    }
}

impl<T> Producer<T> for RandProducer<T>
where
    T: PartialOrd + From<u8> + SampleUniform,
{
    // Simulate whether data is available
    fn data_available(&self) -> bool {
        let mut rng = thread_rng(); // Random number generator
        sleep(Duration::from_millis(rng.gen_range(100..=1000))); // Simulate a delay
        true
    }

    // Produce random data within a given range
    fn produce(&mut self) -> T {
        let mut rng = thread_rng();
        if let Some(waker) = &self.waker {
            waker.wake_by_ref(); // Wake up the async task if the waker is present
        }
        let r = std::ops::Range::<T> {
            start: T::from(1),
            end: T::from(10),
        };
        rng.gen_range(r) // Generate a random value within the range
    }

    impl_waker_methods!();
}

//----------------------------------------------
// ChannelProducer
//----------------------------------------------
// Producer that sends data over a channel
struct ChannelProducer<T> {
    waker: Option<Waker>,
    sender: SyncSender<T>, // Sender to send data across threads
    receiver: Receiver<T>, // Receiver to receive data from another thread
}

// Default implementation for ChannelProducer
impl<T> Default for ChannelProducer<T> {
    fn default() -> Self {
        let (sender, receiver) = sync_channel::<T>(0); // Synchronous channel
        ChannelProducer {
            waker: None,
            sender,
            receiver,
        }
    }
}

// Implementation for ChannelProducer that runs in its own thread
impl<T> ChannelProducer<T>
where
    T: PartialOrd + From<u8> + SampleUniform + Send + 'static,
{
    #[allow(dead_code)]
    fn new() -> Self {
        let prod = Self::default();
        let sender = prod.sender.clone();
        // Spawn a thread to continuously produce random data and send it via the channel
        spawn(move || loop {
            let mut rng = thread_rng();
            let r = std::ops::Range::<T> {
                start: T::from(1),
                end: T::from(100),
            };
            let val = rng.gen_range(r);
            match sender.send(val) {
                Ok(_) => continue, // Keep producing data
                Err(_e) => break,  // Exit the loop if sending fails
            }
        });
        prod
    }
}

// Implement the Producer trait for ChannelProducer
impl<T> Producer<T> for ChannelProducer<T> {
    fn data_available(&self) -> bool {
        let mut rng = thread_rng();
        sleep(Duration::from_millis(rng.gen_range(100..=1000))); // Simulate a delay
        true
    }

    // Receive data from the channel
    fn produce(&mut self) -> T {
        let data = self.receiver.recv().unwrap();
        if let Some(waker) = &self.waker {
            waker.wake_by_ref(); // Wake up the async task if the waker is present
        }
        data
    }

    impl_waker_methods!();
}
//----------------------------------------------
// TcpProducer
//----------------------------------------------

// Producer that reads and writes data over TCP
struct TCPProducer<T> {
    waker: Option<Waker>,
    stream: TcpStream,                    // TCP stream for communication
    _marker: std::marker::PhantomData<T>, // Marker for generic type T
}

// TCPProducer constructor and communication logic
impl<T> TCPProducer<T>
where
    T: ToBytes + PartialOrd + From<u8> + SampleUniform,
{
    const STOP: i8 = -1; // Stop signal constant
    const ACK: i8 = 0; // Acknowledgment signal constant

    #[allow(dead_code)]
    fn new(addr: impl Into<String>) -> Self {
        let addr: String = addr.into();

        // Set up TCP listener to accept one client
        let listener = TcpListener::bind(addr.clone()).expect("Build TCP listener");
        let server_handle = spawn(move || listener.accept().expect("Failed to accept connection"));

        // Spawn a thread to send data over TCP
        spawn({
            move || {
                if let Err(_e) = TCPProducer::<T>::send_data(addr) {}
            }
        });

        let (stream, _addr) = server_handle.join().unwrap();
        Self {
            waker: None,
            stream,
            _marker: std::marker::PhantomData,
        }
    }

    // Read data from the TCP stream
    fn read_data(&mut self) -> std::result::Result<T, std::io::Error> {
        write_number(&mut self.stream, TCPProducer::<T>::ACK)?; // Send ACK

        // Read exactly one value from the stream
        let ret = match read_number(&mut self.stream) {
            Ok(received_number) => received_number,
            Err(e) => {
                write_number(&mut self.stream, TCPProducer::<T>::STOP)?; // Send stop signal on error
                return Err(e);
            }
        };

        Ok(ret)
    }

    // Send data over the TCP stream in a loop
    fn send_data(addr: impl Into<String>) -> Result<()> {
        let addr: String = addr.into();
        let mut stream = TcpStream::connect(addr)?;
        let mut rng = thread_rng();

        loop {
            let r = std::ops::Range::<T> {
                start: T::from(0),
                end: T::from(250),
            };
            let number_to_send = rng.gen_range(r);

            let ret: i8 = match read_number(&mut stream) {
                Ok(received_number) => received_number,
                Err(e) => {
                    return Err(e);
                }
            };

            // Break the loop if stop signal is received
            if ret == TCPProducer::<T>::STOP {
                break;
            }

            // Send the generated number over the stream
            write_number(&mut stream, number_to_send)?;
        }
        Ok(())
    }
}

// Implement the Producer trait for TCPProducer
impl<T> Producer<T> for TCPProducer<T>
where
    T: ToBytes + From<u8> + SampleUniform + PartialOrd,
{
    fn stop(&mut self) {
        let _ = write_number(&mut self.stream, TCPProducer::<T>::STOP); // Send stop signal to the client
    }

    fn data_available(&self) -> bool {
        let mut rng = thread_rng();
        sleep(Duration::from_millis(rng.gen_range(100..=1000))); // Simulate a delay
        true
    }

    // Produce data by reading from the TCP stream
    fn produce(&mut self) -> T {
        let data = self.read_data().unwrap();
        if let Some(waker) = &self.waker {
            waker.wake_by_ref(); // Wake up the async task if the waker is present
        }

        data
    }

    impl_waker_methods!();
}

pub trait ToBytes: Sized {
    fn to_le_bytes(&self) -> Vec<u8>;
    fn from_le_bytes(bytes: &[u8]) -> Self;
}

macro_rules! impl_tobytes_for {
    ($t:ty) => {
        impl ToBytes for $t {
            fn to_le_bytes(&self) -> Vec<u8> {
                Self::to_le_bytes(*self).to_vec()
            }

            fn from_le_bytes(bytes: &[u8]) -> Self {
                let array: [u8; std::mem::size_of::<Self>()] =
                    bytes.try_into().expect("slice with incorrect length");
                Self::from_le_bytes(array)
            }
        }
    };
}

// Implement ToBytes for some integer types
impl_tobytes_for!(i8);
impl_tobytes_for!(u64);
impl_tobytes_for!(i32);
impl_tobytes_for!(i16);
impl_tobytes_for!(u16);

fn write_number<W: Write, T: ToBytes>(stream: &mut W, value: T) -> std::io::Result<()> {
    let bytes = value.to_le_bytes();
    stream.write_all(&bytes[..])
}

fn read_number<R: Read, T: ToBytes>(stream: &mut R) -> std::io::Result<T> {
    let mut buf = vec![0u8; std::mem::size_of::<T>()];
    stream.read_exact(&mut buf)?;
    Ok(T::from_le_bytes(&buf))
}

//----------------------------------------------
// Collector
//----------------------------------------------
// status: tracks the current step in the data collection process
// producer: the generic source of data, which must implement the Producer trait
// result: stores the collected data from producer
// sum: maintain a running total of the collected values, used to check the Ready condition
//      (if sum is divisible by 17, which is READY condition)
// num: used only for debugging; identifying instances of the Collector
// thread_id: used only for debugging; to detect if the working thread has changed during execution

#[pin_project]
struct Collector<T, P> {
    status: usize,  // Status to track progress
    producer: T,    // The producer providing data
    result: Vec<P>, // Vector to store collected data
    sum: P,         // Sum of produced data
    num: u32,       // only used for debugging
    thread_id: Option<std::thread::ThreadId>, // only to check if future changes working thread
}

// Define a Collector that collects data from a producer asynchronously
impl<T, P> Collector<T, P>
where
    T: Producer<P>,
    P: std::convert::From<u8>,
{
    const NUM_DATA: usize = 100; // Number of max data points to collect

    #[allow(dead_code)]
    fn new(producer: T, num: u32) -> Self {
        Collector {
            status: 0, // the status of the future
            producer, // its data producer
            // resulting data gathered
            result: Vec::with_capacity(Collector::<T, P>::NUM_DATA),
            // rolling sum of result
            sum: P::from(0),
            // num: number to identify the collector in debugs/prints
            // nothing to do with functionality
            num, 
            // thread_id: only to check if future changes working thread
            // nothing to do with functionality
            thread_id: None,
        }
    }
}

//-------------------------------------------------------------------------
// Implement the Future trait for the Collector to allow it to be awaited
//-------------------------------------------------------------------------
// 1> Pinning the struct: let myself = self.project() accesses teh internal fields of the Collector
//    in a memory-safe manner, ensuring they reamin pinned in place.
// 2> Store the Waker: the current task's Waker is stored in the producer so the async runtime
//    can be notified when more data is available.
// 3> Polling Logic: poll() method
//
// Implement the Future trait for the Collector to allow it to be awaited
impl<T, P> Future for Collector<T, P>
where
    T: Producer<P> + std::marker::Unpin,
    P: std::convert::From<u8> + Copy + std::ops::Rem + std::ops::AddAssign,
    <P as std::ops::Rem>::Output: PartialEq<P>,
{
    type Output = Vec<P>; // The output type is a vector of produced data

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project(); // Access the internal fields of the Collector safely

        // Store the current waker
        this.producer.store_waker(cx.waker());

        match *this.status {
            // Handle case where all data is collected but no condition match
            Collector::<T, P>::NUM_DATA => {
                println!("READY, NO MATCH: {}", *this.num);
                this.producer.stop(); // Stop the producer
                Poll::Ready(this.result.clone()) // Return the result
            }
            // Handle case where collection of data not ready
            _ => {
                poll_step!(this);
            }
        }
    }
}

// Main function using Tokio runtime to collect data concurrently
#[tokio::main(flavor = "multi_thread", worker_threads = 64)]
async fn main() {
    let mut tasks: Vec<tokio::task::JoinHandle<()>> = Vec::new();

    for i in 0..500 {
        let task: tokio::task::JoinHandle<()> = tokio::spawn(async move {
            if i % 3 == 0 {
                // ChannelProducer for u16
                let p: ChannelProducer<u16> = ChannelProducer::new();
                let collector = Collector::new(p, i);
                let _res = collector.await;
                //println!("Data from ChannelProducer: {:#?}", _res);
            } else if i % 3 == 1 {
                // RandProducer
                let p: RandProducer<i16> = RandProducer::new();
                let collector = Collector::new(p, i);
                let _res = collector.await;
                //println!("Data from RandProducer: {:#?}", _res);
            } else {
                // TCPProducer
                let addr = format!("127.0.0.1:{}", 7800 + i);
                let p: TCPProducer<u64> = TCPProducer::new(addr);
                let collector = Collector::new(p, i);
                let _res = collector.await;
                //println!("Data from TCPProducer: {:#?}", _res);
            }
        });
        tasks.push(task);
    }
    for task in tasks {
        let _ = task.await;
    }
    println!("All tasks completed");
}
