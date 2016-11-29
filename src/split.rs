use std::io;

use tokio_core::io::Io;

use futures::Async;
use futures::sync::BiLock;
use {Buf};

struct Shared<S> {
    socket: S,
    done: bool,
}

/// An input counterpart of IoBuf when the latter is split
pub struct ReadBuf<S> {
    pub in_buf: Buf,
    shared: BiLock<Shared<S>>,
}

/// An output counterpart of IoBuf when the latter is split
pub struct WriteBuf<S> {
    pub out_buf: Buf,
    shared: BiLock<Shared<S>>,
}

pub fn create<S>(in_buf: Buf, out_buf: Buf, socket: S, done: bool)
    -> (WriteBuf<S>, ReadBuf<S>)
{
    let (a, b) = BiLock::new(Shared {
        socket: socket,
        done: done,
    });
    return (
        WriteBuf {
            out_buf: in_buf,
            shared: b,
        },
        ReadBuf {
            in_buf: out_buf,
            shared: a,
        });
}

impl<S: Io> ReadBuf<S> {
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
    ///
    /// Note: this method silently assumes that you will call it on poll
    /// every time until self.done() returns false. I.e. it behaves as Async
    /// method but does't return Async value to allow simpler handling
    pub fn read(&mut self) -> Result<usize, io::Error> {
        if let Async::Ready(ref mut s) = self.shared.poll_lock() {
            match self.in_buf.read_from(&mut s.socket) {
                Ok(0) => {
                    s.done = true;
                    Ok(0)
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
                Err(ref e)
                    if e.kind() == io::ErrorKind::BrokenPipe ||
                       e.kind() == io::ErrorKind::ConnectionReset
                => {
                    s.done = true;
                    Ok(0)
                }
                result => result,
            }
        } else {
            Ok(0)
        }
    }

    /// Returns true when connection is closed by peer
    ///
    /// Note: this method returns false and schedules a wakeup if connecion
    /// is currently locked
    pub fn done(&self) -> bool {
        if let Async::Ready(ref mut s) = self.shared.poll_lock() {
            return s.done;
        } else {
            return false;
        }
    }
}

impl<S: Io> WriteBuf<S> {
    /// Write data in the output buffer to actual stream
    ///
    /// You should put the data to be sent into `self.out_buf` before flush
    ///
    /// Note: this method silently assumes that you will call it on poll
    /// every time until self.done() returns false. I.e. it behaves as Async
    /// method but does't return Async value to allow simpler handling
    pub fn flush(&mut self) -> Result<(), io::Error> {
        if let Async::Ready(ref mut s) = self.shared.poll_lock() {
            loop {
                if self.out_buf.len() == 0 {
                    break;
                }
                match self.out_buf.write_to(&mut s.socket) {
                    Ok(0) => break,
                    Ok(_) => continue,
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        break;
                    }
                    Err(ref e)
                        if e.kind() == io::ErrorKind::BrokenPipe ||
                           e.kind() == io::ErrorKind::ConnectionReset
                    => {
                        s.done = true;
                        break;
                    }
                    Err(e) => {
                        return Err(e);
                    },
                }
            }
            // This probably always does nothing, but we have to support the
            // full Io protocol
            match s.socket.flush() {
                Ok(()) => Ok(()),
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(()),
                Err(ref e) if e.kind() == io::ErrorKind::BrokenPipe ||
                              e.kind() == io::ErrorKind::ConnectionReset
                => {
                    s.done = true;
                    Ok(())
                }
                Err(e) => Err(e),
            }
        } else {
            Ok(())
        }
    }

    /// Returns true when connection is closed by peer
    ///
    /// Note: this method returns false and schedules a wakeup if connecion
    /// is currently locked
    pub fn done(&self) -> bool {
        if let Async::Ready(ref mut s) = self.shared.poll_lock() {
            return s.done;
        } else {
            return false;
        }
    }
}
