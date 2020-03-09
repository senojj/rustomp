use std::collections::BTreeMap;
use std::io::{Write, Read, Cursor};
use std::io;
use std::io::BufWriter;
use std::str;
use std::string;
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

fn encode(input: &str) -> String {
    let mut output = String::with_capacity(input.len());

    for c in input.chars() {
        match c {
            '\\' => output.push_str("\\\\"),
            '\r' => output.push_str("\\r"),
            '\n' => output.push_str("\\n"),
            ':' => output.push_str("\\c"),
            a => output.push(a),
        }
    }
    output
}

#[derive(Default)]
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
