use std::fmt;
use std::str::FromStr;

pub enum ReadError {
    InvalidCommand(String)
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
            _ => Err(ReadError::InvalidCommand(s.into())),
        }
    }
}

enum ResponseFrame<const S: usize> {
    Command(Command),
    Header(String, String),
    Body([u8; S], usize)
}