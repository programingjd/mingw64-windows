use std::io::Error as StdIoError;
use std::result::Result as StdResult;

#[derive(Debug)]
pub enum Error {
    IOError(StdIoError),
    DownloadError,
    DecompressionError,
    ParseError,
}

pub type Result<T> = StdResult<T, Error>;

impl From<StdIoError> for Error {
    fn from(err: StdIoError) -> Self {
        Error::IOError(err)
    }
}
