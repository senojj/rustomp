use std::collections::BTreeMap;
use std::io::{Write, Read};
use std::io;
use std::error;
use std::io::BufWriter;
use std::str;
use std::fmt;

const NULL: char = '\0';
const BACKSLASH: char = '\\';
const NEWLINE: char = '\n';
const CARRIAGE_RETURN: char = '\r';
const COLON: char = ':';

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
            Format(string) => None,
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

pub enum Command {
    Connect,
    Stomp,
    Connected,
    Send,
    Subscribe,
    Unsubscribe,
    Ack,
    Nack,
    Begin,
    Commit,
    Abort,
    Disconnect,
    Message,
    Receipt,
    Error,
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use self::Command::*;

        let value = match self {
            Connect => "CONNECT",
            Stomp => "STOMP",
            Connected => "CONNECTED",
            Send => "SEND",
            Subscribe => "SUBSCRIBE",
            Unsubscribe => "UNSUBSCRIBE",
            Ack => "ACK",
            Nack => "NACK",
            Begin => "BEGIN",
            Commit => "COMMIT",
            Abort => "ABORT",
            Disconnect => "DISCONNECT",
            Message => "MESSAGE",
            Receipt => "RECEIPT",
            Error => "ERROR",
        };

        write!(f, "{}", value)
    }
}

fn encode(input: &str) -> String {
    let mut output = String::with_capacity(input.len());

    for c in input.chars() {
        match c {
            BACKSLASH => output.push_str("\\\\"),
            CARRIAGE_RETURN => output.push_str("\\r"),
            NEWLINE => output.push_str("\\n"),
            COLON => output.push_str("\\c"),
            a => output.push(a),
        }
    }
    output
}

fn decode(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut last_char = NULL;

    for c in input.chars() {
        match c {
            'c' if last_char == BACKSLASH => output.push_str(":"),
            'n' if last_char == BACKSLASH => output.push_str("\n"),
            'r' if last_char == BACKSLASH => output.push_str("\r"),
            BACKSLASH if last_char == BACKSLASH => output.push_str("\\"),
            BACKSLASH => (),
            a => output.push(a),
        }
        last_char = if last_char == BACKSLASH && c == BACKSLASH { NULL } else { c }
    }
    output
}

struct DelimitedReader<R: Read> {
    reader: R,
    delimiter: u8,
    done: bool,
}

impl<R: Read> DelimitedReader<R> {
    fn new(r: R, del: u8) -> Self {
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

#[derive(Default, PartialEq, Debug)]
pub struct Header {
    fields: BTreeMap<String, Vec<String>>,
}

impl Header {
    pub fn new() -> Self {
        Header {
            fields: BTreeMap::new(),
        }
    }

    pub fn add<T: Into<String>>(&mut self, key: T, value: T) {
        self.fields
            .entry(key.into())
            .or_insert_with(|| Vec::with_capacity(1))
            .push(value.into());
    }

    pub fn set<T: Into<String>>(&mut self, key: T, values: Vec<String>) {
        let mut c = Vec::with_capacity(values.len());

        for v in values {
            c.push(v);
        }

        self.fields.insert(key.into(), c);
    }

    pub fn remove(&mut self, key: &str) {
        self.fields.remove(key);
    }

    pub fn write_to<W: Write>(&self, w: &mut W) -> io::Result<u64> {
        let mut bw = BufWriter::new(w);
        let mut bytes_written: u64 = 0;

        for (k, v) in self.fields.iter() {
            let field_str = format!("{}: {}\n", encode(k), encode(&v.join(",")));
            let size = bw.write(field_str.as_bytes())?;
            bytes_written += size as u64;
        }
        bw.flush().and(Ok(bytes_written))
    }

    pub fn read_from<R: Read>(r: &mut R) -> Result<Self, ReadError> {
        let mut limited_reader = r.take(1024 * 1000);
        let mut header = Self::new();

        loop {
            let mut delimited_reader = DelimitedReader::new(&mut limited_reader, b'\n');
            let mut buffer: Vec<u8> = Vec::new();
            let bytes_read = Read::read_to_end(&mut delimited_reader, &mut buffer)?;

            if bytes_read < 1 {
                break;
            }
            let line = str::from_utf8(&buffer)?;
            let parts: Vec<&str> = line.split(':').collect();

            if parts.len() < 2 {
                return Err(ReadError::Format(String::from("invalid header field format")))
            }
            let field_name = decode(parts[0]);
            let field_value = decode(parts[1]);

            header.add(field_name.trim(), field_value.trim_start())

        }
        Ok(header)
    }
}

pub struct Frame<R: Read> {
    pub command: Command,
    pub header: Header,
    pub body: R,
}

impl<R: Read> Frame<R> {
    pub fn new(command: Command, body: R) -> Self {
        Frame {
            command,
            header: Header::new(),
            body,
        }
    }

    pub fn write_to<W: Write>(&mut self, w: &mut W) -> io::Result<u64> {
        let mut bw = BufWriter::new(w);
        let mut bytes_written: u64 = 0;
        bytes_written += bw.write(self.command.to_string().as_bytes())? as u64;
        bytes_written += bw.write(b"\n")? as u64;
        bytes_written += self.header.write_to(&mut bw)?;
        bytes_written += bw.write(b"\n")? as u64;
        bytes_written += io::copy(&mut self.body, &mut bw)?;
        bytes_written += bw.write(b";")? as u64;

        bw.flush().and(Ok(bytes_written))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn encode_backslash() {
        let input = "Hello\\World";
        let target = "Hello\\\\World";
        assert_eq!(target, encode(input))
    }

    #[test]
    fn encode_carriage_return() {
        let input = "Hello\rWorld";
        let target = "Hello\\rWorld";
        assert_eq!(target, encode(input))
    }

    #[test]
    fn encode_newline() {
        let input = "Hello\nWorld";
        let target = "Hello\\nWorld";
        assert_eq!(target, encode(input))
    }

    #[test]
    fn encode_semicolon() {
        let input = "Hello:World";
        let target = "Hello\\cWorld";
        assert_eq!(target, encode(input))
    }

    #[test]
    fn decode_backslash() {
        let input = "Hello\\\\World";
        let target = "Hello\\World";
        assert_eq!(target, decode(input))
    }

    #[test]
    fn decode_newline() {
        let input = "Hello\\nWorld";
        let target = "Hello\nWorld";
        assert_eq!(target, decode(input))
    }

    #[test]
    fn decode_backslash_newline() {
        let input = "Hello\\\\\\nWorld";
        let target = "Hello\\\nWorld";
        assert_eq!(target, decode(input))
    }

    #[test]
    fn decode_colon() {
        let input = "Hello\\cWorld";
        let target = "Hello:World";
        assert_eq!(target, decode(input))
    }

    #[test]
    fn decode_carriage_return() {
        let input = "Hello\\rWorld";
        let target = "Hello\rWorld";
        assert_eq!(target, decode(input))
    }

    #[test]
    fn write_header() {
        let target = "Content-Length: 30\nContent-Type: application/json\n";

        let mut header = Header::new();
        header.add("Content-Type", "application/json");
        header.add("Content-Length", "30");

        let mut buffer: Vec<u8> = Vec::new();
        header.write_to(&mut buffer).unwrap();
        let data = str::from_utf8(&buffer).unwrap();
        assert_eq!(target, data)
    }

    #[test]
    fn write_header_encode_colon() {
        let target = "Content-Length: 30\nContent-Type: vnd\\capplication/json\n";

        let mut header = Header::new();
        header.add("Content-Type", "vnd:application/json");
        header.add("Content-Length", "30");

        let mut buffer: Vec<u8> = Vec::new();
        header.write_to(&mut buffer).unwrap();
        let data = str::from_utf8(&buffer).unwrap();
        assert_eq!(target, data)
    }

    #[test]
    fn write_frame() {
        let target = "CONNECT\nContent-Length: 30\nContent-Type: application/json\n\n;";

        let mut frame = Frame::new(Command::Connect, io::empty());
        frame.header.add("Content-Type", "application/json");
        frame.header.add("Content-Length", "30");

        let mut buffer: Vec<u8> = Vec::new();
        frame.write_to(&mut buffer).unwrap();
        let data = str::from_utf8(&buffer).unwrap();
        assert_eq!(target, data)
    }

    #[test]
    fn write_frame_with_body() {
        let target = "CONNECT\nContent-Length: 30\nContent-Type: application/json\n\n{\"name\":\"Joshua\"};";

        let mut frame = Frame::new(Command::Connect, Cursor::new(b"{\"name\":\"Joshua\"}"));
        frame.header.add("Content-Type", "application/json");
        frame.header.add("Content-Length", "30");

        let mut buffer: Vec<u8> = Vec::new();
        frame.write_to(&mut buffer).unwrap();
        let data = str::from_utf8(&buffer).unwrap();
        assert_eq!(target, data)
    }

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

    #[test]
    fn read_header() {
        let input = b"Content-Type: application/json\nContent-Length: 30\nName: Joshua\n";
        let mut reader: Cursor<&[u8]> = Cursor::new(&input[..]);
        let header = Header::read_from(&mut reader).unwrap();

        let mut target = Header::new();
        target.add("Content-Type", "application/json");
        target.add("Content-Length", "30");
        target.add("Name", "Joshua");
        assert_eq!(target, header);
    }
}