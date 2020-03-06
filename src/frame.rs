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

fn encode(input: &String) -> String {
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

    pub fn write_to<T: Write>(&self, w: &mut T) -> io::Result<usize> {
        let mut bw = BufWriter::new(w);
        let mut bytes_written = 0;

        for (k, v) in self.map.iter() {
            let field_str = format!("{}: {}\n", encode(k), encode(&v.join(",")));
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
fn write_header_colon() {
    let target = "Content-Type: vnd\\capplication/json\nContent-Length: 30\n";

    let mut header = Header::new();
    header.add("Content-Type", "vnd:application/json");
    header.add("Content-Length", "30");

    let mut buffer: Vec<u8> = Vec::new();
    header.write_to(&mut buffer).unwrap();
    let data = str::from_utf8(&buffer).unwrap();
    assert_eq!(target, data)
}

#[test]
fn encode_backslash() {
    let input = "Hello\\World".to_string();
    let target = "Hello\\\\World".to_string();
    assert_eq!(target, encode(&input))
}

#[test]
fn encode_carriage_return() {
    let input = "Hello\rWorld".to_string();
    let target = "Hello\\rWorld".to_string();
    assert_eq!(target, encode(&input))
}

#[test]
fn encode_newline() {
    let input = "Hello\nWorld".to_string();
    let target = "Hello\\nWorld".to_string();
    assert_eq!(target, encode(&input))
}

#[test]
fn encode_semicolon() {
    let input = "Hello:World".to_string();
    let target = "Hello\\cWorld".to_string();
    assert_eq!(target, encode(&input))
}
