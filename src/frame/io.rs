use std::io;
use std::io::Read;
use std::collections::VecDeque;

const NULL: u8 = b'\0';

pub struct DelimitedReader<R: Read> {
    reader: R,
    delimiter: String,
    search_window: VecDeque<u8>,
    done: bool,
}

impl<R: Read> DelimitedReader<R> {
    pub fn new<T: Into<String>>(reader: R, delimiter: T) -> Self {
        let delim = delimiter.into();
        let byte_length = delim.as_bytes().len();

        DelimitedReader {
            reader,
            delimiter: delim,
            search_window: VecDeque::with_capacity(byte_length),
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
        let mut inner_buffer: [u8; 1] = [NULL];

        let mut ctr = 0;
        let mut ndx = 0;

        let delimiter_bytes = self.delimiter.as_bytes();

        while ctr < buf.len() {
            let bytes_read = self.reader.read(&mut inner_buffer)?;

            if bytes_read > 0 {
                self.search_window.push_back(inner_buffer[0]);

                let (front, back) = self.search_window.as_slices();

                if [front, back].concat() == delimiter_bytes {
                    self.done = true;
                    return Ok(total_read);
                }
            }

            if self.search_window.capacity() == self.search_window.len() {
                match self.search_window.pop_front() {
                    Some(head) => {
                        buf[ndx] = head;
                        ndx += 1;
                        total_read += 1;
                    }
                    None => (),
                };
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
        let input = "this 你 is; a test";
        let input_bytes = input.as_bytes();
        let mut reader = Cursor::new(input_bytes);

        let mut dreader = DelimitedReader::new(&mut reader, "你");
        let mut buffer: Vec<u8> = Vec::new();
        Read::read_to_end(&mut dreader, &mut buffer).unwrap();
        let output = str::from_utf8(&buffer).unwrap();
        assert_eq!("this ", output)
    }

    #[test]
    fn delimited_reader_none() {
        let input = b"this is a test";
        let mut reader = Cursor::new(input);

        let mut dreader = DelimitedReader::new(&mut reader, ";");
        let mut buffer: Vec<u8> = Vec::new();
        Read::read_to_end(&mut dreader, &mut buffer).unwrap();
        let output = str::from_utf8(&buffer).unwrap();
        let target = "this is a test";
        assert_eq!(target, output)
    }

    #[test]
    fn delimited_reader_beginning() {
        let input = b"term this is a test";
        let mut reader = Cursor::new(input);

        let mut dreader = DelimitedReader::new(&mut reader, "term");
        let mut buffer: Vec<u8> = Vec::new();
        Read::read_to_end(&mut dreader, &mut buffer).unwrap();
        let output = str::from_utf8(&buffer).unwrap();
        let target = "";
        assert_eq!(target, output)
    }
}