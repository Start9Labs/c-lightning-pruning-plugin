use std::ops::DerefMut;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use tokio::io::AsyncRead;
use tokio::stream::Stream;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RpcResponseState {
    NoNewLines,
    OneNewLine,
    TwoNewLines,
}

/// A byte stream that terminates when 2 consecutive newlines are received
pub struct RpcResponseStream<SP: DerefMut<Target = AR>, AR: AsyncRead> {
    pub inner: SP,
    pub state: RpcResponseState,
    pub buf: [u8; 4096],
}
impl<SP, AR> RpcResponseStream<SP, AR>
where
    SP: DerefMut<Target = AR>,
    AR: AsyncRead,
{
    pub fn new(sp: SP) -> Self {
        RpcResponseStream {
            inner: sp,
            state: RpcResponseState::NoNewLines,
            buf: [0; 4096],
        }
    }
}
impl<SP, AR> Stream for RpcResponseStream<SP, AR>
where
    SP: DerefMut<Target = AR> + std::marker::Unpin,
    AR: AsyncRead + std::marker::Unpin,
{
    type Item = Result<Vec<u8>, tokio::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let mut buf = self.buf;
        if self.state == RpcResponseState::TwoNewLines {
            return Poll::Ready(None);
        }
        let inner_pin: Pin<&mut AR> = Pin::new(self.inner.deref_mut());
        let bytes_read_poll = AsyncRead::poll_read(inner_pin, cx, &mut buf);
        let state = &mut self.state;
        let res = match bytes_read_poll {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(n)) => {
                let mut buf = &buf[..n];
                if buf.ends_with(b"\n\n") {
                    *state = RpcResponseState::TwoNewLines;
                    buf = &buf[..n - 2];
                } else if *state == RpcResponseState::OneNewLine && buf.starts_with(b"\n") {
                    buf = &buf[..n - 1];
                } else if buf.ends_with(b"\n") {
                    *state = RpcResponseState::OneNewLine;
                    buf = &buf[..n - 1];
                } else {
                    *state = RpcResponseState::NoNewLines;
                }
                Poll::Ready(Some(Ok(buf.to_owned())))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
        };
        self.buf = buf;
        res
    }
}
