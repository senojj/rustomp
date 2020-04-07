use std::io;
use std::io::Read;
use crate::frame::NULL;

pub struct DelimitedReader<R: Read> {
    reader: R,
    delimiter: String,
    search_window: Vec<u8>,
    done: bool,
}

impl<R: Read> DelimitedReader<R> {
    pub fn new<T: Into<String>>(reader: R, delimiter: T) -> Self {
        let delim = delimiter.into();
        let byte_length = delim.as_bytes().len();

        DelimitedReader {
            reader,
            delimiter: delim,
            search_window: Vec::with_capacity(byte_length),
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
                self.search_window.push(inner_buffer[0]);

                if self.search_window == delimiter_bytes {
                    self.done = true;
                    return Ok(total_read);
                }
            }
            let mut popped_value: Option<u8> = None;

            if self.search_window.capacity() == self.search_window.len() {
                let split_result = self.search_window.split_first();

                match split_result {
                    Some(split) => {
                        buf[ndx] = *split.0;
                        ndx += 1;
                        total_read += 1;

                        let mut new_vec: Vec<u8> = Vec::with_capacity(delimiter_bytes.len());
                        let mut temp_vec = Vec::from(split.1);
                        new_vec.append(&mut temp_vec);
                        self.search_window = new_vec;
                    },
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
        let input = b";this is a test";
        let mut reader = Cursor::new(input);

        let mut dreader = DelimitedReader::new(&mut reader, ";");
        let mut buffer: Vec<u8> = Vec::new();
        Read::read_to_end(&mut dreader, &mut buffer).unwrap();
        let output = str::from_utf8(&buffer).unwrap();
        let target = "";
        assert_eq!(target, output)
    }
}