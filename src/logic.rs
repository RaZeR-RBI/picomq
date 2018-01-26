extern crate bytes;

use bytes::Bytes;
use mqtt::*;
use mqtt::reader::*;

pub fn answer(packet: MqttPacket) -> Result<Option<Bytes>, &'static str> {
    Ok(Some(Bytes::from(vec![0x20, 0x02, 0x00, 0x00])))
}
