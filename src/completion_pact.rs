extern crate tokio_core;
extern crate futures;

use tokio_core::reactor::Core;
use futures::sync::mpsc::unbounded;
use tokio_core::net::TcpListener;
use std::net::SocketAddr;
use std::str::FromStr;
use futures::{Async, Stream, Future, Poll};
use std::thread;
use std::time::Duration;

pub struct CompletionPact<S, C>
where
    S: Stream,
    C: Stream,
{
    pub stream: S,
    pub completer: C,
}

pub fn stream_completion_pact<S, C>(s: S, c: C) -> CompletionPact<S, C>
where
    S: Stream,
    C: Stream,
{
    CompletionPact {
        stream: s,
        completer: c,
    }
}

impl<S, C> Stream for CompletionPact<S, C>
where
    S: Stream,
    C: Stream,
{
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<S::Item>, S::Error> {
        match self.completer.poll() {
            Ok(Async::Ready(None)) |
            Err(_) |
            Ok(Async::Ready(Some(_))) => Ok(Async::Ready(None)),
            Ok(Async::NotReady) => self.stream.poll(),
        }
    }
}
