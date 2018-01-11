extern crate bytes;
extern crate tokio_io;
extern crate tokio_proto;

use bytes::Bytes;

#[derive(Debug, PartialEq)]
pub enum PacketType {
    Connect,
    ConnAck,
    Publish,
    PubAck,
    PubRec,
    PubRel,
    PubComp,
    Subscribe,
    SubAck,
    Unsubscribe,
    UnsubAck,
    PingReq,
    PingResp,
    Disconnect,
    Reserved,
}

#[derive(Debug, PartialEq)]
pub enum QoS {
    AtMostOnce,
    AtLeastOnce,
    ExactlyOnce,
    Reserved,
}

impl QoS {
    fn from_byte(byte: u8, right_shift: u8) -> QoS {
        match (byte >> right_shift) & 0x03 {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => QoS::Reserved,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct FixedHeader {
    pub packet_type: PacketType,
    pub dup: bool,
    pub qos: QoS,
    pub retain: bool,
    remaining_bytes: u32,
    payload_offset: usize,
}

#[derive(Debug, PartialEq)]
pub struct MqttPacket {
    pub header: FixedHeader,
    pub var_header: VariableHeader,
    pub payload: Bytes,
}

/* Fixed-length headers for different packet types */
#[derive(Debug, PartialEq)]
pub enum VariableHeader {
    None,
    Connect(ConnectHeader),
    ConnAck(ConnAckHeader),
    Publish(PublishHeader),
    PubAck(u16),
    PubRec(u16),
    PubRel(u16),
    PubComp(u16),
    Subscribe(u16),
    SubAck(u16),
    Unsubscribe(u16),
    UnsubAck(u16),
}

/* CONNECT */
#[derive(Debug, PartialEq)]
pub struct ConnectHeader {
    pub protocol_name: Bytes,
    pub protocol_level: u8,
    flag_bits: u8,
    pub keep_alive: u16,
}

impl ConnectHeader {
    fn get_flag(&self, mask: u8) -> bool {
        (self.flag_bits & mask) == mask
    }
    pub fn has_username_flag(&self) -> bool {
        self.get_flag(0b10000000)
    }
    pub fn has_password_flag(&self) -> bool {
        self.get_flag(0b01000000)
    }
    pub fn will_retain(&self) -> bool {
        self.get_flag(0b00100000)
    }
    pub fn will_qos(&self) -> QoS {
        QoS::from_byte(self.flag_bits, 3)
    }
    pub fn has_will_flag(&self) -> bool {
        self.get_flag(0b00000100)
    }
    pub fn clean_session(&self) -> bool {
        self.get_flag(0b00000010)
    }
}

/* CONNACK */
#[derive(Debug, PartialEq)]
pub struct ConnAckHeader {
    flags: u8,
    pub return_code: ConnAckReturnCode,
}

#[derive(Debug, PartialEq)]
pub enum ConnAckReturnCode {
    Accepted,
    UnacceptableProtocol,
    IdentifierRejected,
    ServerUnavailable,
    BadAuth,
    NotAuthorized,
    Reserved,
}

impl ConnAckReturnCode {
    pub fn from_byte(value: u8) -> ConnAckReturnCode {
        match value {
            0 => ConnAckReturnCode::Accepted,
            1 => ConnAckReturnCode::UnacceptableProtocol,
            2 => ConnAckReturnCode::IdentifierRejected,
            3 => ConnAckReturnCode::ServerUnavailable,
            4 => ConnAckReturnCode::NotAuthorized,
            _ => ConnAckReturnCode::Reserved,
        }
    }

    pub fn to_byte(self) -> u8 {
        match self {
            ConnAckReturnCode::Accepted => 0,
            ConnAckReturnCode::UnacceptableProtocol => 1,
            ConnAckReturnCode::IdentifierRejected => 2,
            ConnAckReturnCode::ServerUnavailable => 3,
            ConnAckReturnCode::NotAuthorized => 4,
            _ => 255,
        }
    }
}

impl ConnAckHeader {
    pub fn session_present(&self) -> bool {
        self.flags & 0x01 == 0x01
    }
}

/* Helper functions */
fn read_packet_type(b: u8) -> PacketType {
    let value = (b & 0xF0) >> 4;
    match value {
        1 => PacketType::Connect,
        2 => PacketType::ConnAck,
        3 => PacketType::Publish,
        4 => PacketType::PubAck,
        5 => PacketType::PubRec,
        6 => PacketType::PubRel,
        7 => PacketType::PubComp,
        8 => PacketType::Subscribe,
        9 => PacketType::SubAck,
        10 => PacketType::Unsubscribe,
        11 => PacketType::UnsubAck,
        12 => PacketType::PingReq,
        13 => PacketType::PingResp,
        14 => PacketType::Disconnect,
        _ => PacketType::Reserved,
    }
}

#[derive(Debug, PartialEq)]
pub struct PublishHeader {
    pub topic_name: Bytes,
    pub packet_id: u16,
}

pub mod reader {
    use mqtt::*;

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
                    | ((bytes[2] as u32 & 127) << 14)
                    | ((bytes[3] as u32 & 127) << 21),
                4,
            );
        }
    }

    fn read_header(bytes: &Bytes) -> FixedHeader {
        let byte_1 = bytes[0];
        let ptype = read_packet_type(byte_1);

        let dup = (byte_1 & 0x08) == 8;
        let qos = match (byte_1 & 0x06) >> 1 {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => QoS::Reserved,
        };
        let retain = (byte_1 & 0x01) == 1;

        let (remaining_bytes, offset) = vlq(&bytes.slice_from(1));
        FixedHeader {
            packet_type: ptype,
            dup: dup,
            qos: qos,
            retain: retain,
            remaining_bytes: remaining_bytes,
            payload_offset: offset + 1,
        }
    }

    fn read_var_header(header: &FixedHeader, bytes: &Bytes) -> (VariableHeader, usize) {
        match header.packet_type {
            // CONNECT
            PacketType::Connect => {
                let proto_name_len = to_u16(bytes[0], bytes[1]) as usize;
                let protocol_name = bytes.slice(2, proto_name_len + 2);
                let protocol_level = bytes[proto_name_len + 2];
                let flag_bits = bytes[proto_name_len + 3];
                let keep_alive = to_u16(bytes[proto_name_len + 4], bytes[proto_name_len + 5]);
                (
                    VariableHeader::Connect(ConnectHeader {
                        protocol_name: protocol_name,
                        protocol_level: protocol_level,
                        flag_bits: flag_bits,
                        keep_alive: keep_alive,
                    }),
                    proto_name_len + 6,
                )
            }
            // CONNACK
            PacketType::ConnAck => (
                VariableHeader::ConnAck(ConnAckHeader {
                    flags: bytes[0],
                    return_code: ConnAckReturnCode::from_byte(bytes[1]),
                }),
                2,
            ),
            // PUBLISH
            PacketType::Publish => {
                let topic_len = to_u16(bytes[0], bytes[1]) as usize;
                let topic = bytes.slice(2, topic_len + 2);
                let mut packet_id = 0u16;
                let mut offset = topic_len + 2;
                if header.qos != QoS::AtMostOnce {
                    packet_id = to_u16(bytes[offset], bytes[offset + 1]);
                    offset += 2;
                }
                (
                    VariableHeader::Publish(PublishHeader {
                        topic_name: topic,
                        packet_id: packet_id,
                    }),
                    offset,
                )
            }
            PacketType::PubAck => (VariableHeader::PubAck(to_u16(bytes[0], bytes[1])), 2),
            PacketType::PubRec => (VariableHeader::PubRec(to_u16(bytes[0], bytes[1])), 2),
            PacketType::PubRel => (VariableHeader::PubRel(to_u16(bytes[0], bytes[1])), 2),
            PacketType::PubComp => (VariableHeader::PubComp(to_u16(bytes[0], bytes[1])), 2),
            PacketType::Subscribe => (VariableHeader::Subscribe(to_u16(bytes[0], bytes[1])), 2),
            PacketType::SubAck => (VariableHeader::SubAck(to_u16(bytes[0], bytes[1])), 2),
            PacketType::Unsubscribe => (VariableHeader::Unsubscribe(to_u16(bytes[0], bytes[1])), 2),
            PacketType::UnsubAck => (VariableHeader::UnsubAck(to_u16(bytes[0], bytes[1])), 2),
            _ => (VariableHeader::None, 0),
        }
    }

    pub fn read_packet(bytes: Bytes) -> MqttPacket {
        let header = read_header(&bytes);
        let fixed_offset = header.payload_offset;
        let (var_header, var_offset) = read_var_header(&header, &bytes.slice_from(fixed_offset));
        let payload = bytes.slice_from(fixed_offset + var_offset);
        MqttPacket {
            header: header,
            var_header: var_header,
            payload: payload,
        }
    }


    /* Tests */
    #[allow(unused_variables)]
    #[cfg(test)]
    mod tests {
        use std::collections::HashMap;
        use bytes::Bytes;
        use mqtt::*;
        use mqtt::reader::*;
        use mqtt::VariableHeader::*;

        #[test]
        fn reads_correct_packet_type() {
            let type_map: HashMap<_, _> = vec![
                (0x00, PacketType::Reserved),
                (0x10, PacketType::Connect),
                (0x20, PacketType::ConnAck),
                (0x30, PacketType::Publish),
                (0x40, PacketType::PubAck),
                (0x50, PacketType::PubRec),
                (0x60, PacketType::PubRel),
                (0x70, PacketType::PubComp),
                (0x80, PacketType::Subscribe),
                (0x90, PacketType::SubAck),
                (0xA0, PacketType::Unsubscribe),
                (0xB0, PacketType::UnsubAck),
                (0xC0, PacketType::PingReq),
                (0xD0, PacketType::PingResp),
                (0xE0, PacketType::Disconnect),
                (0xF0, PacketType::Reserved),
            ].into_iter()
                .collect();

            for (byte, ref ptype) in type_map.iter() {
                assert_eq!(read_packet_type(*byte), **ptype);
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

        /* Common tests */
        #[test]
        fn reads_header_without_qos() {
            let connect_command = Bytes::from(vec![0x10, 0x25]);
            let header = read_header(&connect_command);
            assert_eq!(header.packet_type, PacketType::Connect);
            assert_eq!(header.remaining_bytes, 37);

            // Reserved values
            assert_eq!(header.dup, false);
            assert_eq!(header.qos, QoS::AtMostOnce);
            assert_eq!(header.retain, false);
        }

        #[test]
        fn reads_header_with_qos() {
            let subscribe_command = Bytes::from(vec![0x82, 0x10, 0x00, 0x01]);
            let header = read_header(&subscribe_command);
            assert_eq!(header.packet_type, PacketType::Subscribe);
            assert_eq!(header.remaining_bytes, 16);
            assert_eq!(header.dup, false);
            assert_eq!(header.qos, QoS::AtLeastOnce);
            assert_eq!(header.retain, false);
        }

        /* Packet tests */
        #[test]
        fn reads_connect_packet() {
            // CONNECT, MsgLen = 37, protocol name = MQTT, protocol level = 4,
            // flags = 2, keep-alive: 5, client ID = "paho"
            let data = Bytes::from(vec![
                0x10, 0x25, 0x00, 0x04, 0x8D, 0x91, 0x94, 0x94, 0x04, 0x02, 0x00, 0x05, 0x00, 0x04,
                0x70, 0x61, 0x68, 0x6f,
            ]);
            let protocol_name = data.slice(4, 8);
            let payload = data.slice_from(12);
            let packet = read_packet(data);
            assert_eq!(packet.header.packet_type, PacketType::Connect);
            match packet.var_header {
                Connect(h) => {
                    assert_eq!(protocol_name, &h.protocol_name);
                    assert_eq!(h.protocol_level, 0x04);
                    assert_eq!(h.flag_bits, 0x02);
                    assert_eq!(h.clean_session(), true);
                    assert_eq!(h.keep_alive, 5);
                }
                _ => panic!(),
            }
            assert_eq!(packet.payload, &payload);
        }

        #[test]
        fn reads_connack_packet() {
            // CONNACK, no session present, connection accepted
            let data = Bytes::from(vec![0x20, 0x02, 0x00, 0x00]);
            let packet = read_packet(data);
            assert_eq!(packet.header.packet_type, PacketType::ConnAck);
            match packet.var_header {
                ConnAck(h) => {
                    assert_eq!(h.session_present(), false);
                    assert_eq!(h.return_code, ConnAckReturnCode::Accepted);
                }
                _ => panic!(),
            }
        }

        #[test]
        fn reads_publish_packet_with_packet_id() {
            // PUBLISH, QoS = 1 (should have packet ID), topic: a/b, packet ID = 10, payload = Hello
            let data = Bytes::from(vec![
                0x33, 0x30, 0x00, 0x03, 0x61, 0x2F, 0x62, 0x00, 0xA, 0x48, 0x65, 0x6C, 0x6C, 0x6F
            ]);
            let topic = data.slice(4, 7);
            let payload = data.slice_from(9);
            let packet = read_packet(data);
            assert_eq!(packet.header.packet_type, PacketType::Publish);
            match packet.var_header {
                Publish(h) => {
                    assert_eq!(h.packet_id, 10);
                    assert_eq!(h.topic_name, &topic);
                }
                _ => panic!(),
            }
            assert_eq!(packet.payload, &payload);
        }

        #[test]
        fn reads_publish_packet_without_packet_id() {
            // PUBLISH, QoS = 0 (shouldn't have packet ID), topic: a/b, payload = Hello
            let data = Bytes::from(vec![
                0x31, 0x30, 0x00, 0x03, 0x61, 0x2F, 0x62, 0x48, 0x65, 0x6C, 0x6C, 0x6F
            ]);
            let topic = data.slice(4, 7);
            let payload = data.slice_from(7);
            let packet = read_packet(data);
            assert_eq!(packet.header.packet_type, PacketType::Publish);
            match packet.var_header {
                Publish(h) => {
                    assert_eq!(h.topic_name, &topic);
                }
                _ => panic!(),
            }
            assert_eq!(packet.payload, &payload);
        }

        #[test]
        fn reads_packets_with_packetid_varheader() {
            // SUBACK, packet ID 1, payload 0x00
            let data = Bytes::from(vec![0x90, 0x03, 0x00, 0x01, 0x00]);
            let payload = data.slice_from(4);
            let packet = read_packet(data);
            
            assert_eq!(packet.header.packet_type, PacketType::SubAck);
            match packet.var_header {
                SubAck(packet_id) => assert_eq!(packet_id, 1),
                _ => panic!()
            }
            assert_eq!(packet.payload, &payload);
        }
    }
}
