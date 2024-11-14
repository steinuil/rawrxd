use crate::{rar14, rar15, rar50, size::BlockSize};

#[derive(Debug)]
pub enum Block {
    Rar14(rar14::Block),
    Rar15(rar15::Block),
    Rar50(Box<rar50::Block>),
}

#[derive(Debug)]
pub enum HashKind {
    Crc16(u16),
    Crc32(u32),
}

impl BlockSize for Block {
    fn position(&self) -> u64 {
        match self {
            Block::Rar14(b) => b.position(),
            Block::Rar15(b) => b.position(),
            Block::Rar50(b) => b.position(),
        }
    }

    fn header_size(&self) -> u64 {
        match self {
            Block::Rar14(b) => b.header_size(),
            Block::Rar15(b) => b.header_size(),
            Block::Rar50(b) => b.header_size(),
        }
    }

    fn data_size(&self) -> u64 {
        match self {
            Block::Rar14(b) => b.data_size(),
            Block::Rar15(b) => b.data_size(),
            Block::Rar50(b) => b.data_size(),
        }
    }
}

impl Block {
    pub fn header_hash(&self) -> Option<HashKind> {
        match self {
            Block::Rar14(_) => None,
            Block::Rar15(b) => Some(HashKind::Crc16(b.header_crc16)),
            Block::Rar50(b) => Some(HashKind::Crc32(b.header_crc32)),
        }
    }
}
