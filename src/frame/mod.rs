mod error;
mod io;
mod string;

use crate::frame::io::{BiReader, LimitedReader};
use error::ReadError;
use io::DelimitedReader;
use std::borrow::BorrowMut;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::io as stdio;
use std::io::{BufRead, BufReader, BufWriter};
use std::io::{Read, Write};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::str;
use std::str::FromStr;

type LockFlag = isize;
const UNUSED: LockFlag = 0;

const MAX_COMMAND_SIZE: u64 = 1024;
const MAX_HEADER_SIZE: u64 = 1024 * 1000;
const NULL: u8 = b'\0';
const EOL: u8 = b'\n';

#[derive(Debug, Clone)]
pub struct LatchError;

impl Display for LatchError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "latch already locked")
    }
}

impl Error for LatchError {}

struct Guard<'a> {
    value: &'a Cell<LockFlag>,
}

impl<'a> Guard<'a> {
    fn new(value: &'a Cell<LockFlag>) -> Option<Guard<'a>> {
        match value.get() {
            UNUSED => {
                value.set(UNUSED + 1);
                Some(Guard { value })
            }
            _ => None,
        }
    }
}

impl<'a> Drop for Guard<'a> {
    fn drop(&mut self) {
        let value = self.value.get();
        self.value.set(value - 1)
    }
}

struct Gate {
    latch: Cell<LockFlag>,
}

impl Gate {
    fn new() -> Self {
        Self {
            latch: Cell::new(UNUSED),
        }
    }

    fn latch(&self) -> Guard<'_> {
        Guard::new(&self.latch).expect("gate is latched")
    }

    fn try_latch(&self) -> Result<Guard<'_>, LatchError> {
        match Guard::new(&self.latch) {
            Some(g) => Ok(g),
            None => Err(LatchError),
        }
    }
}

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
pub struct Header(BTreeMap<String, Vec<String>>);

impl Deref for Header {
    type Target = BTreeMap<String, Vec<String>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Header {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.borrow_mut()
    }
}

impl Header {
    pub fn new() -> Self {
        Header(BTreeMap::new())
    }

    pub fn push<T: Into<String>>(&mut self, key: T, value: String) {
        self.entry(key.into())
            .or_insert_with(|| Vec::with_capacity(1))
            .push(value)
    }

    pub fn write_to<W: Write>(&self, mut w: W) -> stdio::Result<u64> {
        let mut bytes_written: u64 = 0;

        for (k, v) in self.0.iter() {
            let field_str = format!("{}: {}\n", string::encode(k), string::encode(&v.join(",")));
            let size = w.write(field_str.as_bytes())?;
            bytes_written += size as u64;
        }
        Ok(bytes_written)
    }

    fn read_from<R: Read>(reader: &mut BufReader<R>) -> Result<Self, ReadError> {
        let mut limited_reader = reader.take(MAX_HEADER_SIZE);
        let mut header = Self::new();

        loop {
            let mut buffer: Vec<u8> = Vec::new();
            let bytes_read = limited_reader.read_until(EOL, &mut buffer)?;

            if bytes_read < 1 {
                break;
            }
            let line = str::from_utf8(&buffer)?;
            let clean_line = line.trim_end_matches('\r').trim_end_matches('\n');
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
            let clean_field_value = field_value
                .trim_start()
                .trim_end_matches('\n')
                .trim_end_matches('\r')
                .to_owned();

            if clean_field_name.is_empty() {
                return Err("empty header field name".into());
            }
            header.push(clean_field_name, clean_field_value);
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

struct BodyBuilder<R: Read> {
    reference: Rc<RefCell<R>>,
    content_length: Option<u64>,
}

impl<'a, R: Read + 'a> BodyBuilder<R> {
    fn new(reference: Rc<RefCell<R>>) -> Self {
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
            let limited_reader = LimitedReader::new(self.reference.clone(), n);
            let delimited_reader = DelimitedReader::new(self.reference, NULL);
            Box::new(BiReader::new(limited_reader, delimited_reader))
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
    _guard: Guard<'a>,
}

impl<'a> Frame<'a> {
    fn new(command: Command, body: Body<'a>, guard: Guard<'a>) -> Self {
        Frame {
            command,
            header: Header::new(),
            body,
            _guard: guard,
        }
    }

    fn with_header(command: Command, header: Header, body: Body<'a>, guard: Guard<'a>) -> Self {
        Frame {
            command,
            header,
            body,
            _guard: guard,
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

    fn read_command<R: Read>(r: &mut BufReader<R>) -> Result<Command, ReadError> {
        let mut command_reader = r.take(MAX_COMMAND_SIZE);
        let mut command_buffer: Vec<u8> = Vec::new();
        let cmd_bytes_read = command_reader.read_until(EOL, &mut command_buffer)?;

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
    reader: Rc<RefCell<BufReader<R>>>,
    gate: Gate,
}

impl<R: Read> FrameReader<R> {
    pub fn new(reader: R) -> FrameReader<R> {
        FrameReader {
            reader: Rc::new(RefCell::new(BufReader::new(reader))),
            gate: Gate::new(),
        }
    }

    pub fn read_frame(&'static self) -> Result<Frame, ReadError> {
        let guard = self.gate.try_latch()?;
        let mut reader = self.reader.try_borrow_mut()?;
        let command = Frame::read_command(reader.deref_mut())?;
        let header = Header::read_from(reader.deref_mut())?;

        let clen = header
            .get("content-length")
            .map(|v| v.first())
            .unwrap_or(None);

        let mut body = BodyBuilder::new(self.reader.clone());

        body = if let Some(n) = clen {
            body.content_length(n.parse::<u64>()?)
        } else {
            body
        };

        let frame = Frame::with_header(command, header, body.build(), guard);

        Ok(frame)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Cursor;

    #[test]
    #[should_panic]
    fn gate() {
        let gate = Gate::new();
        let _guard = gate.latch();
        gate.latch();
    }

    #[test]
    fn gate_proper() {
        let gate = Gate::new();
        let guard = gate.latch();
        drop(guard);
        gate.latch();
    }

    #[test]
    fn read_header() {
        let input = b"Content-Type: application/json\r\nContent-Length: 30\r\nName: Joshua\r\n";
        let reader = Cursor::new(&input[..]);
        let mut buf_reader = BufReader::new(reader);
        let header = Header::read_from(&mut buf_reader).unwrap();

        let mut target = Header::new();
        target.push("content-type", "application/json".to_owned());
        target.push("content-length", "30".to_owned());
        target.push("name", "Joshua".to_owned());
        assert_eq!(target, header);
    }

    #[test]
    fn write_header() {
        let target = "Content-Length: 30\nContent-Type: application/json\n";

        let mut header = Header::new();
        header.push("Content-Type", "application/json".to_owned());
        header.push("Content-Length", "30".to_owned());

        let mut buffer: Vec<u8> = Vec::new();
        header.write_to(&mut buffer).unwrap();
        let data = str::from_utf8(&buffer).unwrap();
        assert_eq!(target, data)
    }

    #[test]
    fn write_header_encode_colon() {
        let target = "Content-Length: 30\nContent-Type: vnd\\capplication/json\n";

        let mut header = Header::new();
        header.push("Content-Type", "vnd:application/json".to_owned());
        header.push("Content-Length", "30".to_owned());

        let mut buffer: Vec<u8> = Vec::new();
        header.write_to(&mut buffer).unwrap();
        let data = str::from_utf8(&buffer).unwrap();
        assert_eq!(target, data)
    }

    #[test]
    fn write_frame() {
        let target = "CONNECT\nContent-Length: 30\nContent-Type: application/json\n\n\0";
        let input = stdio::empty();
        let ref_input = Rc::new(RefCell::new(input));
        let mut body = BodyBuilder::new(ref_input);
        body = body.content_length(30);

        let gate = Gate::new();
        let guard = gate.latch();
        let mut frame = Frame::new(Command::Connect, body.build(), guard);
        frame
            .header
            .push("Content-Type", "application/json".to_owned());
        frame.header.push("Content-Length", "30".to_owned());

        let mut buffer: Vec<u8> = Vec::new();
        frame.write_to(&mut buffer).unwrap();
        let data = str::from_utf8(&buffer).unwrap();
        assert_eq!(target, data)
    }

    #[test]
    fn write_frame_with_body() {
        let target = "CONNECT\nContent-Length: 30\nContent-Type: application/json\n\n{\"name\":\"Joshua\"}\0";
        let input = Cursor::new(b"{\"name\":\"Joshua\"}");
        let ref_input = Rc::new(RefCell::new(input));
        let mut body = BodyBuilder::new(ref_input);
        body = body.content_length(30);

        let gate = Gate::new();
        let guard = gate.latch();
        let mut frame = Frame::new(Command::Connect, body.build(), guard);
        frame
            .header
            .push("Content-Type", "application/json".to_owned());
        frame.header.push("Content-Length", "30".to_owned());

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
