mod error;
mod io;
mod string;

use error::ReadError;
use std::collections::BTreeMap;
use std::io::{Write, Read};
use std::io as stdio;
use std::io::BufWriter;
use std::str;
use std::fmt;
use io::DelimitedReader;
use std::str::FromStr;

const MAX_HEADER_SIZE: u64 = 1024 * 1000;

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

impl str::FromStr for Command {
    type Err = String;

    fn from_str(s: &str) -> Result<Command, String> {
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
            _ => Err(String::from("invalid command")),
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

    pub fn get<T: Into<String>>(&self, key: T) -> Option<&Vec<String>> {
        self.fields.get(&key.into())
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

    pub fn write_to<W: Write>(&self, w: &mut W) -> stdio::Result<u64> {
        let mut bw = BufWriter::new(w);
        let mut bytes_written: u64 = 0;

        for (k, v) in self.fields.iter() {
            let field_str = format!("{}: {}\n", string::encode(k), string::encode(&v.join(",")));
            let size = bw.write(field_str.as_bytes())?;
            bytes_written += size as u64;
        }
        bw.flush().and(Ok(bytes_written))
    }

    fn read_from<R: Read>(r: &mut R) -> Result<Self, ReadError> {
        let mut limited_reader = r.take(MAX_HEADER_SIZE);
        let mut header = Self::new();

        loop {
            let mut delimited_reader = DelimitedReader::new(&mut limited_reader, b'\n');
            let mut buffer: Vec<u8> = Vec::new();
            let bytes_read = Read::read_to_end(&mut delimited_reader, &mut buffer)?;

            if bytes_read < 1 {
                break;
            }
            let line = str::from_utf8(&buffer)?;
            let clean_line = line.trim_end_matches('\r');
            let parts: Vec<&str> = clean_line.split(':').collect();

            if parts.len() < 2 {
                return Err(ReadError::Format(
                    format!("invalid number of header field parts. Expected 2, got {}", parts.len())
                ));
            }
            let field_name = string::decode(parts[0]);
            let field_value = string::decode(parts[1]);

            let clean_field_name = field_name.trim();
            let clean_field_value = field_value.trim_start();

            if clean_field_name.is_empty() {
                return Err(ReadError::Format(String::from("empty header field name")));
            }
            header.add(clean_field_name, clean_field_value);
        }
        Ok(header)
    }
}

pub struct Body<'a, R: Read> {
    reader: &'a mut R,
    content_length: u64,
    content_read: u64,
}

impl<'a, R: Read> Body<'a, R> {
    fn new(reader: &'a mut R) -> Self {
        Body {
            reader,
            content_length: 0,
            content_read: 0,
        }
    }

    fn new_with_length(reader: &'a mut R, content_length: u64) -> Self {
        Body {
            reader,
            content_length,
            content_read: 0,
        }
    }

    pub fn close(&mut self) -> stdio::Result<()> {
        stdio::copy(&mut self.reader, &mut stdio::sink()).map(|_| ())
    }
}

impl<'a, R: Read> Read for Body<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> stdio::Result<usize> {
        let bytes_read = if self.content_length > 0 {
            let mut reader_a = self.reader.take(self.content_length - self.content_read);
            let content_read = reader_a.read(buf)?;
            self.content_read += content_read as u64;
            content_read
        } else {
            let mut reader_b  = io::DelimitedReader::new(&mut self.reader, b'\0');
            reader_b.read(buf)?
        };
        Ok(bytes_read)
    }
}

pub struct Frame<'a, R: Read> {
    pub command: Command,
    pub header: Header,
    pub body: Body<'a, R>,
}

impl<'a, R: Read> Frame<'a, R> {
    pub fn new(command: Command, body: &'a mut R) -> Self {
        Frame {
            command,
            header: Header::new(),
            body: Body::new(body),
        }
    }

    fn new_with_header(command: Command, header: Header, body: &'a mut R) -> Self {
        let value = header.get("Content-Length").map(|v| v.first()).unwrap_or(None);

        let content = match value {
            Some(n) => Body::new_with_length(body, n.parse::<u64>().unwrap()),
            None => Body::new(body),
        };

        Frame {
            command,
            header,
            body: content,
        }
    }

    pub fn write_to<W: Write>(&mut self, w: &mut W) -> stdio::Result<u64> {
        let mut bw = BufWriter::new(w);
        let mut bytes_written: u64 = 0;
        bytes_written += bw.write(self.command.to_string().as_bytes())? as u64;
        bytes_written += bw.write(b"\n")? as u64;
        bytes_written += self.header.write_to(&mut bw)?;
        bytes_written += bw.write(b"\n")? as u64;
        bytes_written += stdio::copy(&mut self.body, &mut bw)?;
        bytes_written += bw.write(b";")? as u64;

        bw.flush().and(Ok(bytes_written))
    }

    fn read_command(r: &mut R) -> Result<Command, ReadError> {
        let mut command_reader = r.take(1024);
        let mut command_line_reader = io::DelimitedReader::new(&mut command_reader, b'\n');
        let mut command_buffer: Vec<u8> = Vec::new();
        let cmd_bytes_read = Read::read_to_end(&mut command_line_reader, &mut command_buffer)?;

        if cmd_bytes_read < 1 {
            return Err(ReadError::Format(String::from("empty command")));
        }
        let raw_string_command = str::from_utf8(&command_buffer)?;
        let clean_string_command = raw_string_command.trim();

        if clean_string_command.is_empty() {
            return Err(ReadError::Format(String::from("empty command")));
        }
        Command::from_str(clean_string_command).map_err(|e| ReadError::Format(e))
    }

    pub fn read_from(mut r: &'a mut R) -> Result<Self, ReadError> {
        let mut null_terminated_reader = io::DelimitedReader::new(&mut r, b'\0');
        let command = Frame::read_command(&mut null_terminated_reader)?;
        let header = Header::read_from(&mut null_terminated_reader)?;
        let frame = Frame::new_with_header(command, header, r);
        Ok(frame)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Cursor;

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
        let mut input = stdio::empty();
        let mut frame = Frame::new(Command::Connect, &mut input);
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
        let mut input = Cursor::new(b"{\"name\":\"Joshua\"}");
        let mut frame = Frame::new(Command::Connect, &mut input);
        frame.header.add("Content-Type", "application/json");
        frame.header.add("Content-Length", "30");

        let mut buffer: Vec<u8> = Vec::new();
        frame.write_to(&mut buffer).unwrap();
        let data = str::from_utf8(&buffer).unwrap();
        assert_eq!(target, data)
    }

    #[test]
    fn read_header() {
        let input = b"Content-Type: application/json\r\nContent-Length: 30\r\nName: Joshua\r\n";
        let mut reader: Cursor<&[u8]> = Cursor::new(&input[..]);
        let header = Header::read_from(&mut reader).unwrap();

        let mut target = Header::new();
        target.add("Content-Type", "application/json");
        target.add("Content-Length", "30");
        target.add("Name", "Joshua");
        assert_eq!(target, header);
    }
}