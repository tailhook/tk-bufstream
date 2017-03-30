use std::io;
use std::fmt;
use std::mem;
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, RawFd};

use futures::{Async, Future, Poll};
use futures::sync::{BiLock, BiLockAcquired, BiLockAcquire};
use tokio_io::{AsyncRead, AsyncWrite};

use frame;
use {Buf, Encode, Decode, ReadFramed, WriteFramed};

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

/// A structure that locks IoBuf and allows you to write to the socket directly
///
/// Where "directly" means without buffering and presumably with some zero-copy
/// method like `sendfile()` or `splice()`
///
/// Note: when `WriteRaw` is alive `ReadBuf` is alive, but locked and will
/// wake up as quick as `WriteRaw` is converted back to `WriteBuf`.
pub struct WriteRaw<S> {
    io: BiLockAcquired<Shared<S>>,
}

/// A future which converts `WriteBuf` into `WriteRaw`
pub struct FutureWriteRaw<S>(WriteRawFutState<S>);

enum WriteRawFutState<S> {
    Flushing(WriteBuf<S>),
    Locking(BiLockAcquire<Shared<S>>),
    Done,
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

impl<S> ReadBuf<S> {
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
    pub fn read(&mut self) -> Result<usize, io::Error>
        where S: AsyncRead
    {
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

    pub fn framed<D: Decode>(self, codec: D) -> ReadFramed<S, D> {
        frame::read_framed(self, codec)
    }
}

impl<S> WriteBuf<S> {
    /// Write data in the output buffer to actual stream
    ///
    /// You should put the data to be sent into `self.out_buf` before flush
    ///
    /// Note: this method silently assumes that you will call it on poll
    /// every time until self.done() returns false. I.e. it behaves as Async
    /// method but does't return Async value to allow simpler handling
    pub fn flush(&mut self) -> Result<(), io::Error>
        where S: AsyncWrite
    {
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

    /// Returns a future which will resolve into WriteRaw
    ///
    /// This future resolves when after two conditions:
    ///
    /// 1. Output buffer is fully flushed to the network (i.e. OS buffers)
    /// 2. Internal BiLock is locked
    ///
    /// Note: `WriteRaw` will lock the underlying stream for the whole
    /// lifetime of the `WriteRaw`.
    pub fn borrow_raw(self) -> FutureWriteRaw<S> {
        if self.out_buf.len() == 0 {
            FutureWriteRaw(WriteRawFutState::Locking(self.shared.lock()))
        } else {
            FutureWriteRaw(WriteRawFutState::Flushing(self))
        }
    }

    pub fn framed<E: Encode>(self, codec: E) -> WriteFramed<S, E> {
        frame::write_framed(self, codec)
    }
}

impl<S> WriteRaw<S> {
    /// Turn raw writer back into buffered and release internal BiLock
    pub fn into_buf(self) -> WriteBuf<S> {
        WriteBuf {
            out_buf: Buf::new(),
            shared: self.io.unlock(),
        }
    }
}

impl<S: AsyncWrite> Future for FutureWriteRaw<S> {
    type Item = WriteRaw<S>;
    type Error = io::Error;
    fn poll(&mut self) -> Poll<WriteRaw<S>, io::Error> {
        use self::WriteRawFutState::*;
        self.0 = match mem::replace(&mut self.0, Done) {
            Flushing(mut buf) => {
                buf.flush()?;
                if buf.out_buf.len() == 0 {
                    let mut lock = buf.shared.lock();
                    match lock.poll().expect("lock never fails") {
                        Async::Ready(s) => {
                            return Ok(Async::Ready(WriteRaw { io: s }));
                        }
                        Async::NotReady => {}
                    }
                    Locking(lock)
                } else {
                    Flushing(buf)
                }
            }
            Locking(mut f) => {
                match f.poll().expect("lock never fails") {
                    Async::Ready(s) => {
                        return Ok(Async::Ready(WriteRaw { io: s }));
                    }
                    Async::NotReady => {}
                }
                Locking(f)
            }
            Done => panic!("future polled after completion"),
        };
        return Ok(Async::NotReady);
    }
}

impl<S: AsyncWrite> io::Write for WriteRaw<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.io.socket.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.io.socket.flush()
    }
}

#[cfg(unix)]
impl<S: AsRawFd> AsRawFd for WriteRaw<S> {
    fn as_raw_fd(&self) -> RawFd {
        self.io.socket.as_raw_fd()
    }
}

impl<S: AsyncRead> fmt::Debug for ReadBuf<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ReadBuf {{ in: {}b }}", self.in_buf.len())
    }
}

impl<S: AsyncWrite> fmt::Debug for WriteBuf<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "WriteBuf {{ out: {}b }}", self.out_buf.len())
    }
}
