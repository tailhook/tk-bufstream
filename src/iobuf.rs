use std::io;
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, RawFd};

use tokio_core::io::Io;

use flushed::{Flushed, flushed};
use frame;
use split;
use {Buf, Framed, Encode, Decode};

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
            Err(ref e) if e.kind() == io::ErrorKind::BrokenPipe ||
                          e.kind() == io::ErrorKind::ConnectionReset
            => {
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

    /// Provides a `Stream` and `Sink` interface for reading and writing to
    /// this `IoBuf` object, using `Decode` and `Encode` to read and write the
    /// raw data.
    ///
    /// Raw I/O objects work with byte sequences, but higher-level code
    /// usually wants to batch these into meaningful chunks, called "frames".
    /// This method layers framing on top of an I/O object, by using the
    /// `Encode` and `Decode` traits:
    ///
    /// - `Encode` interprets frames we want to send into bytes;
    /// - `Decode` interprets incoming bytes into a stream of frames.
    ///
    /// Note that the incoming and outgoing frame types may be distinct.
    ///
    /// This function returns a *single* object that is both `Stream` and
    /// `Sink`; grouping this into a single object is often useful for
    /// layering things like gzip or TLS, which require both read and write
    /// access to the underlying object.
    ///
    /// If you want to work more directly with the streams and sink, consider
    /// calling `split` on the `Framed` returned by this method, which will
    /// break them into separate objects, allowing them to interact more
    /// easily.
    pub fn framed<C: Encode + Decode>(self, codec: C) -> Framed<S, C>
        where Self: Sized,
    {
        frame::framed(self, codec)
    }

    pub fn split(self) -> (split::WriteBuf<S>, split::ReadBuf<S>) {
        split::create(self.in_buf, self.out_buf, self.socket, self.done)
    }
}

#[cfg(unix)]
impl<S: AsRawFd + Io> AsRawFd for IoBuf<S> {
    fn as_raw_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }
}

impl<S: Io> io::Write for IoBuf<S> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        // TODO(tailhook) may try to write to the buf directly if
        // output buffer is empty
        self.out_buf.write(buf)?;
        self.flush()?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> Result<(), io::Error> {
        self.flush()
    }
}
