use std::collections::HashMap;
use std::fmt;
use std::io::Write;
use std::io;
use std::io::BufWriter;
use std::str;

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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Command::*;

        let v = match self {
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

        write!(f, "{}", v)
    }
}

fn encode_str(input: &str) -> String {
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

pub struct Header {
    map: HashMap<String, Vec<String>>,
}

impl Header {
    pub fn new() -> Header {
        Header {
            map: HashMap::new(),
        }
    }

    pub fn add(&mut self, key: &str, value: &str) {
        self.map
            .entry(encode_str(key))
            .or_insert_with(|| Vec::with_capacity(1))
            .push(encode_str(value));
    }

    pub fn set(&mut self, key: &str, values: Vec<&str>) {
        let mut c = Vec::with_capacity(values.len());

        for v in values {
            c.push(encode_str(v));
        }

        self.map
            .insert(encode_str(key), c);
    }

    pub fn remove(&mut self, key: &str) {
        self.map.remove(encode_str(key).as_str());
    }

    pub fn write_to<T: Write>(&self, w: &mut T) -> io::Result<usize> {
        let mut bw = BufWriter::new(w);
        let mut bytes_written = 0;

        for (k, v) in self.map.iter() {
            let field_str = format!("{}: {}\n", k, v.join(","));
            let size = bw.write(field_str.as_bytes())?;
            bytes_written += size;
        }
        bw.flush().and(Ok(bytes_written))
    }
}

#[test]
fn write_header() {
    let target = "Content-Type: application/json\nContent-Length: 30\n";

    let mut header = Header::new();
    header.add("Content-Type", "application/json");
    header.add("Content-Length", "30");

    let mut buffer: Vec<u8> = Vec::new();
    header.write_to(&mut buffer).unwrap();
    let data = str::from_utf8(&buffer).unwrap();
    assert_eq!(target, data)
}

#[test]
fn encode_backslash() {
    let input = "Hello\\World";
    let target = "Hello\\\\World";
    assert!(target == encode_str(input))
}

#[test]
fn encode_carriage_return() {
    let input = "Hello\rWorld";
    let target = "Hello\\rWorld";
    assert!(target == encode_str(input))
}

#[test]
fn encode_newline() {
    let input = "Hello\nWorld";
    let target = "Hello\\nWorld";
    assert!(target == encode_str(input))
}

#[test]
fn encode_semicolon() {
    let input = "Hello:World";
    let target = "Hello\\cWorld";
    assert!(target == encode_str(input))
}
