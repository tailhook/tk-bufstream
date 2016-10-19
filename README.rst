============================================
Buffered Stream Abstraction for Tokio (Rust)
============================================

:Status: Alpha

Unlike in synchronous code, where buffered stream are very common, in async
code buffers are usually put in some glue code (I guess it's called Transport
in tokio-proto_).

This crate explores alternative idea of tying buffers (and some helpers for
making transports) to IO Stream, which might make `certain cases`_ easier.

.. _tokio-proto: https://github.com/tokio-rs/tokio-proto
.. _certain cases: https://github.com/popravich/minihttp/blob/333992f71b0abe31d2222345e64e6dbc07d60a2b/examples/sendfile.rs#L44-L46

License
=======

Licensed under either of

* Apache License, Version 2.0,
  (./LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license (./LICENSE-MIT or http://opensource.org/licenses/MIT)
  at your option.

Contribution
------------

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

