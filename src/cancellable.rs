extern crate futures;
use std::io::{Error, ErrorKind};
use futures::{Async, Stream, Future, Poll};

pub struct CancellableIoFuture<S>
where 
    S: Future<Error=Error>,
{
    pub future: S,
    cancel: bool,
}

impl CancellableIoFuture<S>
{
    pub fn request_cancellation(&self) {
        &self.cancel = true;
    }
}

pub fn cancellable_io_future<S>(source: S) -> CancellableIoFuture<S> 
where 
    S: Future<Error=Error>,
{
    CancellableIoFuture {
        future: source,
        cancel: false,
    }
}

impl<S> Future for CancellableIoFuture<S>
where
    S: Future<Error=Error>,
{
    type Item = S::Item;
    type Error = Error;

    fn poll(&mut self) -> Poll<S::Item, Error> {
        match self.cancel {
            false => self.future.poll(),
            true => Err(Error::new(ErrorKind::Interrupted, "Cancelled by user"))
        }
    }
}
