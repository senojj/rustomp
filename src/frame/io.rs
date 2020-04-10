use std::io;
use std::io::{Read, BufRead};

pub struct DelimitedReader<R: BufRead> {
    inner: R,
    delim: u8,
    done: bool,
}

impl<R: BufRead> DelimitedReader<R> {
    pub fn new(reader: R, delimiter: u8) -> Self {
        DelimitedReader {
            inner: reader,
            delim: delimiter,
            done: false,
        }
    }
}

impl<R: BufRead> Read for DelimitedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.done {
            return Ok(0);
        }
        let mut available = self.inner.fill_buf()?;

        let (found, used) = match memchr::memchr(self.delim, available) {
            Some(i) => {
                self.done = true;
                (true, (&available[..i]).read(buf)? + 1)
            }
            None => {
                (false, available.read(buf)?)
            }
        };
        self.inner.consume(used);

        if found {
            return Ok(used - 1);
        }
        return Ok(used);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::str;
    use std::io::Cursor;

    #[test]
    fn delimited_reader_middle() {
        let input = b"this is; a test";
        let mut reader = Cursor::new(input);

        let mut dreader = DelimitedReader::new(&mut reader, b';');
        let mut buffer: Vec<u8> = Vec::new();
        Read::read_to_end(&mut dreader, &mut buffer).unwrap();
        let output = str::from_utf8(&buffer).unwrap();
        let target = "this is";
        assert_eq!(target, output)
    }

    #[test]
    fn delimited_reader_none() {
        let input = b"this is a test";
        let mut reader = Cursor::new(input);

        let mut dreader = DelimitedReader::new(&mut reader, b';');
        let mut buffer: Vec<u8> = Vec::new();
        Read::read_to_end(&mut dreader, &mut buffer).unwrap();
        let output = str::from_utf8(&buffer).unwrap();
        let target = "this is a test";
        assert_eq!(target, output)
    }

    #[test]
    fn delimited_reader_beginning() {
        let input = b";this is a test";
        let mut reader = Cursor::new(input);

        let mut dreader = DelimitedReader::new(&mut reader, b';');
        let mut buffer: Vec<u8> = Vec::new();
        Read::read_to_end(&mut dreader, &mut buffer).unwrap();
        let output = str::from_utf8(&buffer).unwrap();
        let target = "";
        assert_eq!(target, output)
    }
}