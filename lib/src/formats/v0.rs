use binrw::prelude::*;

#[binread]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[br(little, repr = u8)]
pub enum PacketKind {
    #[default]
    Empty = 0x00,
    Translate = 0x01,
    Rotate = 0x02,
    // Button = 0x03, // need to test with my wired device after i swap with my friend again
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
    #[br(pre_assert(kind == PacketKind::Translate))]
    Translate(XYZPacket),
    #[br(pre_assert(kind == PacketKind::Rotate))]
    Rotate(XYZPacket),
}

#[binread]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[br(little)]
pub struct XYZPacket {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

#[binread]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[br(little)]
pub struct ButtonPacket {}
