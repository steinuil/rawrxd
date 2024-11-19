use std::io;

use crate::{
    error::{Error, RarResult},
    size::BlockSize as _,
};

use super::{Block, BlockKind};

#[derive(Debug)]
pub struct BlockIterator<R: io::Read + io::Seek> {
    reader: R,
    file_size: u64,
    next_offset: u64,
    end_of_archive_reached: bool,
}

impl<R: io::Read + io::Seek> BlockIterator<R> {
    pub fn new(mut reader: R, offset: u64) -> RarResult<Self> {
        let file_size = reader.seek(io::SeekFrom::End(0))?;

        Ok(Self {
            reader,
            file_size,
            next_offset: offset,
            end_of_archive_reached: false,
        })
    }

    fn read_block(&mut self) -> RarResult<Block> {
        self.reader.seek(io::SeekFrom::Start(self.next_offset))?;

        let block = Block::read(&mut self.reader)?;

        if block.size() == 0
            || block.offset() + block.header_size() > self.file_size
            || block.offset() + block.size() > self.file_size
        {
            return Err(Error::CorruptHeader);
        }

        self.next_offset = block.offset() + block.size();

        if let BlockKind::EndArchive(_) = block.kind {
            self.end_of_archive_reached = true;
        }

        Ok(block)
    }
}

impl<R: io::Read + io::Seek> Iterator for BlockIterator<R> {
    type Item = RarResult<Block>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.end_of_archive_reached {
            return None;
        }

        if self.next_offset == self.file_size {
            return None;
        }

        Some(self.read_block())
    }
}
