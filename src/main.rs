extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_proto;
extern crate bytes;

mod mqtt;
mod mqtt_net;

use futures::{Future, Stream};
use tokio_io::{io, AsyncRead};
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;

fn main() {
    start_non_secure(1883);
}

fn start_non_secure(port: u16) {
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let addr = "0.0.0.0:1883".parse().unwrap();
    let tcp = TcpListener::bind(&addr, &handle).unwrap();

    let server = tcp.incoming().for_each(|(tcp, addr)| {
        Ok(())
    });

    core.run(server).unwrap();
}
