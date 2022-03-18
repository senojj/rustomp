const NULL: char = '\0';
const BACKSLASH: char = '\\';
const NEWLINE: char = '\n';
const CARRIAGE_RETURN: char = '\r';
const COLON: char = ':';

pub fn encode(input: &str) -> String {
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

pub fn decode(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut last_char = NULL;

    for c in input.chars() {
        match c {
            'c' if last_char == BACKSLASH => output.push(':'),
            'n' if last_char == BACKSLASH => output.push('\n'),
            'r' if last_char == BACKSLASH => output.push('\r'),
            BACKSLASH if last_char == BACKSLASH => output.push('\\'),
            BACKSLASH => (),
            a => output.push(a),
        }
        last_char = if last_char == BACKSLASH && c == BACKSLASH {
            NULL
        } else {
            c
        }
    }
    output
}

#[cfg(test)]
mod test {
    use super::*;

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
}
