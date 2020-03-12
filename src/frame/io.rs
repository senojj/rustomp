use std::io;
use std::io::Read;
use std::cmp::min;

pub trait ReadFrom<R: Read> {
    fn read_from(&mut self, reader: &mut R, buffer: &mut [u8]) -> io::Result<usize>;

    fn read_to_end(&mut self, reader: &mut R, buffer: &mut Vec<u8>) -> io::Result<usize> {
        let mut total_bytes_read: usize = 0;
        let mut bytes_read = self.read_from(reader, buffer)?;

        while bytes_read > 0 {
            total_bytes_read += bytes_read;
            bytes_read = self.read_from(reader, buffer)?;
        };
        Ok(total_bytes_read)
    }
}

pub struct ReadFromReader<'a, T: Read, R: ReadFrom<T>> {
    inner: &'a mut R,
    reader: &'a mut T,
}

impl<'a, T: Read, R: ReadFrom<T>> ReadFromReader<'a, T, R> {
    pub fn new(inner: &'a mut R, reader: &'a mut T) -> Self {
        ReadFromReader {
            inner,
            reader,
        }
    }
}

impl<'a, T: Read, R: ReadFrom<T>> Read for ReadFromReader<'a, T, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read_from(self.reader, buf)
    }
}

pub struct TakeReadFrom {
    limit: u64,
}

impl TakeReadFrom {
    pub fn new(limit: u64) -> Self {
        TakeReadFrom {
            limit,
        }
    }
}

impl<R: Read> ReadFrom<R> for TakeReadFrom {
    fn read_from(&mut self, reader: &mut R, buffer: &mut [u8]) -> io::Result<usize> {
        if self.limit == 0 {
            return Ok(0);
        }
        let max = min(buffer.len() as u64, self.limit) as usize;
        let bytes_read = reader.read(&mut buffer[..max])?;
        self.limit -= bytes_read as u64;
        Ok(bytes_read)
    }
}

pub struct DelimitedReadFrom {
    delimiter: u8,
    done: bool,
}

impl DelimitedReadFrom {
    pub fn new(delimiter: u8) -> Self {
        DelimitedReadFrom {
            delimiter,
            done: false,
        }
    }
}

impl<R: Read> ReadFrom<R> for DelimitedReadFrom {
    fn read_from(&mut self, reader: &mut R, buffer: &mut [u8]) -> io::Result<usize> {
        if self.done {
            return Ok(0);
        }
        let mut total_read: usize = 0;
        let mut inner_buffer: [u8; 1] = [b'\0'];

        let mut ctr = 0;

        while ctr < buffer.len() {
            let bread = reader.read(&mut inner_buffer)?;

            if bread > 0 {
                if inner_buffer[0] == self.delimiter {
                    self.done = true;
                    return Ok(total_read);
                }
                total_read += bread;
                buffer[ctr] = inner_buffer[0];
            }
            ctr += 1;
        }
        Ok(total_read)
    }
}

pub struct MultiReader<'a, R: Read, T: Read> {
    reader_a: &'a mut R,
    reader_b: &'a mut T,
}

impl<'a, R: Read, T: Read> MultiReader<'a, R, T> {
    pub fn new(reader_a: &'a mut R, reader_b: &'a mut T) -> Self {
        MultiReader {
            reader_a,
            reader_b,
        }
    }
}

impl<'a, R: Read, T: Read> Read for MultiReader<'a, R, T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bytes_from_a = self.reader_a.read(buf)?;

        if bytes_from_a > 0 {
            Ok(bytes_from_a)
        } else {
            self.reader_b.read(buf)
        }
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

        let mut dreader = DelimitedReadFrom::new(b';');
        let mut buffer: Vec<u8> = Vec::new();
        ReadFrom::read_to_end(&mut dreader, &mut reader, &mut buffer).unwrap();
        let output = str::from_utf8(&buffer).unwrap();
        let target = "this is";
        assert_eq!(target, output)
    }

    #[test]
    fn delimited_reader_none() {
        let input = b"this is a test";
        let mut reader = Cursor::new(input);

        let mut dreader = DelimitedReadFrom::new(b';');
        let mut buffer: Vec<u8> = Vec::new();
        ReadFrom::read_to_end(&mut dreader, &mut reader, &mut buffer).unwrap();
        let output = str::from_utf8(&buffer).unwrap();
        let target = "this is a test";
        assert_eq!(target, output)
    }

    #[test]
    fn delimited_reader_beginning() {
        let input = b";this is a test";
        let mut reader = Cursor::new(input);

        let mut dreader = DelimitedReadFrom::new(b';');
        let mut buffer: Vec<u8> = Vec::new();
        ReadFrom::read_to_end(&mut dreader, &mut reader, &mut buffer).unwrap();
        let output = str::from_utf8(&buffer).unwrap();
        let target = "";
        assert_eq!(target, output)
    }
}