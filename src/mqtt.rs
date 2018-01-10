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
    payload_offset: u8,
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

fn vlq(bytes: &Bytes) -> (u32, usize) {
    if bytes[0] < 128 {
        return (bytes[0] as u32, 1);
    } else if bytes[1] < 128 {
        return ((bytes[0] as u32 & 127) | ((bytes[1] as u32 & 127) << 7), 2);
    } else if bytes[2] < 128 {
        return (
            (bytes[0] as u32 & 127) | ((bytes[1] as u32 & 127) << 7)
                | ((bytes[2] as u32 & 127) << 14),
            3,
        );
    } else {
        return (
            (bytes[0] as u32 & 127) | ((bytes[1] as u32 & 127) << 7)
                | ((bytes[2] as u32 & 127) << 14) | ((bytes[3] as u32 & 127) << 21),
            4,
        );
    }
}

fn read_header(bytes: Bytes) -> MqttHeader {
    let byte_1 = &bytes[0];
    let ptype = read_packet_type(byte_1);
    let mut payload_offset = 1u8;

    let dup = (byte_1 & 0x08) == 8;
    let qos = match (byte_1 & 0x06) >> 1 {
        0 => QoS::AT_MOST_ONCE,
        1 => QoS::AT_LEAST_ONCE,
        2 => QoS::EXACTLY_ONCE,
        _ => QoS::RESERVED,
    };
    let retain = (byte_1 & 0x01) == 1;

    let (remaining_bytes, offset) = vlq(&bytes.slice_from(1));
    payload_offset += offset as u8;

    let packet_id = match ptype {
        PacketType::PUBLISH if qos != QoS::AT_MOST_ONCE => {
            payload_offset += 2;
            to_u16(bytes[offset + 1], bytes[offset + 2])
        }
        PacketType::CONNECT
        | PacketType::CONNACK
        | PacketType::PINGREQ
        | PacketType::PINGRESP
        | PacketType::DISCONNECT => 0,
        _ => {
            payload_offset += 2;
            to_u16(bytes[offset + 1], bytes[offset + 2])
        }
    };

    MqttHeader {
        packet_type: ptype,
        dup: dup,
        qos: qos,
        retain: retain,
        remaining_bytes: remaining_bytes,
        packet_identifier: packet_id,
        payload_offset: payload_offset,
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
        assert_eq!(vlq(&Bytes::from(vec![0])), (0, 1));
        assert_eq!(vlq(&Bytes::from(vec![0x40])), (64, 1));
        assert_eq!(vlq(&Bytes::from(vec![0x7F])), (127, 1));
        assert_eq!(vlq(&Bytes::from(vec![0x80, 0x01])), (128, 2));
        assert_eq!(vlq(&Bytes::from(vec![193, 2])), (321, 2));
        assert_eq!(vlq(&Bytes::from(vec![0xFF, 0x7F])), (16383, 2));
        assert_eq!(vlq(&Bytes::from(vec![0x80, 0x80, 0x01])), (16384, 3));
        assert_eq!(vlq(&Bytes::from(vec![0xFF, 0xFF, 0x7F])), (2097151, 3));
        assert_eq!(
            vlq(&Bytes::from(vec![0x80, 0x80, 0x80, 0x01])),
            (2097152, 4)
        );
        assert_eq!(
            vlq(&Bytes::from(vec![0xFF, 0xFF, 0xFF, 0x7F])),
            (268435455, 4)
        );
    }

    #[test]
    fn reads_header_without_packet_id() {
        let connect_command = Bytes::from(vec![0x10, 0x25]);
        let header = read_header(connect_command);
        assert_eq!(header.packet_type, PacketType::CONNECT);
        assert_eq!(header.remaining_bytes, 37);

        // reserved values
        assert_eq!(header.dup, false);
        assert_eq!(header.qos, QoS::AT_MOST_ONCE);
        assert_eq!(header.retain, false);
        assert_eq!(header.packet_identifier, 0); // not defined
    }

    #[test]
    fn reads_header_with_qos_and_packet_id() {
        let subscribe_command = Bytes::from(vec![0x82, 0x10, 0x00, 0x01]);
        let header = read_header(subscribe_command);
        assert_eq!(header.packet_type, PacketType::SUBSCRIBE);
        assert_eq!(header.remaining_bytes, 16);
        assert_eq!(header.dup, false);
        assert_eq!(header.qos, QoS::AT_LEAST_ONCE);
        assert_eq!(header.retain, false);
        assert_eq!(header.packet_identifier, 1);
    }
}
