use std::error;
use std::fmt;
use std::io;
use std::str;

#[derive(Debug)]
pub enum ReadError {
    IO(io::Error),
    Encoding(str::Utf8Error),
    Format(String),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ReadError::*;

        match self {
            IO(e) => e.fmt(f),
            Encoding(e) => e.fmt(f),
            Format(s) => s.fmt(f),
        }
    }
}

impl error::Error for ReadError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use self::ReadError::*;

        match self {
            IO(e) => Some(e),
            Encoding(e) => Some(e),
            Format(_) => None,
        }
    }
}

impl std::convert::From<io::Error> for ReadError {
    fn from(error: io::Error) -> Self {
        ReadError::IO(error)
    }
}

impl std::convert::From<str::Utf8Error> for ReadError {
    fn from(error: str::Utf8Error) -> Self {
        ReadError::Encoding(error)
    }
}