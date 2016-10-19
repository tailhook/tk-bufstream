use std::io;

use {Buf};

/// Main trait of a stream (meaning socket) with input and output buffers
///
/// This is ought to be similar to `tokio_core::Io` but with buffers
trait BufIo {
    /// Returns both input and output bufers at the same time
    ///
    /// It's useful to avoid some lifetime constraints otherwise you should use
    /// `in_buf()` and `out_buf()`
    ///
    /// First buffer is input buffer (where data ,which is read from socket,
    /// is put). Second buffer is the output buffer (where data to write to
    /// socket is buffered).
    fn buffers(&mut self) -> (&mut Buf, &mut Buf);

    /// Returns just an input buffer
    fn in_buf(&mut self) -> &mut Buf { self.buffers().0 }

    /// Returns just an output buffer
    fn out_buf(&mut self) -> &mut Buf { self.buffers().1 }

    /// Read a chunk of data into a buffer
    ///
    /// The data just read can then be found in `self.in_buf()`
    fn read(&mut self) -> Result<usize, io::Error>;

    /// Write data in the output buffer to actual stream
    ///
    /// You should put the data to be sent into `self.out_buf()` before flush
    fn flush(&mut self) -> Result<usize, io::Error>;
}
