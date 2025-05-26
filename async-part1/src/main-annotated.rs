
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

// // perform the next step in polling
// macro_rules! poll_step {
//     ($self:ident) => {
//         // checks if the working thread has changed        
//         if let Some(new_id) = check_thread_id!($self) {
//             *$self.thread_id = Some(new_id);
//         }
//         // get new data and check if the condition is READY
//         if new_data_and_check_ready!($self) {
//             println!("MATCH {} Steps: {}", $self.num, $self.status);
//             return Poll::Ready($self.result.clone());
//         }
//         // condition is NOT met yet
//         return Poll::Pending
//     }
// }

// get new data and check if read_condition is reached
// macro_rules! new_data_and_check_ready {
//     ($self:ident) => {{
//         if !$self.producer.data_available() {
//             false
//         } else {
//             // produce new data
//             let data = $self.producer.produce();
//             // store the produced data
//             $self.result.push(data);
//             // update the future's sum
//             *$self.sum += data;
//             // increment future's status
//             *$self.status += 1;
// 
//             // check ready-condition
//             // if ready_condition!($self) {
//             //     // stop if the ready condition is met
//             //     $self.producer.stop();
//             //     true
//             // } else {
//             //     false
//             // }
//             if *$self.sum % P::from(17) == P::from(0) {
//                 // stop if the ready condition is met
//                 $self.producer.stop();
//                 true
//             } else {
//                 false
//             }
//         }
//     }};
// }

// macro_rules! ready_condition {
//     ($self:ident) => {
//         *$self.sum % P::from(17) == P::from(0)
//     };
// }

// macro_rules! check_thread_id {
//     ($self:ident) => {{
//         let new_thread_id = std::thread::current().id();
//         if *$self.thread_id != Some(new_thread_id) {
//             if let Some(stored_thread_id) = *$self.thread_id {
//                 println!("------------------> Thread ID Changed: {} from/to {:?}/{:?}",
//                         $self.num, stored_thread_id, new_thread_id);
//             }
//             Some(new_thread_id)
//         } else {
//             *$self.thread_id 
//         }
//     }};
// }

//----------------------------------------------
// Producer trait
//----------------------------------------------

trait Producer<T> {
    // generates a new data point
    fn produce(&mut self) -> T;
    // checks if data is available for collection
    fn data_available(&self) -> bool;
    // stores a Waker to notify the runtime(Executor) when new data is Ready
    fn store_waker(&mut self, waker: &Waker) {
        match self.get_waker() {
            None => {
                self.set_waker(Some(waker.clone()));
            },
            Some(old_waker) => {
                if !waker.will_wake(old_waker) {
                    println!("  ------------------> Waker Changed");
                    self.set_waker(Some(waker.clone()));
                }
            }
        }
    }
    fn stop(&mut self) {}
    fn set_waker(&mut self, waker: Option<Waker>);
    fn get_waker(&self) -> Option<&Waker>;
}

// macro_rules! impl_waker_methods {
//     () => {
//         fn get_waker(&self) -> Option<&Waker> {
//             self.waker.as_ref()
//         }

//         fn set_waker(&mut self, waker: Option<Waker>) {
//             self.waker = waker;
//         }
//     };
// }

//----------------------------------------------
// RandProducer
//----------------------------------------------

#[derive(Default, Debug)]
struct RandProducer<T> {
    waker: Option<Waker>,
    _marker: std::marker::PhantomData<T>,
}

impl<T> RandProducer<T>
where 
    T: Default {
    #[allow(dead_code)]
    fn new() -> Self {
        Self::default()
    }
}

impl<T> Producer<T> for RandProducer<T> 
where 
    // T: PartialOrd + From<u8> + SampleUniform {
    // T: PartialOrd + From<i8> + SampleUniform + Debug {
    T: PartialOrd + From<i16> + SampleUniform + Debug {
    fn produce(&mut self) -> T {
        let mut rng = thread_rng();
        if let Some(waker) = &self.waker {
            waker.wake_by_ref();
        }
        // let r = std::ops::Range::<T> { start: T::from(1), end: T::from(10) };
        let r = std::ops::Range::<T> { start: T::from(-100), end: T::from(100) };
        let value = rng.gen_range(r);
        println!("  RandProducer<T>::produce() - value: {:?}", value);
        value
    }

    fn data_available(&self) -> bool {
        let mut rng = thread_rng();
        sleep(Duration::from_millis(rng.gen_range(100..=1000)));
        // sleep(Duration::from_millis(rng.gen_range(1000..=10000)));
        true
    }
    //-----------------------
    // impl_waker_methods!();
    fn set_waker(&mut self, waker: Option<Waker>) {
        self.waker = waker;
    }
    fn get_waker(&self) -> Option<&Waker> {
        self.waker.as_ref()
    }
    
    fn store_waker(&mut self, waker: &Waker) {
        match self.get_waker() {
            None => {
                self.set_waker(Some(waker.clone()));
            },
            Some(old_waker) => {
                if !waker.will_wake(old_waker) {
                    std::println!("  ------------------> Waker Changed");
                    self.set_waker(Some(waker.clone()));
                }
            }
        }
    }
    
    fn stop(&mut self) {
        println!("RandProducer<T>::stop()--");
    }
}

//----------------------------------------------
// ChannelProducer
//----------------------------------------------
struct ChannelProducer<T> {
    id: u32,
    waker: Option<Waker>,
    sender: SyncSender<T>,
    receiver: Receiver<T>,
}

impl<T> Default for ChannelProducer<T> {
    fn default() -> Self {
        let (sender, receiver) = sync_channel::<T>(0);
        ChannelProducer {
            id: 0,
            waker: None,
            sender,
            receiver,
        }
    }
}

impl<T> ChannelProducer<T>
where 
    // T: PartialOrd + From<u8> + SampleUniform + Send + 'static,
    // T: PartialOrd + From<u8> + SampleUniform + Send + Debug + 'static,
    T: PartialOrd + From<i16> + SampleUniform + Send + Debug + 'static,
{
    fn init(id: u32) -> Self {
        let (sender, receiver) = sync_channel::<T>(0);
        ChannelProducer {
            id,
            waker: None,
            sender,
            receiver,
        }
    }

    #[allow(dead_code)]
    fn new(id: u32) -> Self {
        // let mut prod = Self::default();
        let prod = Self::init(id);

        let sender = prod.sender.clone();

        spawn(move || loop {
            let mut rng = thread_rng();
            // let r = std::ops::Range::<T> { start: T::from(1), end: T::from(100) };
            let r = std::ops::Range::<T> { start: T::from(-512), end: T::from(64) };
            let val = rng.gen_range(r);

            println!("ChannelProducer::new() LOOP - SEND [{}]: {:?}", id, val);
            match sender.send(val) {
                Ok(_) => continue,
                Err(_e) => break,
            }
        });
        prod
    }
}

impl<T> Producer<T> for ChannelProducer<T> 
where 
    // T: PartialOrd + From<u8> + SampleUniform + Send + 'static,
    // T: PartialOrd + From<u8> + SampleUniform + Send + Debug + 'static, 
    T: PartialOrd + From<i16> + SampleUniform + Send + Debug + 'static,
{
    fn data_available(&self) -> bool {
        let mut rng = thread_rng();
        sleep(Duration::from_millis(rng.gen_range(100..=1000)));
        // sleep(Duration::from_millis(rng.gen_range(1000..=10000)));
        true
    }

    fn produce(&mut self) -> T {
        let data = self.receiver.recv().unwrap();
        println!("ChannelProducer::produce() - GET[{}]: {:?}", self.id, data);

        if let Some(waker) = &self.waker {
            waker.wake_by_ref();
        }

        data
    }

    //-----------------------
    // impl_waker_methods!();
    fn get_waker(&self) -> Option<&Waker> {
        self.waker.as_ref()
    }
    fn set_waker(&mut self, waker: Option<Waker>) {
        self.waker = waker;
    }
}

//----------------------------------------------
// TcpProducer
//----------------------------------------------

#[derive(Debug)]
struct TcpProducer<T> {
    waker: Option<Waker>,
    stream: TcpStream,
    _marker: std::marker::PhantomData<T>,
}

impl<T> TcpProducer<T>
where 
    // T: ToBytes + PartialOrd + From<u8> + SampleUniform,
    // T: ToBytes + PartialOrd + From<u8> + SampleUniform + Debug,
    T: ToBytes + PartialOrd + From<i16> + SampleUniform + Debug,
{
    const STOP: i8 = -1;
    const ACK: i8 = 0;

    #[allow(dead_code)]
    fn new(addr: impl Into<String>) -> Self {
        let addr: String = addr.into();

        let listener = TcpListener::bind(addr.clone()).expect("Build TCP listener");
        let server_handle = spawn(move || listener.accept().expect("Failed to accept connection"));

        spawn(move || {
            if let Err(_e) = TcpProducer::<T>::send_data(addr) { 
                    println!("Error in TcpProducer::new() - TcpProducer::<T>::send_data(addr)")
            }
        });

        let (stream, _addr) = server_handle.join().unwrap();
        Self {
            waker: None,
            stream,
            _marker: std::marker::PhantomData,
        }
    }

    fn read_data(&mut self) -> std::result::Result<T, std::io::Error> {
        //-- connect 이후, 최초에 먼저 ACK를 보내야 시작 된다.
        // write_number(&mut self.stream, TcpProducer::<T>::ACK)?;
        write_bytes_from_number(&mut self.stream, TcpProducer::<T>::ACK)?;

        // let ret = match read_number(&mut self.stream) {
        let ret = match read_number_from_bytes(&mut self.stream) {
            // Ok(received_number) => received_number,
            Ok(received_number) => {
                println!("-->> TcpProducer::read_data() - GET : {:?}", received_number);
                received_number
            }
            Err(e) => {
                // write_number(&mut self.stream, TcpProducer::<T>::STOP)?;
                write_bytes_from_number(&mut self.stream, TcpProducer::<T>::STOP)?;
                return Err(e);
            }
        };

        Ok(ret)
    }

    fn send_data(addr: impl Into<String>) -> Result<()> {
        let addr: String = addr.into();
        let mut stream = TcpStream::connect(addr)?;
        let mut rng = thread_rng();

        loop {
            // let r = std::ops::Range::<T> { start: T::from(0), end: T::from(250) };
            let r = std::ops::Range::<T> { start: T::from(-250), end: T::from(250) };
            // let number_to_read = rng.gen_range(r);
            let number_to_send = rng.gen_range(r);

            // ACK/STOP을 먼저 수신해야 데이터를 전송 한다.
            // let ret: i8 = match read_number(&mut stream) {
            let ret: i8 = match read_number_from_bytes(&mut stream) {
                // Ok(received_number) => received_number,
                Ok(received_token) => {
                    println!("-->> TcpProducer::send_data() - GET ACK/STOP : {}", received_token);
                    received_token
                },  // ACK or STOP
                Err(e) => {
                    return Err(e);
                }
            };

            if ret == TcpProducer::<T>::STOP {
                break;
            }

            println!("<<-- TcpProducer::send_data() - SEND DATA : {:?}", number_to_send);
            // write_number(&mut stream, number_to_read)?;
            write_bytes_from_number(&mut stream, number_to_send)?;
        }

        Ok(())
    }
}

impl<T> Producer<T> for TcpProducer<T> 
where 
    // T: ToBytes + From<u8> + SampleUniform + PartialOrd,
    // T: ToBytes + PartialOrd + From<u8> + SampleUniform + Debug,
    T: ToBytes + PartialOrd + From<i16> + SampleUniform + Debug,
{   
    fn stop(&mut self) {
        // let _ = write_number(&mut self.stream, TcpProducer::<T>::STOP);
        let _ = write_bytes_from_number(&mut self.stream, TcpProducer::<T>::STOP);
    }

    fn data_available(&self) -> bool {
        let mut rng = thread_rng();
        sleep(Duration::from_millis(rng.gen_range(100..=1000)));
        // sleep(Duration::from_millis(rng.gen_range(1000..=10000)));
        // sleep(Duration::from_millis(5000));
        true
    }

    fn produce(&mut self) -> T {
        let data = self.read_data().unwrap();
        if let Some(waker) = &self.waker {
            waker.wake_by_ref();
        }
        println!("  TcpProducer<T>::produce() - data: {:?}", data);
        data
    }

    //-----------------------
    // impl_waker_methods!();
    fn get_waker(&self) -> Option<&Waker> {
        self.waker.as_ref()
    }
    fn set_waker(&mut self, waker: Option<Waker>) {
        self.waker = waker;
    }
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
impl_tobytes_for!(i16);
impl_tobytes_for!(u16);
impl_tobytes_for!(i32);
// impl_tobytes_for!(u32);
impl_tobytes_for!(i64);
impl_tobytes_for!(u64);

impl ToBytes for u32 {
    fn to_le_bytes(&self) -> Vec<u8> {
        Self::to_be_bytes(*self).to_vec()
    }

    fn from_le_bytes(bytes: &[u8]) -> Self {
        let array: [u8; std::mem::size_of::<Self>()] = 
            bytes.try_into().expect("slice with incorrect length");
        Self::from_le_bytes(array)
    }
}

// 숫자(현재 정의된 T)를 bytes array로 변환하여 TcpStream에 전송(쓰기) 한다.
// fn write_number<W: Write, T: ToBytes>(stream: &mut W, value: T) -> std::io::Result<()> {
fn write_bytes_from_number<W: Write, T: ToBytes>(stream: &mut W, value: T) -> std::io::Result<()> {
    let bytes = value.to_le_bytes();
    stream.write_all(&bytes[..])
}

// TcpStream으로 부터 데이터를 수신(읽기) 한다. bytes로 읽어서 숫자(현재 정의된 T)로 변환하여 리턴하다.
// fn read_number<R: Read, T: ToBytes>(stream: &mut R) -> std::io::Result<T> {
fn read_number_from_bytes<R: Read, T: ToBytes>(stream: &mut R) -> std::io::Result<T> {
    let mut buf = vec![0u8; std::mem::size_of::<T>()];
    stream.read_exact(&mut buf)?;
    Ok(T::from_le_bytes(&buf))
}

//----------------------------------------------
// Collector
//----------------------------------------------

#[pin_project]
struct Collector<T, P> {
    status: usize,
    producer: T,
    result: Vec<P>,
    sum: P,
    num: u32,
    thread_id: Option<std::thread::ThreadId>,
}
// status: tracks the current step in the data collection process
// producer: the generic source of data, which must implement the Producer trait
// result: stores the collected data from producer
// sum: maintain a running total of the collected values, used to check the Ready condition
//      (if sum is divisible by 17, which is READY condition)
// num: used only for debugging; identifying instances of the Collector
// thread_id: used only for debugging; to detect if the working thread has changed during execution

impl<T, P> Collector<T, P>
where 
    T: Producer<P>,
    P: std::convert::From<u8>,
{
    // const NUM_DATA: usize = 100;
    const NUM_DATA: usize = 10;

    #[allow(dead_code)]
    fn new(producer: T, num: u32) -> Self {
        Collector {
            status: 0,
            producer,
            result: Vec::with_capacity(Collector::<T,P>::NUM_DATA),
            sum: P::from(0),
            num,
            thread_id: None,
        }
    }
}

//-------------------------------------------------------------------------
// Implement the Future trait for the Collector to allow it to be awaited
//-------------------------------------------------------------------------
// 1> Pinning the struct: let me = self.project() accesses teh internal fields of the Collector
//    in a memory-safe manner, ensuring they reamin pinned in place.
// 2> Store the Waker: the current task's Waker is stored in the producer so the async runtime
//    can be notified when more data is available.
// 3> Polling Logic: poll() method
//
impl<T, P> Future for Collector<T, P>
where 
    T: Producer<P> + std::marker::Unpin,
    // P: std::convert::From<u8> + Copy + std::ops::Rem + std::ops::AddAssign,
    P: std::convert::From<u8> + Copy + std::ops::Rem + std::ops::AddAssign + Debug,
    // P: std::convert::From<i8> + Copy + std::ops::Rem + std::ops::AddAssign + Debug,
    <P as std::ops::Rem>::Output: PartialEq<P>, 
{
    // The Output type is a vector of produced data
    type Output = Vec<P>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        println!("--------------|| Collector::poll() ||----------------------------------------");
        // Access the internal fields of the Collector safely
        let me = self.project();

        // Store the current waker
        me.producer.store_waker(cx.waker());

        match *me.status {
            // Handle case where all data is collected but no condition match
            Collector::<T,P>::NUM_DATA => {
                println!("READY, NO MATCH: {}", *me.num);
                // Stop the Producer
                me.producer.stop();
                // Return the Result
                Poll::Ready(me.result.clone())
            }
            // Handle case where collection of data NOT Ready
            _ => {
                // poll_step!(me);
                // if let Some(new_id) = check_thread_id!(me) {
                //     *me.thread_id = Some(new_id);
                // }
                let new_thread_id = std::thread::current().id();
                if *me.thread_id != Some(new_thread_id) {
                    if let Some(stored_thread_id) = *me.thread_id {
                        println!("------------------> Thread ID Changed: {} from/to {:?}/{:?}",
                                me.num, stored_thread_id, new_thread_id);
                    }
                    *me.thread_id = Some(new_thread_id);
                } 

                // if new_data_and_check_ready!(me) {
                //     println!("MATCH {} Steps: {}", me.num, me.status);
                //     return Poll::Ready(me.result.clone());
                // } else {
                //     return Poll::Pending;
                // }
                // match new_data_and_check_ready!(me) {
                //     true => {
                //         println!("self.num = {}, self.status  = {}", me.num, me.status);
                //         // println!("MATCH {} Steps: {}", me.num, me.status);
                //         return Poll::Ready(me.result.clone());
                //     },
                //     false => {
                //         return Poll::Pending;
                //     }
                // }
                if !me.producer.data_available() {
                    Poll::Pending
                } else {
                    // produce new data
                    let data = me.producer.produce();
                    // store the produced data
                    me.result.push(data);
                    // update the future's sum
                    *me.sum += data;
                    // increment future's status
                    *me.status += 1;
        
                    // check ready-condition
                    // if ready_condition!($self) {
                    //     // stop if the ready condition is met
                    //     $self.producer.stop();
                    //     true
                    // } else {
                    //     false
                    // }
                    println!("Collector::poll() --> sum = {:?}, status(count) = {}", me.sum, me.status);
                    if *me.sum % P::from(17) == P::from(0) {
                        println!("Collector::poll() ===|| STOP ===>> sum = {:?}, status(count) = {}", me.sum, me.status);
                    // if *me.sum % P::from(20) == P::from(0) {
                        // stop if the ready condition is met
                        me.producer.stop();
                        Poll::Ready(me.result.clone())
                    } else {
                        Poll::Pending
                    }
                }
            }
        }
    }
}



// Main function using Tokio runtime to collect data concurrently
// #[tokio::main(flavor = "multi_thread", worker_threads = 64)]
#[tokio::main(flavor = "multi_thread", worker_threads = 64)]
async fn main() {
    println!("Hello, world!");
    let mut tasks:  Vec<tokio::task::JoinHandle<()>> = Vec::new();

    // for i in 0..500 {
    for i in 0..500 {
        let task: tokio::task::JoinHandle<()> = tokio::spawn(async move {
            match i % 3 {
                0 => {
                    //--- RandProducer for i16
                    let p: RandProducer<i16> = RandProducer::new();
                    // let collector = Collector::new(p, i);
                    let collector: Collector<RandProducer<i16>, i16> = Collector::new(p, i);
                    println!("Collector<RandProducer<_>,_>.await --> {}", i);
                    let _res = collector.await;
                    println!("[**]--- Thread[{}] EXITED - MAIN::RandProducer: {:?}", i, _res);
                },
                1 => {
                    //--- ChannelProducer for u16
                    let p: ChannelProducer<i16> = ChannelProducer::new(i);
                    // let p: ChannelProducer<i16> = ChannelProducer::new();
                    let collector = Collector::new(p, i);
                    // let collector: Collector<ChannelProducer<u16>, _> = Collector::new(p, i);
                    println!("Collector<ChannelProducer<_>,_>.await --> {}", i);
                    let _res = collector.await;
                    println!("[**]--- Thread[{}] EXITED - MAIN::ChannelProducer: {:?}", i, _res);
                },
                _ => {
                    //--- TcpProducer for u64
                    let addr = format!("127.0.0.1:{}", 7800 + i);
                    // let p: TcpProducer<u64> = TcpProducer::new(addr);
                    let p: TcpProducer<i32> = TcpProducer::new(addr);
                    // let collector = Collector::new(p, i);
                    let collector: Collector<TcpProducer<i32>, _> = Collector::new(p, i);
                    println!("Collector<TcpProducer<_>,_>.await --> {}", i);
                    let _res = collector.await;
                    println!("[**]--- Thread[{}] EXITED -  MAIN::TcpProducer: {:?}", i, _res);
                }
            }
        });
        tasks.push(task);
    }

    for task in tasks {
        let _ = task.await;
    }

    println!("All tasks completed");
}
