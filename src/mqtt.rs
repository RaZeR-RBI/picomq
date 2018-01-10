extern crate bytes;
extern crate tokio_io;
extern crate tokio_proto;

use bytes::Bytes;

#[derive(Debug, PartialEq)]
enum PacketType {
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
    RESERVED,
}

#[derive(Debug, PartialEq)]
enum QoS {
    AT_MOST_ONCE,
    AT_LEAST_ONCE,
    EXACTLY_ONCE,
    RESERVED,
}

#[derive(Debug, PartialEq)]
struct MqttHeader {
    packet_type: PacketType,
    dup: bool,
    qos: QoS,
    retain: bool,
    remaining_bytes: u32,
    packet_identifier: u16,
}

#[derive(Debug, PartialEq)]
struct MqttPacket {
    header: MqttHeader,
    payload: Bytes,
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
        _ => PacketType::RESERVED,
    }
}

fn to_u16(msb: u8, lsb: u8) -> u16 {
    lsb as u16 | (msb as u16) << 8
}

fn vlq(bytes: &Bytes) -> u32 {
    if bytes[0] < 128 {
        return bytes[0] as u32;
    } else if bytes[1] < 128 {
        return (bytes[0] as u32 & 127) | ((bytes[1] as u32 & 127) << 7);
    } else if bytes[2] < 128 {
        return (bytes[0] as u32 & 127) | ((bytes[1] as u32 & 127) << 7)
            | ((bytes[2] as u32 & 127) << 14);
    } else {
        return (bytes[0] as u32 & 127) | ((bytes[1] as u32 & 127) << 7)
            | ((bytes[2] as u32 & 127) << 14) | ((bytes[3] as u32 & 127) << 21);
    }
}

fn read_header(bytes: Bytes) -> MqttHeader {
    let byte_1 = &bytes[0];
    let ptype = read_packet_type(byte_1);
    let mut dup = false;
    let mut qos = QoS::AT_MOST_ONCE;
    let mut retain = false;

    let byte_2 = &bytes[1];
    match ptype {
        PacketType::PUBLISH => {
            dup = (byte_2 & 0x08) == 8;
            qos = match byte_2 & 0x06 {
                0 => QoS::AT_MOST_ONCE,
                1 => QoS::AT_LEAST_ONCE,
                2 => QoS::EXACTLY_ONCE,
                _ => QoS::RESERVED,
            };
            retain = (byte_2 & 0x01) == 1;
        }
        _ => {}
    }

    let packet_id = match ptype {
        PacketType::PUBLISH if qos != QoS::AT_MOST_ONCE => to_u16(bytes[2], bytes[3]),
        PacketType::CONNECT
        | PacketType::CONNACK
        | PacketType::PINGREQ
        | PacketType::PINGRESP
        | PacketType::DISCONNECT => to_u16(bytes[2], bytes[3]),
        _ => 1,
    };

    MqttHeader {
        packet_type: ptype,
        dup: dup,
        qos: qos,
        retain: retain,
        remaining_bytes: vlq(&bytes.slice_from(2)),
        packet_identifier: packet_id,
    }
}

#[allow(unused_variables)]
#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use bytes::Bytes;
    use mqtt::*;

    #[test]
    fn reads_correct_packet_type() {
        let type_map: HashMap<_, _> = vec![
            (0x00, PacketType::RESERVED),
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
            (0xF0, PacketType::RESERVED),
        ].into_iter()
            .collect();

        for (byte, ref ptype) in type_map.iter() {
            assert_eq!(read_packet_type(byte), **ptype);
        }
    }

    #[test]
    fn reads_vlq() {
        assert_eq!(vlq(&Bytes::from(vec![0])), 0);
        assert_eq!(vlq(&Bytes::from(vec![0x40])), 64);
        assert_eq!(vlq(&Bytes::from(vec![0x7F])), 127);
        assert_eq!(vlq(&Bytes::from(vec![0x80, 0x01])), 128);
        assert_eq!(vlq(&Bytes::from(vec![193, 2])), 321);
        assert_eq!(vlq(&Bytes::from(vec![0xFF, 0x7F])), 16383);
        assert_eq!(vlq(&Bytes::from(vec![0x80, 0x80, 0x01])), 16384);
        assert_eq!(vlq(&Bytes::from(vec![0xFF, 0xFF, 0x7F])), 2097151);
        assert_eq!(vlq(&Bytes::from(vec![0x80, 0x80, 0x80, 0x01])), 2097152);
        assert_eq!(vlq(&Bytes::from(vec![0xFF, 0xFF, 0xFF, 0x7F])), 268435455);
    }
}
