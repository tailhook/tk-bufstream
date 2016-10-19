#[warn(missing_docs)]

extern crate netbuf;
extern crate futures;
extern crate tokio_core;

pub use netbuf::Buf;
pub use iobuf::IoBuf;

mod iobuf;

