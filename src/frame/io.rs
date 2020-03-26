use std::io;
use std::io::Read;

pub struct DelimitedReader<R: Read> {
    reader: R,
    delimiter: u8,
    done: bool,
}

impl<R: Read> DelimitedReader<R> {
    pub fn new(reader: R, delimiter: u8) -> Self {
        DelimitedReader {
            reader,
            delimiter,
            done: false,
        }
    }
}

impl<R: Read> Read for DelimitedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.done {
            return Ok(0);
        }
        let mut total_read: usize = 0;
        let mut inner_buffer: [u8; 1] = [b'\0'];

        let mut ctr = 0;

        while ctr < buf.len() {
            let bytes_read = self.reader.read(&mut inner_buffer)?;

            if bytes_read > 0 {
                if inner_buffer[0] == self.delimiter {
                    self.done = true;
                    return Ok(total_read);
                }
                total_read += bytes_read;
                buf[ctr] = inner_buffer[0];
            }
            ctr += 1;
        }
        Ok(total_read)
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