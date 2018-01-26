extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_proto;
extern crate bytes;

mod mqtt;
mod cancellable;
mod logic;

use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use std::iter;
use std::net::Shutdown;
use std::io::{Error, ErrorKind, BufReader};

use cancellable::cancellable_io_future;
use logic::*;
use mqtt::reader::*;

use bytes::Bytes;
use futures::Future;
use futures::stream::{self, Stream};
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;
use tokio_io::io;
use tokio_io::*;

fn main() {
    start_non_secure();
}

fn start_non_secure() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let addr = "0.0.0.0:1883".parse().unwrap();
    let tcp = TcpListener::bind(&addr, &handle).unwrap();

    let connections = Rc::new(RefCell::new(HashMap::new()));

    let server = tcp.incoming().for_each(|(stream, addr)| {
        let (reader, writer) = stream.split();
        let (tx, rx) = futures::sync::mpsc::unbounded();
        connections.borrow_mut().insert(addr, tx);

        let connections_inner = connections.clone();
        let reader = BufReader::new(reader);
        let mut shutdown = false;
        let iter = stream::iter_ok::<_, Error>(iter::repeat(()));
        let socket_reader = iter.fold(reader, move |reader, _| {
            let bytes = io::read_to_end(reader, Vec::new());
            let bytes = bytes.and_then(|(reader, vec)| if vec.len() == 0 {
                Err(Error::new(ErrorKind::BrokenPipe, "broken pipe"))
            } else {
                Ok((reader, vec))
            });
            let bytes = bytes.map(|(reader, vec)| (reader, read_packet(Bytes::from(vec))));
            let connections = connections_inner.clone();
            bytes.map(move |(reader, packet)| {
                // TODO
                let mut conns = connections.borrow_mut();
                println!("Received {:#?}", packet);
                if let Ok(packet) = packet {
                    match answer(packet) {
                        Ok(Some(x)) => {
                            let tx = conns.get_mut(&addr).unwrap();
                            tx.unbounded_send(x).unwrap();
                        },
                        Err(e) => {
                            println!("Error: {}", e);
                            shutdown = true;
                        }
                        _ => {},
                    }
                }
                reader
            })
        });

        let socket_writer = rx.fold(writer, |writer, msg| {
            let amt = io::write_all(writer, msg);
            let amt = amt.map(|(writer, _)| writer);
            amt.map_err(|_| ())
        });

        let connections = connections.clone();
        let mut socket_reader = cancellable_io_future(socket_reader);
        if shutdown {
            socket_reader.request_cancellation();
        }
        let socket_reader = socket_reader.map_err(|_| ());
        let connection = socket_reader.map(|_| ()).select(socket_writer.map(|_| ()));

        handle.spawn(connection.then(move |_| {
            connections.borrow_mut().remove(&addr);
            println!("Connection {} closed.", addr);
            Ok(())
        }));

        Ok(())
    });

    core.run(server).unwrap();
}
