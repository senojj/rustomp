use std::collections::BTreeMap;
use std::io::{Write, Read};
use std::io;
use std::io::BufWriter;
use std::str;
use std::string;
use std::convert::TryInto;

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

impl string::ToString for Command {
    fn to_string(&self) -> string::String {
        use self::Command::*;

        match self {
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
        }.to_string()
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
    map: BTreeMap<String, Vec<String>>,
}

impl Header {
    pub fn new() -> Self {
        Header {
            map: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, key: &str, value: &str) {
        self.map
            .entry(key.to_string())
            .or_insert_with(|| Vec::with_capacity(1))
            .push(value.to_string());
    }

    pub fn set(&mut self, key: &str, values: Vec<&str>) {
        let mut c = Vec::with_capacity(values.len());

        for v in values {
            c.push(v.to_string());
        }

        self.map
            .insert(key.to_string(), c);
    }

    pub fn remove(&mut self, key: &str) {
        self.map.remove(key);
    }

    pub fn write_to<W: Write>(&self, w: &mut W) -> io::Result<u64> {
        let mut bw = BufWriter::new(w);
        let mut bytes_written: u64 = 0;

        for (k, v) in self.map.iter() {
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
        bytes_written += self.header.write_to(&mut bw)? as u64;
        bytes_written += bw.write(b"\n")? as u64;
        bytes_written += io::copy(&mut self.body, &mut bw)?;

        bw.flush().and(Ok(bytes_written))
    }
}

#[test]
fn write_frame() {
    let target = "CONNECT\nContent-Length: 30\nContent-Type: application/json\n\n";

    let mut frame = Frame::new(Command::Connect, io::empty());
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
