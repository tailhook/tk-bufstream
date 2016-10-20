use std::io;

use tokio_core::io::Io;

use flushed::{Flushed, flushed};
use {Buf};

/// A wrapper for full-duplex stream
pub struct IoBuf<S: Io> {
    pub in_buf: Buf,
    pub out_buf: Buf,
    socket: S,
    done: bool,
}

/// Main trait of a stream (meaning socket) with input and output buffers
///
/// This is ought to be similar to `tokio_core::Io` but with buffers
impl<S: Io> IoBuf<S> {
    /// Create a new IoBuf object with empty buffers
    pub fn new(sock: S) -> IoBuf<S> {
        IoBuf {
            in_buf: Buf::new(),
            out_buf: Buf::new(),
            socket: sock,
            done: false,
        }
    }
    /// Read a chunk of data into a buffer
    ///
    /// The data just read can then be found in `self.in_buf`.
    ///
    /// This method does just one read. Because you are ought to try parse
    /// request after every read rather than reading a lot of the data in
    /// memory.
    ///
    /// This method returns `0` when no bytes are read, both when WouldBlock
    /// occurred and when connection has been closed. You may then use
    /// `self.done()` to distinguish from these conditions.
    pub fn read(&mut self) -> Result<usize, io::Error> {
        match self.in_buf.read_from(&mut self.socket) {
            Ok(0) => {
                self.done = true;
                Ok(0)
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
            Err(ref e)
                if e.kind() == io::ErrorKind::BrokenPipe ||
                   e.kind() == io::ErrorKind::ConnectionReset
            => {
                self.done = true;
                Ok(0)
            }
            result => result,
        }
    }

    /// Write data in the output buffer to actual stream
    ///
    /// You should put the data to be sent into `self.out_buf` before flush
    pub fn flush(&mut self) -> Result<(), io::Error> {
        loop {
            if self.out_buf.len() == 0 {
                break;
            }
            match self.out_buf.write_to(&mut self.socket) {
                Ok(0) => break,
                Ok(_) => continue,
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    break;
                }
                Err(ref e)
                    if e.kind() == io::ErrorKind::BrokenPipe ||
                       e.kind() == io::ErrorKind::ConnectionReset
                => {
                    self.done = true;
                    break;
                }
                Err(e) => {
                    return Err(e);
                },
            }
        }
        // This probably aways does nothing, but we have to support the full
        // Io protocol
        match self.socket.flush() {
            Ok(()) => Ok(()),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(()),
            Err(ref e) if e.kind() == io::ErrorKind::BrokenPipe => {
                self.done = true;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Returns true when connection is closed by peer
    pub fn done(&self) -> bool {
        return self.done;
    }

    /// Returns a future which resolves to this stream when output buffer is
    /// flushed
    pub fn flushed(self) -> Flushed<S> {
        flushed(self)
    }
}
