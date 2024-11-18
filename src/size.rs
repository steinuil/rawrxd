pub trait BlockSize {
    fn offset(&self) -> u64;

    fn header_size(&self) -> u64;

    fn data_size(&self) -> u64;

    fn full_size(&self) -> u64 {
        self.header_size() + self.data_size()
    }
}
