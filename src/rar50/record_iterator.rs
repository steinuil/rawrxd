use std::io;

use crate::read::*;

pub struct CommonRecord {
    pub record_type: u64,
    pub data: io::Cursor<Vec<u8>>,
}

pub struct RecordIterator<'a, R: io::Read + io::Seek> {
    reader: &'a mut R,
    end_offset: u64,
    next_record_offset: u64,
}

impl<'a, R: io::Read + io::Seek> RecordIterator<'a, R> {
    pub fn new(reader: &'a mut R, extra_area_size: u64) -> io::Result<Self> {
        let offset = reader.stream_position()?;
        let end_offset = offset + extra_area_size;
        let next_record_offset = offset;

        Ok(Self {
            reader,
            end_offset,
            next_record_offset,
        })
    }

    fn read_record(&mut self) -> io::Result<CommonRecord> {
        self.reader
            .seek(io::SeekFrom::Start(self.next_record_offset))?;

        let (record_size, byte_size) = read_vint(self.reader)?;
        let (record_type, type_byte_size) = read_vint(self.reader)?;

        let data = read_vec(self.reader, record_size as usize - type_byte_size as usize)?;

        self.next_record_offset += record_size + byte_size as u64;

        Ok(CommonRecord {
            record_type,
            data: io::Cursor::new(data),
        })
    }
}

impl<'a, R: io::Read + io::Seek> Iterator for RecordIterator<'a, R> {
    type Item = io::Result<CommonRecord>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_record_offset >= self.end_offset {
            return None;
        }

        Some(self.read_record())
    }
}
