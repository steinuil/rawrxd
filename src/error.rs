use std::io;

use thiserror::Error;

pub type RarResult<T> = core::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("expected more data")]
    UnexpectedEof,

    #[error("header reported sizes are 0 or exceed the EOF")]
    CorruptHeader,

    #[error(transparent)]
    Io(io::Error),
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        match value.kind() {
            io::ErrorKind::UnexpectedEof => Self::UnexpectedEof,
            _ => Self::Io(value),
        }
    }
}
