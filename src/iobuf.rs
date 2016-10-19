use std::io;

use tokio_core::io::Io;

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
    /// Read a chunk of data into a buffer
    ///
    /// The data just read can then be found in `self.in_buf()`
    pub fn read(&mut self) -> Result<usize, io::Error> {
        match self.in_buf.read_from(&mut self.socket) {
            Ok(0) => {
                self.done = true;
                Ok(0)
            }
            result => result,
        }
    }

    /// Write data in the output buffer to actual stream
    ///
    /// You should put the data to be sent into `self.out_buf()` before flush
    pub fn flush(&mut self) -> Result<(), io::Error> {
        if self.in_buf.len() > 0 {
            loop {
                match self.in_buf.write_to(&mut self.socket) {
                    Ok(_) => break,
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock
                    => {
                        break;
                    }
                    Err(e) => {
                        return Err(e);
                    },
                }
            }
        }
        // This probably aways does nothing, but we have to support the full
        // Io protocol
        self.socket.flush()
    }
}
