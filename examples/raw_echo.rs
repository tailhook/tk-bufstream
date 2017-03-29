extern crate futures;
extern crate tokio_io;
extern crate tokio_core;
extern crate tk_bufstream;
extern crate tokio_service;

use std::io;
use std::str;
use std::io::Write;
use std::net::SocketAddr;
use std::env;

use futures::{Future, Poll, Async};
use futures::future::{FutureResult, ok};
use futures::stream::Stream;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;
use tokio_service::Service;
use tk_bufstream::IoBuf;

struct LineProto<T, S: AsyncRead+AsyncWrite>
    where T: Service<Request=String, Response=String, Error=io::Error>,
{
    io: IoBuf<S>,
    service: T,
    in_flight: Option<T::Future>,
}

struct LineService;


impl<T, S: AsyncRead+AsyncWrite> LineProto<T, S>
    where T: Service<Request=String, Response=String, Error=io::Error>,
{
    fn new(socket: S, service: T) -> LineProto<T, S> {
        LineProto {
            io: IoBuf::new(socket),
            service: service,
            in_flight: None,
        }
    }
}

impl<T, S: AsyncRead+AsyncWrite> Future for LineProto<T, S>
    where T: Service<Request=String, Response=String, Error=io::Error>,
{
    type Item = ();
    type Error = io::Error;
    fn poll(&mut self) -> Poll<(), io::Error> {
        self.io.flush()?;
        loop {
            if let Some(mut future) = self.in_flight.take() {
                match future.poll()? {
                    Async::Ready(value) => {

                        // This is how we emulate a protocol serializer
                        writeln!(&mut self.io.out_buf, "Echo: {}", value)
                            .expect("buffer write never fails");

                        self.io.flush()?;
                    }
                    Async::NotReady => {
                        self.in_flight = Some(future);
                        return Ok(Async::NotReady);
                    }
                }
            }
            let endline = self.io.in_buf[..].iter().position(|&x| x == b'\n');
            if let Some(pos) = endline {
                let chunk = self.io.in_buf[..pos].to_vec();
                self.io.in_buf.consume(pos+1);  // consume together with '\n'
                // Only process valid utf-8 "requests"
                if let Ok(line) = String::from_utf8(chunk) {
                    self.in_flight = Some(self.service.call(line));
                    continue;
                }
            } else {
                let nbytes = self.io.read()?;
                if nbytes == 0 {
                    if self.io.done() {
                        return Ok(Async::Ready(()));
                    } else {
                        return Ok(Async::NotReady);
                    }
                }
            }
        }
    }
}

impl Service for LineService {
    type Request = String;
    type Response = String;
    type Error = io::Error;
    type Future = FutureResult<String, io::Error>;

    fn call(&self, input: String) -> Self::Future {
        // To emulate some useful service we trim and replace
        // all spaces into pluses
        ok(input.trim().replace(" ", "+"))
    }
}

fn main() {
    let addr = env::args().nth(1).unwrap_or("127.0.0.1:7777".to_string());
    let addr = addr.parse::<SocketAddr>().unwrap();

    let mut lp = Core::new().unwrap();
    let handle = lp.handle();
    let socket = TcpListener::bind(&addr, &handle).unwrap();
    println!("Listening on: {}", addr);

    let done = socket.incoming().for_each(|(socket, _addr)| {
        handle.spawn(
            LineProto::new(socket, LineService)
            .map_err(|e| println!("Connection error: {}", e))
        );
        Ok(())
    });

    lp.run(done).unwrap();
}

