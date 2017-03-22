use std::cmp::min;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};

use netbuf::RangeArgument;
use tokio_io::{AsyncRead, AsyncWrite};

/// A thing that implements tokio_core::io::Io but never ready
///
/// Useful for tests on codecs, where tests owns actual buffered stream.
pub struct Mock;

impl Read for Mock {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::WouldBlock, "No read"))
    }
}

impl Write for Mock {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::WouldBlock, "No write"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::WouldBlock, "No flush"))
    }
}

impl AsyncRead for Mock {}
impl AsyncWrite for Mock {}


/// A mock stream where you can push data to/from
///
/// Useful for more complex tests on streams than are possible with `Mock`
#[derive(Clone)]
pub struct MockData {
    input: Arc<Mutex<Vec<u8>>>,
    output: Arc<Mutex<Vec<u8>>>,
}

impl MockData {
    /// New, empty Mockdata
    pub fn new() -> MockData {
        MockData {
            input: Arc::new(Mutex::new(Vec::new())),
            output: Arc::new(Mutex::new(Vec::new())),
        }
    }
    /// Add some bytes to the next input. This data will be read by IoBuf
    /// on thext `read()` call
    pub fn add_input<D: AsRef<[u8]>>(&self, data: D) {
        self.input.lock().unwrap().extend(data.as_ref())
    }

    /// Get slice of output
    ///
    /// Note: we only read bytes that has been `write`'en here, not
    /// the bytes buffered inside `IoBuf`.
    ///
    /// Note 2: we use RangeArgument from `netbuf` here, as we already depend
    /// on netbuf anyway. Eventually we will switch to `RangeArgument` from
    /// `std` (both in `netbuf` and here) when latter is stable.
    pub fn output<T: Into<RangeArgument>>(&self, range: T) -> Vec<u8> {
        let buf = self.output.lock().unwrap();
        use netbuf::RangeArgument::*;
        match range.into() {
            RangeTo(x) => buf[..x].to_vec(),
            RangeFrom(x) => buf[x..].to_vec(),
            Range(x, y) => buf[x..y].to_vec(),
        }
    }

    /// Get first bytes of output and remove them from output buffer
    ///
    /// Note: we only read bytes that has been `write`'en here, not
    /// the bytes buffered inside `IoBuf`.
    pub fn get_output(&self, num: usize) -> Vec<u8> {
        let mut buf = self.output.lock().unwrap();
        return buf.drain(..num).collect();
    }
}

impl Read for MockData {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut inp = self.input.lock().unwrap();
        let bytes = min(buf.len(), inp.len());
        if bytes == 0 {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        buf[..bytes].copy_from_slice(&inp[..bytes]);
        inp.drain(..bytes);
        return Ok(bytes);
    }
}

impl Write for MockData {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut out = self.output.lock().unwrap();
        out.extend(buf);
        return Ok(buf.len());
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsyncRead for MockData {}
impl AsyncWrite for MockData {}


#[cfg(test)]
mod test {

    use {IoBuf, Mock, MockData};

    #[test]
    fn mock() {
        let mut buf = IoBuf::new(Mock);
        buf.read().ok();
        assert_eq!(&buf.in_buf[..], b"");
        buf.out_buf.extend(b"hello");
        assert_eq!(&buf.out_buf[..], b"hello");
        buf.flush().ok(); // never ready
        assert_eq!(&buf.out_buf[..], b"hello");
    }

    #[test]
    fn mock_data() {
        let data = MockData::new();
        let mut buf = IoBuf::new(data.clone());
        buf.read().ok();
        assert_eq!(&buf.in_buf[..], b"");
        data.add_input("test me");
        buf.read().ok();
        assert_eq!(&buf.in_buf[..], b"test me");

        buf.out_buf.extend(b"hello");
        assert_eq!(&buf.out_buf[..], b"hello");
        buf.flush().ok();
        assert_eq!(&buf.out_buf[..], b"");
        assert_eq!(&data.output(..), b"hello");
    }

}
