// This module contains some code from tokio-core/src/io/framed.rs
use std::io;

use futures::{Async, Poll, Stream, Sink, StartSend, AsyncSink};
use tokio_io::{AsyncRead, AsyncWrite};

use {IoBuf, WriteBuf, ReadBuf, Buf};


/// Decoding of a frame from an internal buffer.
///
/// This trait is used when constructing an instance of `Framed`. It defines how
/// to decode the incoming bytes on a stream to the specified type of frame for
/// that framed I/O stream.
///
/// The primary method of this trait, `decode`, attempts to decode a
/// frame from a buffer of bytes. It has the option of returning `NotReady`,
/// indicating that more bytes need to be read before decoding can
/// continue.
pub trait Decode: Sized {
    /// Decoded message
    type Item;
    /// Attempts to decode a frame from the provided buffer of bytes.
    ///
    /// This method is called by `Framed` whenever bytes are ready to be parsed.
    /// The provided buffer of bytes is what's been read so far, and this
    /// instance of `Decode` can determine whether an entire frame is in the
    /// buffer and is ready to be returned.
    ///
    /// If an entire frame is available, then this instance will remove those
    /// bytes from the buffer provided and return them as a decoded
    /// frame. Note that removing bytes from the provided buffer doesn't always
    /// necessarily copy the bytes, so this should be an efficient operation in
    /// most circumstances.
    ///
    /// If the bytes look valid, but a frame isn't fully available yet, then
    /// `Ok(None)` is returned. This indicates to the `Framed` instance that
    /// it needs to read some more bytes before calling this method again.
    ///
    /// Finally, if the bytes in the buffer are malformed then an error is
    /// returned indicating why. This informs `Framed` that the stream is now
    /// corrupt and should be terminated.
    fn decode(&mut self, buf: &mut Buf)
        -> Result<Option<Self::Item>, io::Error>;

    /// A default method available to be called when there are no more bytes
    /// available to be read from the underlying I/O.
    ///
    /// This method defaults to calling `decode` and returns an error if
    /// `Ok(None)` is returned. Typically this doesn't need to be implemented
    /// unless the framing protocol differs near the end of the stream.
    fn done(&mut self, buf: &mut Buf) -> io::Result<Self::Item> {
        match self.decode(buf)? {
            Some(frame) => Ok(frame),
            None => Err(io::Error::new(io::ErrorKind::Other,
                                       "bytes remaining on stream")),
        }
    }
}

/// A trait for encoding frames into a byte buffer.
///
/// This trait is used as a building block of `Framed` to define how frames are
/// encoded into bytes to get passed to the underlying byte stream. each
/// frame written to `Framed` will be encoded with this trait to an internal
/// buffer. That buffer is then written out when possible to the underlying I/O
/// stream.
pub trait Encode {
    /// Value to encode
    type Item: Sized;
    /// Encodes a frame into the buffer provided.
    ///
    /// This method will encode `msg` into the byte buffer provided by `buf`.
    /// The `buf` provided is an internal buffer of the `Framed` instance and
    /// will be written out when possible.
    fn encode(&mut self, value: Self::Item, buf: &mut Buf);
}

/// A unified `Stream` and `Sink` interface to an underlying `Io` object, using
/// the `Encode` and `Decode` traits to encode and decode frames.
pub struct Framed<T, C>(IoBuf<T>, C);

/// A `Stream` interface to `ReadBuf` object
pub struct ReadFramed<T, C>(ReadBuf<T>, C);

/// A `Sink` interface to `WriteBuf` object
pub struct WriteFramed<T, C>(WriteBuf<T>, C);

impl<T: AsyncRead, C: Decode> Stream for Framed<T, C> {
    type Item = C::Item;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, io::Error> {
        loop {
            if let Some(frame) = self.1.decode(&mut self.0.in_buf)? {
                return Ok(Async::Ready(Some(frame)));
            } else {
                let nbytes = self.0.read()?;
                if nbytes == 0 {
                    if self.0.done() {
                        return Ok(Async::Ready(None));
                    } else {
                        return Ok(Async::NotReady);
                    }
                }
            }
        }
    }
}

impl<T: AsyncWrite, C: Encode> Sink for Framed<T, C> {
    type SinkItem = C::Item;
    type SinkError = io::Error;

    fn start_send(&mut self, item: C::Item) -> StartSend<C::Item, io::Error> {
        self.1.encode(item, &mut self.0.out_buf);
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        self.0.flush()?;
        Ok(Async::Ready(()))
    }
}

pub fn framed<T, C>(io: IoBuf<T>, codec: C) -> Framed<T, C> {
    Framed(io, codec)
}

impl<T, C> Framed<T, C> {
    /// Returns a reference to the underlying I/O stream wrapped by `Framed`.
    pub fn get_ref(&self) -> &IoBuf<T> {
        &self.0
    }

    /// Returns a mutable reference to the underlying I/O stream wrapped by
    /// `Framed`.
    ///
    /// Note that care should be taken to not tamper with the underlying stream
    /// of data coming in as it may corrupt the stream of frames otherwise being
    /// worked with.
    pub fn get_mut(&mut self) -> &mut IoBuf<T> {
        &mut self.0
    }

    /// Consumes the `Framed`, returning its underlying I/O stream.
    ///
    /// Note that stream may contain both input and output data buffered.
    pub fn into_inner(self) -> IoBuf<T> {
        self.0
    }
}

impl<T: AsyncRead, C: Decode> Stream for ReadFramed<T, C> {
    type Item = C::Item;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, io::Error> {
        loop {
            if let Some(frame) = self.1.decode(&mut self.0.in_buf)? {
                return Ok(Async::Ready(Some(frame)));
            } else {
                let nbytes = self.0.read()?;
                if nbytes == 0 {
                    if self.0.done() {
                        return Ok(Async::Ready(None));
                    } else {
                        return Ok(Async::NotReady);
                    }
                }
            }
        }
    }
}

pub fn read_framed<T: AsyncRead, C>(io: ReadBuf<T>, codec: C)
    -> ReadFramed<T, C>
{
    ReadFramed(io, codec)
}

impl<T: AsyncRead, C> ReadFramed<T, C> {
    /// Returns a reference to the underlying I/O stream wrapped by `ReadFramed`.
    pub fn get_ref(&self) -> &ReadBuf<T> {
        &self.0
    }

    /// Returns a mutable reference to the underlying I/O stream wrapped by
    /// `ReadFramed`.
    ///
    /// Note that care should be taken to not tamper with the underlying stream
    /// of data coming in as it may corrupt the stream of frames otherwise being
    /// worked with.
    pub fn get_mut(&mut self) -> &mut ReadBuf<T> {
        &mut self.0
    }

    /// Consumes the `ReadFramed`, returning its underlying I/O stream.
    ///
    /// Note that stream may contain both input and output data buffered.
    pub fn into_inner(self) -> ReadBuf<T> {
        self.0
    }
}

impl<T: AsyncWrite, C: Encode> Sink for WriteFramed<T, C> {
    type SinkItem = C::Item;
    type SinkError = io::Error;

    fn start_send(&mut self, item: C::Item) -> StartSend<C::Item, io::Error> {
        self.1.encode(item, &mut self.0.out_buf);
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        self.0.flush()?;
        Ok(Async::Ready(()))
    }
}

pub fn write_framed<T, C>(io: WriteBuf<T>, codec: C) -> WriteFramed<T, C> {
    WriteFramed(io, codec)
}

impl<T: AsyncWrite, C> WriteFramed<T, C> {
    /// Returns a reference to the underlying I/O stream wrapped by `WriteFramed`.
    pub fn get_ref(&self) -> &WriteBuf<T> {
        &self.0
    }

    /// Returns a mutable reference to the underlying I/O stream wrapped by
    /// `WriteFramed`.
    ///
    /// Note that care should be taken to not tamper with the underlying stream
    /// of data coming in as it may corrupt the stream of frames otherwise being
    /// worked with.
    pub fn get_mut(&mut self) -> &mut WriteBuf<T> {
        &mut self.0
    }

    /// Consumes the `WriteFramed`, returning its underlying I/O stream.
    ///
    /// Note that stream may contain both input and output data buffered.
    pub fn into_inner(self) -> WriteBuf<T> {
        self.0
    }
}
