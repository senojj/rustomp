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
            IO(err) => err.fmt(f),
            Encoding(err) => err.fmt(f),
            Format(string) => string.fmt(f),
        }
    }
}

impl error::Error for ReadError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use self::ReadError::*;

        match self {
            IO(err) => Some(err),
            Encoding(err) => Some(err),
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