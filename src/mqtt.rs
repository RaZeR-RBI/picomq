extern crate tokio_proto;
extern crate tokio_io;
extern crate bytes;

use bytes::Bytes;

#[derive(Debug, PartialEq)]
enum PacketType
{
    CONNECT,
    CONNACK,
    PUBLISH,
    PUBACK,
    PUBREC,
    PUBREL,
    PUBCOMP,
    SUBSCRIBE,
    SUBACK,
    UNSUBSCRIBE,
    UNSUBACK,
    PINGREQ,
    PINGRESP,
    DISCONNECT,
    Reserved
}

#[derive(Debug, PartialEq)]
enum QoS
{
    AT_MOST_ONCE,
    AT_LEAST_ONCE,
    EXACTLY_ONCE
}

struct MqttHeader {
    packet_type: PacketType,
    dup: bool,
    qos: QoS,
    retain: bool,
    remaining_bytes: u32,
    packet_identifier: Option<u16>
}

struct MqttPacket<'a> {
    header: MqttHeader,
    payload: Option<&'a[u8]>
}

fn read_packet_type(b: &u8) -> PacketType {
    let value = (b & 0xF0) >> 4;
    match value {
        1 => PacketType::CONNECT,
        2 => PacketType::CONNACK,
        3 => PacketType::PUBLISH,
        4 => PacketType::PUBACK,
        5 => PacketType::PUBREC,
        6 => PacketType::PUBREL,
        7 => PacketType::PUBCOMP,
        8 => PacketType::SUBSCRIBE,
        9 => PacketType::SUBACK,
        10 => PacketType::UNSUBSCRIBE,
        11 => PacketType::UNSUBACK,
        12 => PacketType::PINGREQ,
        13 => PacketType::PINGRESP,
        14 => PacketType::DISCONNECT,
        _ => PacketType::Reserved
    }
}

fn read_header(bytes: Bytes) -> MqttHeader {
    panic!("Not implemented");
}

#[allow(unused_variables)]
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use mqtt::read_packet_type;
    use mqtt::PacketType;

    #[test]
    fn reads_correct_packet_type()
    {
        let type_map: HashMap<_, _> = vec![
            (0x00, PacketType::Reserved),
            (0x10, PacketType::CONNECT),
            (0x20, PacketType::CONNACK),
            (0x30, PacketType::PUBLISH),
            (0x40, PacketType::PUBACK),
            (0x50, PacketType::PUBREC),
            (0x60, PacketType::PUBREL),
            (0x70, PacketType::PUBCOMP),
            (0x80, PacketType::SUBSCRIBE),
            (0x90, PacketType::SUBACK),
            (0xA0, PacketType::UNSUBSCRIBE),
            (0xB0, PacketType::UNSUBACK),
            (0xC0, PacketType::PINGREQ),
            (0xD0, PacketType::PINGRESP),
            (0xE0, PacketType::DISCONNECT),
            (0xF0, PacketType::Reserved)
        ].into_iter().collect();

        for (byte, ref ptype) in type_map.iter() {
            assert_eq!(read_packet_type(byte), **ptype);
        }
    }
}