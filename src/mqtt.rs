extern crate bytes;
extern crate tokio_io;
extern crate tokio_proto;

use bytes::Bytes;
use std::str::from_utf8;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone)]
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

#[derive(Debug, PartialEq, Clone)]
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
    pub payload: Bytes,
}

#[derive(Debug, PartialEq)]
pub struct MqttPacket {
    pub header: FixedHeader,
    pub var_header: VariableHeader,
    pub payload: Bytes,
}

/* Fixed-length headers for different packet types */
#[derive(Debug, PartialEq, Clone)]
pub enum VariableHeader {
    None,
    Connect(ConnectHeader),
    ConnAck(ConnAckHeader),
    Publish(PublishHeader),
    WithPacketId(u16),
}

/* CONNECT */
#[derive(Debug, PartialEq, Clone)]
pub struct ConnectHeader {
    pub protocol_name: String,
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

#[derive(Debug, PartialEq, Default)]
pub struct ConnectPayload {
    pub client_id: String,
    pub will_topic: Option<String>,
    pub will_message: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

/* CONNACK */
#[derive(Debug, PartialEq, Clone)]
pub struct ConnAckHeader {
    flags: u8,
    pub return_code: ConnAckReturnCode,
}

#[derive(Debug, PartialEq, Clone)]
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
            _ => panic!("Reserved return code should not be used"),
        }
    }
}

impl ConnAckHeader {
    pub fn session_present(&self) -> bool {
        self.flags & 0x01 == 0x01
    }
}

/* PUBLISH */
#[derive(Debug, PartialEq, Clone)]
pub struct PublishHeader {
    pub topic_name: String,
    pub packet_id: u16,
}

/* SUBSCRIBE */
#[derive(Debug, PartialEq)]
pub struct SubscribePayload {
    pub packet_id: u16,
    pub filters: HashMap<String, QoS>,
}

impl Clone for SubscribePayload {
    fn clone(&self) -> Self {
        let packet_id = &self.packet_id;
        let filters_to_copy = &self.filters;

        let mut filters = HashMap::new();
        for (key, val) in filters_to_copy.iter() {
            filters.insert(key.clone().to_string(), val.clone());
        }
        SubscribePayload {
            packet_id: *packet_id,
            filters: filters,
        }
    }
}

/* SUBACK */
#[derive(Debug, PartialEq, Clone)]
pub enum SubAckReturnCode {
    MaximumQoS0,
    MaximumQoS1,
    MaximumQoS2,
    Failure,
    Reserved,
}

impl SubAckReturnCode {
    pub fn from_byte(value: u8) -> SubAckReturnCode {
        match value {
            0x00 => SubAckReturnCode::MaximumQoS0,
            0x01 => SubAckReturnCode::MaximumQoS1,
            0x02 => SubAckReturnCode::MaximumQoS2,
            0x80 => SubAckReturnCode::Failure,
            _ => SubAckReturnCode::Reserved,
        }
    }

    pub fn to_byte(self) -> u8 {
        match self {
            SubAckReturnCode::MaximumQoS0 => 0x00,
            SubAckReturnCode::MaximumQoS1 => 0x01,
            SubAckReturnCode::MaximumQoS2 => 0x02,
            SubAckReturnCode::Failure => 0x80,
            _ => panic!("Reserved return code should not be used"),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct SubAckPayload {
    packet_id: u16,
    return_codes: Vec<SubAckReturnCode>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct UnsubscribePayload {
    packet_id: u16,
    filters: Vec<String>,
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

    fn utf8_safe_decode(bytes: &Bytes, start: usize) -> Option<String> {
        if bytes.len() < 2 {
            return None;
        }
        let str_len = to_u16(bytes[0], bytes[1]) as usize;
        if str_len == 0 {
            return Some(String::new());
        }
        if str_len + 2 > bytes.len() {
            return None;
        }
        println!("{:?} - len = {}", bytes, str_len); 
        match from_utf8(&bytes.slice(2, str_len + 2)) {
            Ok(s) => Some(s.to_string()),
            Err(_) => None,
        }
    }

    fn utf8_safe_scan(bytes: &mut Bytes) -> Option<String> {
        if bytes.len() < 2 {
            return None;
        }
        let str_len = to_u16(bytes[0], bytes[1]) as usize;
        if bytes.len() < str_len + 2 {
            return None;
        }
        let result = bytes.slice(2, 2 + str_len);
        bytes.advance(str_len + 2);
        match from_utf8(&result) {
            Ok(s) => Some(s.to_string()),
            Err(_) => None,
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

    fn validate_header_flags(ptype: &PacketType, input: u8) -> bool {
        match *ptype {
            PacketType::Publish => true,
            PacketType::PubRel | PacketType::Subscribe | PacketType::Unsubscribe => input == 0x02,
            _ => input == 0x00,
        }
    }

    fn read_header(bytes: &Bytes) -> Result<FixedHeader, &'static str> {
        if bytes.len() < 2 {
            return Err("Not enough bytes received");
        }
        let byte_1 = bytes[0];
        let ptype = read_packet_type(byte_1);
        if ptype == PacketType::Reserved {
            return Err("Invalid packet type");
        }
        if !validate_header_flags(&ptype, byte_1 & 0x0F) {
            return Err("Invalid header flags");
        }

        let dup = (byte_1 & 0x08) == 8;
        let qos = match (byte_1 & 0x06) >> 1 {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => QoS::Reserved,
        };
        let retain = (byte_1 & 0x01) == 1;

        let (remaining_bytes, offset) = vlq(&bytes.slice_from(1));
        Ok(FixedHeader {
            packet_type: ptype,
            dup: dup,
            qos: qos,
            retain: retain,
            remaining_bytes: remaining_bytes,
            payload: bytes.slice_from(offset + 1),
        })
    }

    fn construct_packet(header: FixedHeader) -> Result<MqttPacket, &'static str> {
        // TODO: Remove clone()
        let bytes = header.payload.clone();
        match header.packet_type {
            // CONNECT
            PacketType::Connect => {
                if bytes.len() < 4 {
                    return Err("Not enough data supplied");
                }
                println!("{:?}", bytes);
                if let Some(protocol_name) = utf8_safe_decode(&bytes, 0) {
                    let proto_name_len = protocol_name.len();
                    if bytes.len() < proto_name_len + 6 {
                        return Err("Not enough data supplied");
                    }
                    let protocol_level = bytes[proto_name_len + 2];
                    let flag_bits = bytes[proto_name_len + 3];
                    let keep_alive = to_u16(bytes[proto_name_len + 4], bytes[proto_name_len + 5]);
                    let var_header = VariableHeader::Connect(ConnectHeader {
                        protocol_name: protocol_name,
                        protocol_level: protocol_level,
                        flag_bits: flag_bits,
                        keep_alive: keep_alive,
                    });
                    return Ok(MqttPacket {
                        header: header,
                        var_header: var_header,
                        payload: bytes.slice_from(proto_name_len + 6),
                    });
                }
                Err("Invalid UTF-8 sequence in protocol name")
            }
            // CONNACK
            PacketType::ConnAck => match bytes.len() {
                2 => {
                    let var_header = VariableHeader::ConnAck(ConnAckHeader {
                        flags: bytes[0],
                        return_code: ConnAckReturnCode::from_byte(bytes[1]),
                    });
                    return Ok(MqttPacket {
                        header: header,
                        var_header: var_header,
                        payload: Bytes::new(),
                    });
                }
                _ => Err("Invalid data supplied"),
            },
            // PUBLISH
            PacketType::Publish => {
                if (header.qos != QoS::AtMostOnce && bytes.len() < 6) || bytes.len() < 4 {
                    return Err("Not enough data supplied");
                }
                if let Some(topic) = utf8_safe_decode(&bytes, 0) {
                    let topic_len = topic.len();
                    let mut packet_id = 0u16;
                    let mut offset = topic_len + 2;
                    if header.qos != QoS::AtMostOnce {
                        packet_id = to_u16(bytes[offset], bytes[offset + 1]);
                        offset += 2;
                    }
                    return Ok(MqttPacket {
                        header: header,
                        var_header: VariableHeader::Publish(PublishHeader {
                            topic_name: topic,
                            packet_id: packet_id,
                        }),
                        payload: bytes.slice_from(offset),
                    });
                }
                Err("Invalid UTF-8 sequence")
            }
            PacketType::PubAck
            | PacketType::PubRec
            | PacketType::PubRel
            | PacketType::PubComp
            | PacketType::Subscribe
            | PacketType::SubAck
            | PacketType::Unsubscribe
            | PacketType::UnsubAck => {
                if bytes.len() < 2 {
                    return Err("Not enough data supplied");
                }
                Ok(MqttPacket {
                    header: header,
                    var_header: VariableHeader::WithPacketId(to_u16(bytes[0], bytes[1])),
                    payload: bytes.slice_from(2),
                })
            }
            _ => Ok(MqttPacket {
                header: header,
                var_header: VariableHeader::None,
                payload: bytes,
            }),
        }
    }

    pub fn read_packet(bytes: Bytes) -> Result<MqttPacket, &'static str> {
        read_header(&bytes).and_then(|h| construct_packet(h))
    }

    /* Packet-type related payload reading */
    impl MqttPacket {
        pub fn get_connect_payload(self) -> Result<ConnectPayload, &'static str> {
            if self.header.packet_type != PacketType::Connect {
                panic!("Tried to read non-CONNECT packet as CONNECT type");
            }
            let head = match self.var_header {
                VariableHeader::Connect(h) => h,
                _ => panic!("Found non-CONNECT varheader in CONNECT packet type"),
            };
            let mut bytes = self.payload.clone();
            let mut result = ConnectPayload::default();

            if let Some(client_id) = utf8_safe_scan(&mut bytes) {
                result.client_id = client_id;
            } else {
                return Err("No client ID, neither zero-byte client ID is specified");
            }

            if head.has_will_flag() {
                let topic = utf8_safe_scan(&mut bytes);
                let message = utf8_safe_scan(&mut bytes);
                match (topic, message) {
                    (Some(t), Some(msg)) => {
                        result.will_topic = Some(t);
                        result.will_message = Some(msg);
                    }
                    _ => return Err("Will flag is defined, but no topic or message is found"),
                }
            }

            if head.has_username_flag() {
                match utf8_safe_scan(&mut bytes) {
                    Some(u) => result.username = Some(u),
                    _ => return Err("Username flag is defined, but no username is found"),
                }
            }

            if head.has_password_flag() {
                match utf8_safe_scan(&mut bytes) {
                    Some(p) => result.password = Some(p),
                    _ => return Err("Password flag is defined, but no password is found"),
                }
            }

            Ok(result)
        }

        pub fn get_subscribe_payload(self) -> Result<SubscribePayload, &'static str> {
            if self.header.packet_type != PacketType::Subscribe {
                panic!("Tried to read non-SUBSCRIBE packet as SUBSCRIBE type");
            }
            let packet_id = match self.var_header {
                VariableHeader::WithPacketId(h) => h,
                _ => panic!("Found uncompatible varheader in SUBSCRIBE packet type"),
            };
            if self.payload.len() == 0 {
                return Err("No payload found");
            }
            let mut bytes = self.payload.clone();
            let mut filters = HashMap::<String, QoS>::new();
            loop {
                if bytes.len() < 2 {
                    break;
                }
                match utf8_safe_scan(&mut bytes) {
                    Some(filter) => {
                        if bytes.len() == 0 {
                            return Err("Unexpected end of stream");
                        }
                        let qos = QoS::from_byte(bytes[0], 0);
                        bytes.advance(1);
                        filters.insert(filter, qos);
                    }
                    None => return Err("Found invalid UTF-8 sequence"),
                }
            }

            Ok(SubscribePayload {
                packet_id: packet_id,
                filters: filters,
            })
        }

        pub fn get_suback_payload(self) -> Result<SubAckPayload, &'static str> {
            if self.header.packet_type != PacketType::SubAck {
                panic!("Tried to read non-SUBACK packet as SUBACK type");
            }
            let packet_id = match self.var_header {
                VariableHeader::WithPacketId(h) => h,
                _ => panic!("Found incompatible varheader in SUBACK packet type"),
            };
            let bytes = self.payload;
            if bytes.len() == 0 {
                return Err("No result codes found in payload");
            }
            let mut return_codes = Vec::<SubAckReturnCode>::with_capacity(bytes.len());
            for byte in bytes {
                return_codes.push(SubAckReturnCode::from_byte(byte));
            }
            Ok(SubAckPayload {
                packet_id: packet_id,
                return_codes: return_codes,
            })
        }

        pub fn get_unsubscribe_payload(self) -> Result<UnsubscribePayload, &'static str> {
            if self.header.packet_type != PacketType::Unsubscribe {
                panic!("Tried to read non-UNSUBSCRIBE packet as UNSUBSCRIBE type");
            }
            let packet_id = match self.var_header {
                VariableHeader::WithPacketId(h) => h,
                _ => panic!("Found incompatible varheader in UNSUBSCRIBE packet type"),
            };
            if self.payload.len() == 0 {
                return Err("No payload found");
            }
            let mut bytes = self.payload.clone();
            let mut filters = vec![];
            loop {
                if bytes.len() < 2 {
                    break;
                }
                match utf8_safe_scan(&mut bytes) {
                    Some(filter) => filters.push(filter),
                    None => return Err("Found invalid UTF-8 sequence"),
                }
            }

            Ok(UnsubscribePayload {
                packet_id: packet_id,
                filters: filters,
            })
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
            let header = read_header(&connect_command).unwrap();
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
            let header = read_header(&subscribe_command).unwrap();
            assert_eq!(header.packet_type, PacketType::Subscribe);
            assert_eq!(header.remaining_bytes, 16);
            assert_eq!(header.dup, false);
            assert_eq!(header.qos, QoS::AtLeastOnce);
            assert_eq!(header.retain, false);
        }

        /* Packet tests */
        #[test]
        fn reads_connect_packet() {
            // CONNECT, MsgLen = 18, protocol name = MQTT, protocol level = 4,
            // flags = 2, keep-alive: 5, client ID = "paho"
            let data = Bytes::from(vec![
                0x10, 0x12, 0x00, 0x04, 0x4D, 0x51, 0x54, 0x54, 0x04, 0x02, 0x00, 0x05, 0x00, 0x04,
                0x70, 0x61, 0x68, 0x6f,
            ]);
            let packet = read_packet(data).unwrap();
            assert_eq!(packet.header.packet_type, PacketType::Connect);
            match packet.var_header.clone() {
                Connect(h) => {
                    assert_eq!(h.protocol_name, "MQTT");
                    assert_eq!(h.protocol_level, 0x04);
                    assert_eq!(h.flag_bits, 0x02);
                    assert_eq!(h.clean_session(), true);
                    assert_eq!(h.keep_alive, 5);
                }
                _ => panic!(),
            }
            let payload = packet.get_connect_payload();
            match payload {
                Ok(p) => {
                    assert_eq!(p.client_id, "paho");
                    assert_eq!(p.will_topic, Option::None);
                    assert_eq!(p.will_message, Option::None);
                    assert_eq!(p.username, Option::None);
                    assert_eq!(p.password, Option::None);
                }
                Err(r) => panic!(r),
            }
        }

        #[test]
        fn reads_connack_packet() {
            // CONNACK, no session present, connection accepted
            let data = Bytes::from(vec![0x20, 0x02, 0x00, 0x00]);
            let packet = read_packet(data).unwrap();
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
        fn reads_subscribe_packet() {
            // SUBSCRIBE, packet ID = 1, topic "SampleTopic", QoS = 0
            let data = Bytes::from(vec![
                0x82, 0x10, 0x00, 0x01, 0x00, 0x0b, 0x53, 0x61, 0x6d, 0x70, 0x6c, 0x65, 0x54, 0x6f,
                0x70, 0x69, 0x63, 0x00,
            ]);
            let packet = read_packet(data).unwrap();
            let mut filters = HashMap::new();
            filters.insert("SampleTopic".to_string(), QoS::AtMostOnce);

            assert_eq!(packet.header.packet_type, PacketType::Subscribe);
            match packet.var_header.clone() {
                WithPacketId(id) => assert_eq!(id, 1),
                _ => panic!(),
            }
            let payload = packet.get_subscribe_payload();
            match payload {
                Ok(p) => {
                    assert_eq!(p.filters, filters);
                    assert_eq!(p.packet_id, 1);
                }
                Err(r) => panic!(r),
            }
        }

        #[test]
        fn reads_suback_packet() {
            // SUBACK, packet ID 1, payload: QoS 0, QoS 2
            let data = Bytes::from(vec![0x90, 0x04, 0x00, 0x01, 0x00, 0x02]);
            let packet = read_packet(data).unwrap();

            assert_eq!(packet.header.packet_type, PacketType::SubAck);
            match packet.var_header {
                WithPacketId(packet_id) => assert_eq!(packet_id, 1),
                _ => panic!(),
            }
            let payload = packet.get_suback_payload();
            match payload {
                Ok(p) => {
                    assert_eq!(
                        p.return_codes,
                        vec![SubAckReturnCode::MaximumQoS0, SubAckReturnCode::MaximumQoS2]
                    );
                    assert_eq!(p.packet_id, 1);
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
            let payload = data.slice_from(9);
            let packet = read_packet(data).unwrap();
            assert_eq!(packet.header.packet_type, PacketType::Publish);
            match packet.var_header {
                Publish(h) => {
                    assert_eq!(h.packet_id, 10);
                    assert_eq!(h.topic_name, "a/b");
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
            let payload = data.slice_from(7);
            let packet = read_packet(data).unwrap();
            assert_eq!(packet.header.packet_type, PacketType::Publish);
            match packet.var_header {
                Publish(h) => {
                    assert_eq!(h.topic_name, "a/b");
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
            let packet = read_packet(data).unwrap();

            assert_eq!(packet.header.packet_type, PacketType::SubAck);
            match packet.var_header {
                WithPacketId(packet_id) => assert_eq!(packet_id, 1),
                _ => panic!(),
            }
            assert_eq!(packet.payload, &payload);
        }

        #[test]
        fn reads_unsubscribe_packet() {
            // UNSUBSCRIBE, packet ID = 1, topic "SampleTopic"
            let data = Bytes::from(vec![
                0xA2, 0x0F, 0x00, 0x01, 0x00, 0x0b, 0x53, 0x61, 0x6d, 0x70, 0x6c, 0x65, 0x54, 0x6f,
                0x70, 0x69, 0x63,
            ]);
            let packet = read_packet(data).unwrap();
            let filters = vec!["SampleTopic".to_string()];

            assert_eq!(packet.header.packet_type, PacketType::Unsubscribe);
            match packet.var_header.clone() {
                WithPacketId(id) => assert_eq!(id, 1),
                _ => panic!(),
            }
            let payload = packet.get_unsubscribe_payload();
            match payload {
                Ok(p) => {
                    assert_eq!(p.filters, filters);
                    assert_eq!(p.packet_id, 1);
                }
                Err(r) => panic!(r),
            }
        }
    }
}
