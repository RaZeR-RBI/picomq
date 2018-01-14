extern crate futures;
extern crate tokio_io;
extern crate tokio_core;
extern crate tokio_proto;
extern crate bytes;

use mqtt::*;
use mqtt::reader::read_packet;

use bytes::*;

use futures::{future, Future};

use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{Encoder, Decoder, Framed};
use tokio_core::net::TcpStream;
use tokio_core::reactor::Handle;
use tokio_proto::{TcpClient, TcpServer};

use std::{io, str};

pub struct MqttProto;
pub struct MqttCodec;

impl Decoder for MqttCodec {
    type Item = MqttPacket;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<MqttPacket>, io::Error> {
        match read_packet(buf.clone().freeze()) {
            Ok(p) => Ok(Some(p)),
            Err(s) => Err(io::Error::new(io::ErrorKind::InvalidData, s))
        }
    }
}

impl Encoder for MqttCodec {
    type Item = MqttPacket;
    type Error = io::Error;

    fn encode(&mut self, item: MqttPacket, buf: &mut BytesMut) -> io::Result<()> {
        unimplemented!()
    }
}