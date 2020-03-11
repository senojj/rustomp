use std::io;
use std::io::Read;

pub struct DelimitedReader<R: Read> {
    reader: R,
    delimiter: u8,
    done: bool,
}

impl<R: Read> DelimitedReader<R> {
    pub fn new(r: R, del: u8) -> Self {
        DelimitedReader{
            reader: r,
            delimiter: del,
            done: false,
        }
    }
}

impl<R: Read> Read for DelimitedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.done {
            return Ok(0)
        }
        let mut total_read: usize = 0;
        let mut inner_buffer: [u8; 1] = [b'\0'];

        let mut ctr = 0;

        while ctr < buf.len() {
            let bread = self.reader.read(&mut inner_buffer)?;

            if bread > 0 {
                if inner_buffer[0] == self.delimiter {
                    self.done = true;
                    return Ok(total_read)
                }
                total_read += bread;
                buf[ctr] = inner_buffer[0];
            }
            ctr += 1;
        }
        Ok(total_read)
    }
}

pub struct MultiReader<A: Read, B: Read> {
    reader_a: A,
    reader_b: B,
}

impl<A: Read, B: Read> MultiReader<A, B> {
    pub fn new(a: A, b: B) -> Self {
        MultiReader{
            reader_a: a,
            reader_b: b,
        }
    }
}

impl<A: Read, B: Read> Read for MultiReader<A, B> {
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
        let reader = Cursor::new(input);

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
        let reader = Cursor::new(input);

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
        let reader = Cursor::new(input);

        let mut dreader = DelimitedReader::new(reader, b';');
        let mut buffer: Vec<u8> = Vec::new();
        Read::read_to_end(&mut dreader, &mut buffer).unwrap();
        let output = str::from_utf8(&buffer).unwrap();
        let target = "";
        assert_eq!(target, output)
    }
}