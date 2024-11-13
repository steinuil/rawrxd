pub trait HeaderSize: Sized {
    fn header_size(&self) -> u64;
}

pub trait DataSize: Sized {
    fn data_size(&self) -> u64 {
        0
    }
}

pub trait FullSize: Sized {
    fn full_size(&self) -> u64;
}

impl<T: HeaderSize + DataSize> FullSize for T {
    fn full_size(&self) -> u64 {
        self.header_size() + self.data_size()
    }
}
