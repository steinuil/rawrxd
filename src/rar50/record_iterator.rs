use std::io;

use crate::{read::*, Error, RarResult};

pub struct CommonRecord {
    pub record_type: u64,
    pub data: io::Cursor<Vec<u8>>,
}

pub struct RecordIterator<'a, R: io::Read + io::Seek> {
    reader: &'a mut R,
    end_offset: u64,
    next_record_offset: u64,
}

impl<'r, R: io::Read + io::Seek> RecordIterator<'r, R> {
    const SERVICE_DATA: u64 = 0x07;
    const MIN_RECORD_SIZE: u64 = 2;

    pub fn new(reader: &'r mut R, header_size: u64, extra_area_size: u64) -> RarResult<Self> {
        let start = header_size
            .checked_sub(extra_area_size)
            .ok_or(Error::CorruptHeader)?;

        // We expect this record iterator to be called from exactly the start of the extra fields.
        // If the position doesn't match, the extra area overlaps the header, or we
        // have a bug in the header parsing. Either way, we should bail out.
        //
        // unrar seems to just ignore the extra area in this case.
        if reader.stream_position()? > start {
            return Err(Error::CorruptHeader);
        }

        Ok(Self {
            reader,
            end_offset: header_size,
            next_record_offset: start,
        })
    }

    fn read_record(&mut self) -> RarResult<CommonRecord> {
        self.reader
            .seek(io::SeekFrom::Start(self.next_record_offset))?;

        let (record_size, size_byte_size) = read_vint(self.reader)?;

        let record_start = self
            .next_record_offset
            .checked_add(size_byte_size as u64)
            .ok_or(Error::CorruptHeader)?;
        let record_end = record_start
            .checked_add(record_size)
            .ok_or(Error::CorruptHeader)?;

        if record_size == 0 || record_end > self.end_offset {
            return Err(Error::CorruptHeader);
        }

        let (record_type, type_byte_size) = read_vint(self.reader)?;

        // RAR 5.12 and earlier wrote the service data record 1 byte too short.
        let record_end = if record_type == Self::SERVICE_DATA && self.end_offset - record_end == 1 {
            record_end + 1
        } else {
            record_end
        };

        let data_size = record_end
            .checked_sub(record_start + type_byte_size as u64)
            .ok_or(Error::CorruptHeader)?;

        let data = read_vec(self.reader, data_size as usize)?;

        self.next_record_offset = record_end;

        Ok(CommonRecord {
            record_type,
            data: io::Cursor::new(data),
        })
    }
}

impl<R: io::Read + io::Seek> Iterator for RecordIterator<'_, R> {
    type Item = RarResult<CommonRecord>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.end_offset.saturating_sub(self.next_record_offset) < Self::MIN_RECORD_SIZE {
            return None;
        }

        Some(self.read_record())
    }
}
