extern crate futures;
extern crate tokio_core;
extern crate tk_bufstream;

use std::io;
use std::str;
use std::io::Write;
use std::net::SocketAddr;
use std::env;

use futures::{Future, Sink};
use futures::stream::Stream;
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;
use tk_bufstream::{Buf, IoBuf, Encode, Decode};

const MAX_CONNECTIONS: usize = 200;

struct Line(String);

impl Encode for Line {
    fn encode(self, buf: &mut Buf) {
        writeln!(buf, "{}", &self.0).unwrap();
    }
}

impl Decode for Line {
    fn decode(buf: &mut Buf) -> Result<Option<Self>, io::Error> {
        if let Some(end) = buf[..].iter().position(|&x| x == b'\n') {
            let s = try!(str::from_utf8(&buf[..end])
                .map(|v| String::from(v))
                .map_err(|_| io::Error::new(io::ErrorKind::Other,
                                            "can't decode utf-8")));
            buf.consume(end+1);
            Ok(Some(Line(s)))
        } else {
            Ok(None)
        }
    }
}

fn main() {
    let addr = env::args().nth(1).unwrap_or("127.0.0.1:7777".to_string());
    let addr = addr.parse::<SocketAddr>().unwrap();

    let mut lp = Core::new().unwrap();
    let handle = lp.handle();
    let socket = TcpListener::bind(&addr, &handle).unwrap();
    println!("Listening on: {}", addr);

    let done = socket.incoming()
        .map_err(|e| { println!("Accept error: {}", e); })
        .map(|(socket, _addr)| {
            let (stream, sink) = IoBuf::new(socket)
                .framed::<Line, Line>().split();
            sink.send_all(stream)
                .map(|_| ())
                .map_err(|e| { println!("Connection error: {}", e); })
        }).buffer_unordered(MAX_CONNECTIONS)
          .for_each(|()| Ok(()));
    lp.run(done).unwrap();
}

