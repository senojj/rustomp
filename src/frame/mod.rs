mod error;
mod io;
mod string;

use crate::frame::io::LimitedReader;
use error::ReadError;
use io::DelimitedReader;
use std::cell::{RefCell, RefMut};
use std::collections::BTreeMap;
use std::fmt;
use std::io as stdio;
use std::io::{BufRead, BufReader, BufWriter};
use std::io::{Read, Write};
use std::ops::DerefMut;
use std::str;
use std::str::FromStr;

const MAX_COMMAND_SIZE: u64 = 1024;
const MAX_HEADER_SIZE: u64 = 1024 * 1000;
const NULL: u8 = b'\0';
const EOL: u8 = b'\n';

#[derive(Debug, PartialEq)]
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

impl FromStr for Command {
    type Err = ReadError;

    fn from_str(s: &str) -> Result<Command, ReadError> {
        use self::Command::*;

        match s {
            "CONNECT" => Ok(Connect),
            "STOMP" => Ok(Stomp),
            "CONNECTED" => Ok(Connected),
            "SEND" => Ok(Send),
            "SUBSCRIBE" => Ok(Subscribe),
            "UNSUBSCRIBE" => Ok(Unsubscribe),
            "ACK" => Ok(Ack),
            "NACK" => Ok(Nack),
            "BEGIN" => Ok(Begin),
            "COMMIT" => Ok(Commit),
            "ABORT" => Ok(Abort),
            "DISCONNECT" => Ok(Disconnect),
            "MESSAGE" => Ok(Message),
            "RECEIPT" => Ok(Receipt),
            "ERROR" => Ok(Error),
            _ => Err("invalid command".into()),
        }
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

    pub fn get_field(&self, key: &str) -> Option<&Vec<String>> {
        self.fields.get(key)
    }

    pub fn add_field<T: Into<String>>(&mut self, key: T, value: T) {
        self.fields
            .entry(key.into())
            .or_insert_with(|| Vec::with_capacity(1))
            .push(value.into());
    }

    pub fn set_field<T: Into<String>>(&mut self, key: T, values: Vec<String>) {
        let mut c = Vec::with_capacity(values.len());

        for v in values {
            c.push(v);
        }

        self.fields.insert(key.into(), c);
    }

    pub fn remove_field(&mut self, key: &str) {
        self.fields.remove(key);
    }

    pub fn write_to<W: Write>(&self, mut w: W) -> stdio::Result<u64> {
        let mut bytes_written: u64 = 0;

        for (k, v) in self.fields.iter() {
            let field_str = format!("{}: {}\n", string::encode(k), string::encode(&v.join(",")));
            let size = w.write(field_str.as_bytes())?;
            bytes_written += size as u64;
        }
        Ok(bytes_written)
    }

    fn read_from<R: Read>(reader: &mut R) -> Result<Self, ReadError> {
        let mut limited_reader = reader.take(MAX_HEADER_SIZE);
        let mut header = Self::new();

        loop {
            let mut buf_reader = BufReader::new(&mut limited_reader);
            let mut buffer: Vec<u8> = Vec::new();
            let bytes_read = buf_reader.read_until(EOL, &mut buffer)?;

            if bytes_read < 1 {
                break;
            }
            let line = str::from_utf8(&buffer)?;
            let clean_line = line.trim_end_matches('\r');
            let parts: Vec<&str> = clean_line.split(':').collect();

            if parts.len() < 2 {
                return Err(format!(
                    "invalid number of header field parts. Expected 2, got {}",
                    parts.len()
                )
                .into());
            }
            let field_name = string::decode(parts[0]);
            let field_value = string::decode(parts[1]);

            let clean_field_name = field_name.trim().to_lowercase();
            let clean_field_value = field_value.trim_start().to_lowercase();

            if clean_field_name.is_empty() {
                return Err("empty header field name".into());
            }
            header.add_field(clean_field_name, clean_field_value);
        }
        Ok(header)
    }
}

pub struct Body<'a> {
    reader: Box<dyn Read + 'a>,
}

impl<'a> Body<'a> {
    pub fn close(&mut self) -> stdio::Result<()> {
        stdio::copy(&mut *self.reader, &mut stdio::sink()).map(|_| ())
    }
}

impl<'a> Read for Body<'a> {
    fn read(&mut self, buf: &mut [u8]) -> stdio::Result<usize> {
        self.reader.read(buf)
    }
}

struct BodyBuilder<'a, R: Read> {
    reference: RefMut<'a, R>,
    content_length: Option<u64>,
}

impl<'a, R: Read> BodyBuilder<'a, R> {
    fn new(reference: RefMut<'a, R>) -> Self {
        BodyBuilder {
            reference,
            content_length: None,
        }
    }

    fn content_length(mut self, length: u64) -> Self {
        self.content_length = Some(length);
        self
    }

    fn build(self) -> Body<'a> {
        let reader: Box<dyn Read> = if let Some(n) = self.content_length {
            Box::new(LimitedReader::new(self.reference, n))
        } else {
            Box::new(DelimitedReader::new(self.reference, NULL))
        };

        Body { reader }
    }
}

pub struct Frame<'a> {
    pub command: Command,
    pub header: Header,
    pub body: Body<'a>,
}

impl<'a> Frame<'a> {
    pub fn new(command: Command, body: Body<'a>) -> Self {
        Frame {
            command,
            header: Header::new(),
            body,
        }
    }

    fn with_header(command: Command, header: Header, body: Body<'a>) -> Self {
        Frame {
            command,
            header,
            body,
        }
    }

    pub fn write_to<W: Write>(&mut self, w: W) -> stdio::Result<u64> {
        let mut bw = BufWriter::new(w);
        let mut bytes_written: u64 = 0;
        bytes_written += bw.write(self.command.to_string().as_bytes())? as u64;
        bytes_written += bw.write(&[EOL])? as u64;
        bytes_written += self.header.write_to(&mut bw)?;
        bytes_written += bw.write(&[EOL])? as u64;
        bytes_written += stdio::copy(&mut self.body, &mut bw)?;
        bytes_written += bw.write(&[NULL])? as u64;

        bw.flush().and(Ok(bytes_written))
    }

    fn read_command<R: Read>(r: &mut R) -> Result<Command, ReadError> {
        let command_reader = r.take(MAX_COMMAND_SIZE);
        let mut buf_reader = BufReader::new(command_reader);
        let mut command_buffer: Vec<u8> = Vec::new();
        let cmd_bytes_read = buf_reader.read_until(EOL, &mut command_buffer)?;

        if cmd_bytes_read < 1 {
            return Err("empty command".into());
        }
        let raw_string_command = str::from_utf8(&command_buffer)?;
        let clean_string_command = raw_string_command.trim();

        if clean_string_command.is_empty() {
            return Err("empty command".into());
        }
        Command::from_str(clean_string_command)
    }
}

impl<'a> Drop for Frame<'a> {
    fn drop(&mut self) {
        self.body.close().unwrap();
    }
}

pub struct FrameReader<R: Read> {
    reader: RefCell<R>,
}

impl<R: Read> FrameReader<R> {
    pub fn new(reader: R) -> FrameReader<R> {
        FrameReader {
            reader: RefCell::new(reader),
        }
    }

    pub fn read_frame(&'static self) -> Result<Frame, ReadError> {
        let mut reader = self.reader.try_borrow_mut()?;
        let command = Frame::read_command(reader.deref_mut())?;
        let header = Header::read_from(reader.deref_mut())?;

        let clen = header
            .get_field("content-length")
            .map(|v| v.first())
            .unwrap_or(None);

        let mut body = BodyBuilder::new(reader);

        body = if let Some(n) = clen {
            body.content_length(n.parse::<u64>()?)
        } else {
            body
        };

        let frame = Frame::with_header(command, header, body.build());

        Ok(frame)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn read_header() {
        let input = b"Content-Type: application/json\r\nContent-Length: 30\r\nName: Joshua\r\n";
        let mut reader = Cursor::new(&input[..]);
        let header = Header::read_from(&mut reader).unwrap();

        let mut target = Header::new();
        target.add_field("Content-Type", "application/json");
        target.add_field("Content-Length", "30");
        target.add_field("Name", "Joshua");
        assert_eq!(target, header);
    }

    #[test]
    fn write_header() {
        let target = "Content-Length: 30\nContent-Type: application/json\n";

        let mut header = Header::new();
        header.add_field("Content-Type", "application/json");
        header.add_field("Content-Length", "30");

        let mut buffer: Vec<u8> = Vec::new();
        header.write_to(&mut buffer).unwrap();
        let data = str::from_utf8(&buffer).unwrap();
        assert_eq!(target, data)
    }

    #[test]
    fn write_header_encode_colon() {
        let target = "Content-Length: 30\nContent-Type: vnd\\capplication/json\n";

        let mut header = Header::new();
        header.add_field("Content-Type", "vnd:application/json");
        header.add_field("Content-Length", "30");

        let mut buffer: Vec<u8> = Vec::new();
        header.write_to(&mut buffer).unwrap();
        let data = str::from_utf8(&buffer).unwrap();
        assert_eq!(target, data)
    }

    #[test]
    fn write_frame() {
        let target = "CONNECT\nContent-Length: 30\nContent-Type: application/json\n\n\0";
        let input = stdio::empty();
        let ref_input = RefCell::new(input);
        let mut body = BodyBuilder::new(ref_input.borrow_mut());
        body = body.content_length(30);
        let mut frame = Frame::new(Command::Connect, body.build());
        frame.header.add_field("Content-Type", "application/json");
        frame.header.add_field("Content-Length", "30");

        let mut buffer: Vec<u8> = Vec::new();
        frame.write_to(&mut buffer).unwrap();
        let data = str::from_utf8(&buffer).unwrap();
        assert_eq!(target, data)
    }

    #[test]
    fn write_frame_with_body() {
        let target = "CONNECT\nContent-Length: 30\nContent-Type: application/json\n\n{\"name\":\"Joshua\"}\0";
        let input = Cursor::new(b"{\"name\":\"Joshua\"}");
        let ref_input = RefCell::new(input);
        let mut body = BodyBuilder::new(ref_input.borrow_mut());
        body = body.content_length(30);
        let mut frame = Frame::new(Command::Connect, body.build());
        frame.header.add_field("Content-Type", "application/json");
        frame.header.add_field("Content-Length", "30");

        let mut buffer: Vec<u8> = Vec::new();
        frame.write_to(&mut buffer).unwrap();
        let data = str::from_utf8(&buffer).unwrap();
        assert_eq!(target, data)
    }

    /*

    #[test]
    fn read_frame_with_body_with_content_length() {
        let input = b"CONNECT\nContent-Length: 17\nContent-Type: application/json\n\n{\"name\":\"Joshua\"}\0";
        let mut reader = Cursor::new(&input[..]);
        let mut frame = Frame::read_from(&mut reader).unwrap();

        let mut target_header = Header::new();
        target_header.add_field("Content-Type", "application/json");
        target_header.add_field("Content-Length", "17");

        let target_body = b"{\"name\":\"Joshua\"}".to_vec();

        let mut buffer: Vec<u8> = Vec::new();
        Read::read_to_end(&mut frame.body, &mut buffer).unwrap();

        assert_eq!(Command::Connect, frame.command);
        assert_eq!(target_header, frame.header);
        assert_eq!(target_body, buffer);
    }

    #[test]
    fn read_frame_with_body_without_content_length() {
        let input = b"CONNECT\nContent-Type: application/json\n\n{\"name\":\"Joshua\"}\0(Should not read this)";
        let mut reader = Cursor::new(&input[..]);
        let mut frame = Frame::read_from(&mut reader).unwrap();

        let mut target_header = Header::new();
        target_header.add_field("Content-Type", "application/json");

        let target_body = b"{\"name\":\"Joshua\"}".to_vec();

        let mut buffer: Vec<u8> = Vec::new();
        Read::read_to_end(&mut frame.body, &mut buffer).unwrap();

        assert_eq!(Command::Connect, frame.command);
        assert_eq!(target_header, frame.header);
        assert_eq!(target_body, buffer);
    }

     */
}
