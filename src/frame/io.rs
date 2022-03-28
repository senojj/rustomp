use std::cell::RefCell;
use std::io;
use std::io::Read;
use std::rc::Rc;

pub struct LimitedReader<R: Read> {
    reader: Rc<RefCell<R>>,
    limit: u64,
}

impl<R: Read> LimitedReader<R> {
    pub fn new(reader: Rc<RefCell<R>>, limit: u64) -> Self {
        LimitedReader { reader, limit }
    }
}

impl<R: Read> Read for LimitedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.limit == 0 {
            return Ok(0);
        }

        let local_buf = if (buf.len() as u64) > self.limit {
            &mut buf[..self.limit as usize]
        } else {
            buf
        };
        let mut reader = self.reader.borrow_mut();
        let result = reader.read(local_buf);

        if let Ok(v) = result {
            self.limit -= v as u64
        }
        result
    }
}

pub struct DelimitedReader<R: Read> {
    inner: Rc<RefCell<R>>,
    delim: u8,
    done: bool,
}

impl<R: Read> DelimitedReader<R> {
    pub fn new(reader: Rc<RefCell<R>>, delimiter: u8) -> Self {
        DelimitedReader {
            inner: reader,
            delim: delimiter,
            done: false,
        }
    }
}

impl<R: Read> Read for DelimitedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.done {
            return Ok(0);
        }
        let mut local_buf: [u8; 1] = [0];
        let mut total_bytes_read = 0;

        let mut reader = self.inner.borrow_mut();

        for x in buf {
            let bytes_read = reader.read(&mut local_buf)?;

            if bytes_read < 1 {
                break;
            }
            *x = local_buf[0];
            total_bytes_read += bytes_read;

            if local_buf[0] == self.delim {
                self.done = true;
                break;
            }
        }
        Ok(total_bytes_read)
    }
}

pub struct BiReader<R1: Read, R2: Read> {
    first: R1,
    second: R2,
}

impl<R1: Read, R2: Read> BiReader<R1, R2> {
    pub fn new(first: R1, second: R2) -> Self {
        BiReader { first, second }
    }
}

impl<R1: Read, R2: Read> Read for BiReader<R1, R2> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let first_bytes_read = self.first.read(buf)?;

        if first_bytes_read > 0 {
            return Ok(first_bytes_read);
        }
        self.second.read(buf)
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
        let cell = Rc::new(RefCell::new(Cursor::new(input)));

        let mut dreader = DelimitedReader::new(cell.clone(), b';');
        let mut buffer: Vec<u8> = Vec::new();
        Read::read_to_end(&mut dreader, &mut buffer).unwrap();
        let output = str::from_utf8(&buffer).unwrap();
        let target = "this is;";
        assert_eq!(target, output)
    }

    #[test]
    fn delimited_reader_none() {
        let input = b"this is a test";
        let cell = Rc::new(RefCell::new(Cursor::new(input)));

        let mut dreader = DelimitedReader::new(cell.clone(), b';');
        let mut buffer: Vec<u8> = Vec::new();
        Read::read_to_end(&mut dreader, &mut buffer).unwrap();
        let output = str::from_utf8(&buffer).unwrap();
        let target = "this is a test";
        assert_eq!(target, output)
    }

    #[test]
    fn delimited_reader_beginning() {
        let input = b";this is a test";
        let cell = Rc::new(RefCell::new(Cursor::new(input)));

        let mut dreader = DelimitedReader::new(cell.clone(), b';');
        let mut buffer: Vec<u8> = Vec::new();
        Read::read_to_end(&mut dreader, &mut buffer).unwrap();
        let output = str::from_utf8(&buffer).unwrap();
        let target = ";";
        assert_eq!(target, output)
    }

    #[test]
    fn bi_reader_read() {
        let input = b"this is a test; that already ended";
        let cell = Rc::new(RefCell::new(Cursor::new(input)));

        let limited_reader = LimitedReader::new(cell.clone(), 3);
        let delimited_reader = DelimitedReader::new(cell.clone(), b';');
        let mut bi_reader = BiReader::new(limited_reader, delimited_reader);

        let mut buffer: Vec<u8> = Vec::new();
        Read::read_to_end(&mut bi_reader, &mut buffer).unwrap();
        let output = str::from_utf8(&buffer).unwrap();
        let target = "this is a test;";
        assert_eq!(target, output)
    }
}
