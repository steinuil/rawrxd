use std::io;

#[derive(Debug)]
pub struct Bounded<'a, R: io::Read> {
    inner: &'a mut R,
    limit: u64,
}

impl<'a, R: io::Read> Bounded<'a, R> {
    pub fn new(inner: &'a mut R, limit: u64) -> Self {
        Self { inner, limit }
    }
}

impl<'a, R: io::Read> io::Read for Bounded<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.limit == 0 {
            return Ok(0);
        }

        let max = buf
            .len()
            .min(usize::try_from(self.limit).unwrap_or(usize::MAX));
        let n = self.inner.read(&mut buf[..max])?;
        self.limit -= n as u64;

        Ok(n)
    }
}
