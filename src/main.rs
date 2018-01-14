extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_proto;
extern crate bytes;

mod mqtt;
mod mqtt_net;

use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use std::iter;
use std::env;
use std::io::{Error, ErrorKind, BufReader};

use futures::Future;
use futures::stream::{self, Stream};
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;
use tokio_io::io;
use tokio_io::AsyncRead;

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

        let iter = stream::iter_ok::<_, Error>(iter::repeat(()));
        let socket_reader = iter.fold(reader, move |reader, _| {
            let line = io::read_until(reader, b'\n', Vec::new());
            let line = line.and_then(|(reader, vec)| {
                if vec.len() == 0 {
                    Err(Error::new(ErrorKind::BrokenPipe, "broken pipe"))
                } else {
                    Ok((reader, vec))
                }
            });

            let line = line.map(|(reader, vec)| {
                (reader, String::from_utf8(vec))
            });
            let connections = connections_inner.clone();
            line.map(move |(reader, message)| {
                println!("{}: {:?}", addr, message);
                let mut conns = connections.borrow_mut();
                if let Ok(msg) = message {
                    let iter = conns.iter_mut()
                                    .filter(|&(&k, _)| k != addr)
                                    .map(|(_, v)| v);
                    for tx in iter {
                        tx.unbounded_send(format!("{}: {}", addr, msg)).unwrap();
                    }
                } else {
                    let tx = conns.get_mut(&addr).unwrap();
                    tx.unbounded_send("You didn't send valid UTF-8.".to_string()).unwrap();
                }
                reader
            })
        });

        let socket_writer = rx.fold(writer, |writer, msg| {
            let amt = io::write_all(writer, msg.into_bytes());
            let amt = amt.map(|(writer, _)| writer);
            amt.map_err(|_| ())
        });

        let connections = connections.clone();
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
