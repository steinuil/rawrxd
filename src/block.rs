use crate::size::{DataSize, HeaderSize};

pub trait RarBlock: HeaderSize + DataSize {
    fn position(&self) -> u64;
}
