use std::io::{self, Read, Write};
use tokio_core::io::Io;
/// A thing that implements tokio_core::io::Io but never ready
///
/// Useful for tests
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

impl Io for Mock {}
