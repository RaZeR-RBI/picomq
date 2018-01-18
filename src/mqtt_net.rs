extern crate futures;
extern crate tokio_io;
extern crate tokio_core;
extern crate tokio_proto;
extern crate bytes;

use mqtt::*;
use mqtt::reader::*;

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


pub struct MqttReader;

impl MqttReader {

    pub fn from_stream(byte_1: u8, read_exact: &Fn(u32) -> Vec<u8>) -> Result<MqttPacket, &'static str> {
       let mut bytes_vec = Vec::<u8>::new();
       let ptype = read_packet_type(byte_1);
       if ptype == PacketType::Reserved {
           return Err("Invalid packet type");
       }
       bytes_vec.push(byte_1);

       let flags = read_exact(1)[0];
       if !validate_header_flags(&ptype, flags) {
           return Err("Invalid header flags");
       }
       bytes_vec.push(flags);

       let mut vlq_vec = Vec::<u8>::new();
       let mut vlq_count = 0u8;
       loop {
           let cur_byte = read_exact(1)[0];
           vlq_count += 1;
           bytes_vec.push(cur_byte);
           vlq_vec.push(cur_byte);
           if vlq_count == 4 || cur_byte < 128{
               break;
           } 
       }
       let (remaining_length, _) = vlq(&Bytes::from(vlq_vec));
       let mut remaining_bytes = read_exact(remaining_length);
       bytes_vec.append(&mut remaining_bytes);

       let bytes = Bytes::from(bytes_vec);
       read_packet(bytes)
    }
}