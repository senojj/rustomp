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

    pub fn read_from<R: Read>(r: &mut R) -> Result<Self, ReadError> {
        let mut limited_reader = r.take(1024 * 1000);
        let mut header = Self::new();

        loop {
            let mut delimited_reader = io::DelimitedReader::new(&mut limited_reader, b'\n');
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
            let field_name = string::decode(parts[0]);
            let field_value = string::decode(parts[1]);

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

        let mut frame = Frame::new(Command::Connect, stdio::empty());
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