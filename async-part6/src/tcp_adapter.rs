use futures::stream::Stream;
use pin_project::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, BufReader, ReadBuf};
use tokio::net::TcpStream;

#[pin_project]
pub struct TcpAdapter {
    #[pin]
    tcp_stream: BufReader<TcpStream>,
    buffer: Vec<u8>,
}

impl TcpAdapter {
    #[allow(dead_code)]
    pub fn new(tcp_stream: TcpStream) -> Self {
        Self { 
            tcp_stream: BufReader::new(tcp_stream), 
            buffer: vec![0; 4  * 1024],
        }
    }
}

impl Stream for TcpAdapter {
    type Item = Vec<u8>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        let mut read_buf = ReadBuf::new(this.buffer);

        match this.tcp_stream.poll_read(cx, &mut read_buf) {
            Poll::Ready(Ok(_)) => {
                if read_buf.filled().is_empty() {
                    return Poll::Ready(None);
                } else {
                    let chunk = read_buf.filled().to_vec();
                    Poll::Ready(Some(chunk))
                }
            },
            Poll::Ready(Err(e)) => {
                log::error!("Error reading from stream: {:?}", e);
                return Poll::Ready(Some(vec![]));
            },
            Poll::Pending => {
                Poll::Pending
            }
        }
    }
}
