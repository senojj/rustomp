use std::cell::RefMut;
use std::io;
use std::io::Read;

pub struct LimitedReader<'a, R: Read> {
    reader: RefMut<'a, R>,
    limit: u64,
}

impl<'a, R: Read> LimitedReader<'a, R> {
    pub fn new(reader: RefMut<'a, R>, limit: u64) -> Self {
        LimitedReader { reader, limit }
    }
}

impl<'a, R: Read> Read for LimitedReader<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.limit == 0 {
            return Ok(0);
        }

        let local_buf = if (buf.len() as u64) > self.limit {
            &mut buf[..self.limit as usize]
        } else {
            buf
        };
        let result = self.reader.read(local_buf);
        match result {
            Ok(v) => self.limit -= v as u64,
            _ => (),
        }
        return result;
    }
}

pub struct DelimitedReader<'a, R: Read> {
    inner: RefMut<'a, R>,
    delim: u8,
    done: bool,
}

impl<'a, R: Read> DelimitedReader<'a, R> {
    pub fn new(reader: RefMut<'a, R>, delimiter: u8) -> Self {
        DelimitedReader {
            inner: reader,
            delim: delimiter,
            done: false,
        }
    }
}

impl<'a, R: Read> Read for DelimitedReader<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.done {
            return Ok(0);
        }
        let mut local_buf: [u8; 1] = [0];
        let mut total_bytes_read = 0;

        for x in 0..buf.len() {
            let bytes_read = self.inner.read(&mut local_buf)?;
            if bytes_read > 0 {
                if local_buf[0] == self.delim {
                    self.done = true;
                    return Ok(total_bytes_read);
                } else {
                    buf[x] = local_buf[0];
                }
            } else {
                break;
            }
            total_bytes_read += bytes_read;
        }
        Ok(total_bytes_read)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::cell::RefCell;
    use std::io::Cursor;
    use std::str;

    #[test]
    fn delimited_reader_middle() {
        let input = b"this is; a test";
        let cell = RefCell::new(Cursor::new(input));
        let mut reader = cell.borrow_mut();
        let mut dreader = DelimitedReader::new(reader, b';');
        let mut buffer: Vec<u8> = Vec::new();
        Read::read_to_end(&mut dreader, &mut buffer).unwrap();
        let output = str::from_utf8(&buffer).unwrap();
        let target = "this is";
        assert_eq!(target, output)
    }

    #[test]
    fn delimited_reader_none() {
        let input = b"this is a test";
        let cell = RefCell::new(Cursor::new(input));
        let mut reader = cell.borrow_mut();

        let mut dreader = DelimitedReader::new(reader, b';');
        let mut buffer: Vec<u8> = Vec::new();
        Read::read_to_end(&mut dreader, &mut buffer).unwrap();
        let output = str::from_utf8(&buffer).unwrap();
        let target = "this is a test";
        assert_eq!(target, output)
    }

    #[test]
    fn delimited_reader_beginning() {
        let input = b";this is a test";
        let cell = RefCell::new(Cursor::new(input));
        let mut reader = cell.borrow_mut();

        let mut dreader = DelimitedReader::new(reader, b';');
        let mut buffer: Vec<u8> = Vec::new();
        Read::read_to_end(&mut dreader, &mut buffer).unwrap();
        let output = str::from_utf8(&buffer).unwrap();
        let target = "";
        assert_eq!(target, output)
    }
}
