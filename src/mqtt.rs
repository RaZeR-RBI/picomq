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

#[derive(Debug, PartialEq)]
pub struct FixedHeader {
    packet_type: PacketType,
    dup: bool,
    qos: QoS,
    retain: bool,
    remaining_bytes: u32,
    payload_offset: usize,
}

#[derive(Debug, PartialEq)]
pub struct MqttPacket {
    header: FixedHeader,
    var_header: TypeHeader,
    payload: Bytes,
}

/* Fixed-length headers for different packet types */
#[derive(Debug, PartialEq)]
pub enum TypeHeader {
    None,
    Connect(ConnectHeader),
    ConnAck(ConnAckHeader),
    PubAck(u16),
    PubRec(u16),
    PubRel(u16),
    PubComp(u16),
    Subscribe(u16),
    SubAck(u16),
    Unsubscribe(u16),
    UnsubAck(u16),
}

#[derive(Debug, PartialEq)]
pub struct ConnectHeader {
    supported: bool, // checked by reading protocol name and level
    flag_bits: u8,
    keep_alive: u16,
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
        match (self.flag_bits & 0b00011000) >> 3 {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => QoS::Reserved,
        }
    }
    pub fn has_will_flag(&self) -> bool {
        self.get_flag(0b00000100)
    }
    pub fn clean_session(&self) -> bool {
        self.get_flag(0b00000010)
    }
}

#[derive(Debug, PartialEq)]
pub struct ConnAckHeader {
    flags: u8,
    return_code: u8,
}

impl ConnAckHeader {
    pub fn session_present(&self) -> bool {
        self.flags & 0x01 == 0x01
    }
}

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

fn read_header(bytes: &Bytes) -> FixedHeader {
    let byte_1 = bytes[0];
    let ptype = read_packet_type(byte_1);
    let mut payload_offset = 1;

    let dup = (byte_1 & 0x08) == 8;
    let qos = match (byte_1 & 0x06) >> 1 {
        0 => QoS::AtMostOnce,
        1 => QoS::AtLeastOnce,
        2 => QoS::ExactlyOnce,
        _ => QoS::Reserved,
    };
    let retain = (byte_1 & 0x01) == 1;

    let (remaining_bytes, offset) = vlq(&bytes.slice_from(1));
    payload_offset += offset;
    FixedHeader {
        packet_type: ptype,
        dup: dup,
        qos: qos,
        retain: retain,
        remaining_bytes: remaining_bytes,
        payload_offset: payload_offset,
    }
}

fn read_var_header(header: &FixedHeader, bytes: &Bytes) -> (TypeHeader, usize) {
    match header.packet_type {
        // CONNECT
        PacketType::Connect => {
            let proto_name_len = to_u16(bytes[0], bytes[1]);
            let mut supported = false;
            let mut flag_bits = 0u8;
            let mut keep_alive = 0u16;
            match proto_name_len {
                4 => {
                    // MQTT version 3.1.1
                    supported = bytes[2] == 0x8D && bytes[3] == 0x91 && bytes[4] == 0x94
                        && bytes[5] == 0x94 && bytes[6] == 0x04;
                    if supported {
                        flag_bits = bytes[7];
                        keep_alive = to_u16(bytes[8], bytes[9]);
                    }
                }
                _ => supported = false,
            }
            (
                TypeHeader::Connect(ConnectHeader {
                    supported: supported,
                    flag_bits: flag_bits,
                    keep_alive: keep_alive,
                }),
                10,
            )
        }
        _ => (TypeHeader::None, 0),
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

#[allow(unused_variables)]
#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use bytes::Bytes;
    use mqtt::*;
    use mqtt::TypeHeader::*;

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
    fn reads_header_without_packet_id() {
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
    fn reads_header_with_qos_and_packet_id() {
        let subscribe_command = Bytes::from(vec![0x82, 0x10, 0x00, 0x01]);
        let header = read_header(&subscribe_command);
        assert_eq!(header.packet_type, PacketType::Subscribe);
        assert_eq!(header.remaining_bytes, 16);
        assert_eq!(header.dup, false);
        assert_eq!(header.qos, QoS::AtLeastOnce);
        assert_eq!(header.retain, false);
    }

    /* Type header tests */
    #[test]
    fn reads_connect_varheader() {
        // CONNECT, MsgLen = 37, protocol name = MQTT, protocol level = 4,
        // flags = 2, keep-alive: 5
        let data = Bytes::from(vec![
            0x10, 0x25, 0x00, 0x04, 0x8D, 0x91, 0x94, 0x94, 0x04, 0x02, 0x00, 0x05
        ]);
        let header = read_header(&data);
        let fixed_offset = header.payload_offset;
        let (var_header, var_offset) = read_var_header(&header, &data.slice_from(fixed_offset));
        match var_header {
            Connect(h) => {
                assert_eq!(h.supported, true);
                assert_eq!(h.flag_bits, 0x02);
                assert_eq!(h.keep_alive, 5);
            }
            _ => assert_eq!(true, false),
        }
    }
}
