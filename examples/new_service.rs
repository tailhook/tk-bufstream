extern crate futures;
extern crate tokio_core;
extern crate tk_bufstream;

use std::io;
use std::str;
use std::io::Write;
use std::net::SocketAddr;
use std::time::Duration;
use std::env;

use futures::Future;
use futures::stream::Stream;
use tokio_core::net::TcpListener;
use tokio_core::reactor::{Core, Timeout};
use tk_bufstream::{Buf, IoBuf, Encode, Decode};

const MAX_CONNECTIONS: usize = 200;

struct Codec;

impl Encode for Codec {
    type Item = String;
    fn encode(&mut self, data: String, buf: &mut Buf) {
        writeln!(buf, "{}", &data).unwrap();
    }
}

impl Decode for Codec {
    type Item = String;
    fn decode(&mut self, buf: &mut Buf) -> Result<Option<String>, io::Error> {
        if let Some(end) = buf[..].iter().position(|&x| x == b'\n') {
            let s = str::from_utf8(&buf[..end])
                .map(|v| String::from(v))
                .map_err(|_| io::Error::new(io::ErrorKind::Other,
                                            "can't decode utf-8"))?;
            buf.consume(end+1);
            Ok(Some(s))
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
            let (sink, stream) = IoBuf::new(socket)
                .framed(Codec).split();
            stream
            .map(|l| {
                Timeout::new(Duration::new(5, 0), &handle).unwrap()
                .map(move |()| format!("after timeout: {}", l))
            })
            .buffered(5)
            .forward(sink)
            .map(|_| ())
            .map_err(|e| { println!("Connection error: {}", e); })
        }).buffer_unordered(MAX_CONNECTIONS)
          .for_each(|()| Ok(()));
    lp.run(done).unwrap();
}

