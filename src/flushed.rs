use std::io;

use futures::{Future, Poll, Async};
use tokio_core::io::Io;

use {IoBuf};

/// A future which yields the original stream when output buffer is fully
/// written to the socket
pub struct Flushed<S: Io>(Option<IoBuf<S>>);


impl<S: Io> Future for Flushed<S> {
    type Item = IoBuf<S>;
    type Error = io::Error;
    fn poll(&mut self) -> Poll<IoBuf<S>, io::Error> {
        if let Some(ref mut conn) = self.0 {
            try!(conn.flush());
            if conn.out_buf.len() > 0 && !conn.done() {
                return Ok(Async::NotReady);
            }
        }
        Ok(Async::Ready(self.0.take().unwrap()))
    }
}

pub fn flushed<S: Io>(sock: IoBuf<S>) -> Flushed<S> {
    Flushed(Some(sock))
}
