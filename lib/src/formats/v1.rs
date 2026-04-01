use binrw::prelude::*;

#[binread]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[br(little, repr = u8)]
pub enum PacketKind {
    #[default]
    Empty = 0x00,
    Motion = 0x01,
    Button = 0x02,
    Battery = 0x17,
}

#[binread]
#[derive(Clone, Debug)]
#[br(little)]
pub struct Frame {
    #[br(temp)]
    pub kind: PacketKind,
    #[br(args(kind.clone()))]
    pub packet: Packet,
}

#[binread]
#[derive(Clone, Debug)]
#[br(little, import(kind: PacketKind))]
pub enum Packet {
    #[br(pre_assert(kind == PacketKind::Empty))]
    Empty,
    #[br(pre_assert(kind == PacketKind::Motion))]
    Motion(MotionPacket),
    #[br(pre_assert(kind == PacketKind::Button))]
    Button(ButtonPacket),
    #[br(pre_assert(kind == PacketKind::Battery))]
    Battery(u8),
    #[br()]
    Unknown([u8; 13]),
}

#[binread]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[br(little)]
pub struct MotionPacket {
    pub x: i16,
    pub y: i16,
    pub z: i16,

    pub rx: i16,
    pub ry: i16,
    pub rz: i16,
}

#[binread]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[br(little)]
pub struct ButtonPacket {}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use std::io::Cursor;

    use super::*;

    #[rstest]
    #[case::push_left(
        &[0x01, 0xAF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        MotionPacket { x: -81, y: 0, z: 0, rx: 0, ry: 0, rz: 0 },
    )]
    #[case::push_right(
        &[0x01, 0x51, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        MotionPacket { x: 81, y: 0, z: 0, rx: 0, ry: 0, rz: 0 },
    )]
    #[case::push_forward(
        &[0x01, 0x00, 0x00, 0x33, 0xFF, 0x00, 0x00, 0x0, 0x00, 0x00, 0x00, 0x00, 0x00],
        MotionPacket { x: 0, y: -205, z: 0, rx: 0, ry: 0, rz: 0 },
    )]
    #[case::push_up(
        &[0x01, 0x00, 0x00, 0x00, 0x00, 0x36, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        MotionPacket { x: 0, y: 0, z: -202, rx: 0, ry: 0, rz: 0 },
    )]
    #[case::push_down(
        &[0x01, 0x00, 0x00, 0x00, 0x00, 0x1E, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        MotionPacket { x: 0, y: 0, z: 286, rx: 0, ry: 0, rz: 0 },
    )]
    #[case::rotate_forward(
        &[0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x6f, 0xff, 0x00, 0x00, 0x00, 0x00],
        MotionPacket { x: 0, y: 0, z: 0, rx: -145, ry: 0, rz: 0 },
    )]
    #[case::rotate_right(
        &[0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF7, 0xFE, 0x00, 0x00],
        MotionPacket { x: 0, y: 0, z: 0, rx: 0, ry: -265, rz: 0 },
    )]
    #[case::spin_right(
        &[0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x9C, 0x00],
        MotionPacket { x: 0, y: 0, z: 0, rx: 0, ry: 0, rz: 156 },
    )]
    fn read_motion_packet(#[case] data: &[u8; 13], #[case] expected: MotionPacket) {
        let frame = Frame::read(&mut Cursor::new(data)).unwrap();

        assert!(matches!(
            frame,
            Frame {
                packet: Packet::Motion(_)
            }
        ));
        if let Frame {
            packet: Packet::Motion(packet),
            ..
        } = frame
        {
            assert_eq!(packet, expected);
        }
    }

    #[test]
    fn read_battery_packet() {
        let data: &[u8; 13] = &[
            0x17, 0x5E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let frame = Frame::read(&mut Cursor::new(data)).unwrap();

        assert!(matches!(
            frame,
            Frame {
                packet: Packet::Battery(_)
            }
        ));
        if let Frame {
            packet: Packet::Battery(packet),
            ..
        } = frame
        {
            assert_eq!(packet, 94);
        }
    }
}
