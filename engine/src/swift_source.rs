#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SwiftLexed {
    pub code: String,
    pub string_literals: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SwiftIdentifier {
    pub text: String,
    pub line: usize,
    pub column: usize,
    pub followed_by_call: bool,
}

/// Lex Swift far enough for conservative source-policy evidence. Comments and
/// string literal bodies become spaces while newlines and byte positions stay
/// stable; literal values are returned separately for gates where the value
/// itself is evidence (for example a runtime URL).
pub(crate) fn lex(source: &str) -> SwiftLexed {
    #[derive(Clone, Copy)]
    enum State {
        Code,
        LineComment,
        BlockComment(u32),
        String {
            hashes: usize,
            multiline: bool,
            escaped: bool,
        },
    }

    let bytes = source.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut literals = Vec::new();
    let mut literal = Vec::new();
    let mut state = State::Code;
    let mut index = 0;
    while index < bytes.len() {
        let remaining = &bytes[index..];
        match state {
            State::Code if remaining.starts_with(b"//") => {
                blank(&mut output, &bytes[index..index + 2]);
                state = State::LineComment;
                index += 2;
            }
            State::Code if remaining.starts_with(b"/*") => {
                blank(&mut output, &bytes[index..index + 2]);
                state = State::BlockComment(1);
                index += 2;
            }
            State::Code => {
                let hashes = bytes[index..]
                    .iter()
                    .take_while(|byte| **byte == b'#')
                    .count();
                let quote = index + hashes;
                let multiline = bytes
                    .get(quote..quote.saturating_add(3))
                    .is_some_and(|value| value == b"\"\"\"");
                let normal = bytes.get(quote) == Some(&b'"');
                if normal {
                    let width = hashes + if multiline { 3 } else { 1 };
                    blank(&mut output, &bytes[index..index + width]);
                    literal.clear();
                    state = State::String {
                        hashes,
                        multiline,
                        escaped: false,
                    };
                    index += width;
                } else {
                    output.push(bytes[index]);
                    index += 1;
                }
            }
            State::LineComment if bytes[index] == b'\n' => {
                output.push(b'\n');
                state = State::Code;
                index += 1;
            }
            State::LineComment => {
                output.push(b' ');
                index += 1;
            }
            State::BlockComment(depth) if remaining.starts_with(b"/*") => {
                blank(&mut output, &bytes[index..index + 2]);
                state = State::BlockComment(depth.saturating_add(1));
                index += 2;
            }
            State::BlockComment(depth) if remaining.starts_with(b"*/") => {
                blank(&mut output, &bytes[index..index + 2]);
                state = if depth == 1 {
                    State::Code
                } else {
                    State::BlockComment(depth - 1)
                };
                index += 2;
            }
            State::BlockComment(depth) => {
                output.push(if bytes[index] == b'\n' { b'\n' } else { b' ' });
                state = State::BlockComment(depth);
                index += 1;
            }
            State::String {
                hashes,
                multiline,
                escaped,
            } => {
                let quote_count = if multiline { 3 } else { 1 };
                let close_width = quote_count + hashes;
                let closes = bytes
                    .get(index..index.saturating_add(close_width))
                    .is_some_and(|candidate| {
                        candidate[..quote_count].iter().all(|byte| *byte == b'"')
                            && candidate[quote_count..].iter().all(|byte| *byte == b'#')
                    });
                if closes && (!escaped || hashes > 0) {
                    blank(&mut output, &bytes[index..index + close_width]);
                    literals.push(String::from_utf8_lossy(&literal).into_owned());
                    literal.clear();
                    state = State::Code;
                    index += close_width;
                } else {
                    let byte = bytes[index];
                    output.push(if byte == b'\n' { b'\n' } else { b' ' });
                    literal.push(byte);
                    let next_escaped = hashes == 0 && byte == b'\\' && !escaped;
                    state = State::String {
                        hashes,
                        multiline,
                        escaped: next_escaped,
                    };
                    index += 1;
                }
            }
        }
    }
    if !literal.is_empty() {
        literals.push(String::from_utf8_lossy(&literal).into_owned());
    }
    SwiftLexed {
        code: String::from_utf8(output).unwrap_or_else(|_| {
            source
                .chars()
                .map(|character| if character == '\n' { '\n' } else { ' ' })
                .collect()
        }),
        string_literals: literals,
    }
}

pub(crate) fn identifiers(code: &str) -> Vec<SwiftIdentifier> {
    let bytes = code.as_bytes();
    let mut found = Vec::new();
    let mut index = 0;
    let mut line = 1_usize;
    let mut column = 1_usize;
    while index < bytes.len() {
        if bytes[index] == b'\n' {
            line += 1;
            column = 1;
            index += 1;
            continue;
        }
        if is_identifier_start(bytes[index]) {
            let start = index;
            let start_column = column;
            index += 1;
            column += 1;
            while index < bytes.len() && is_identifier_continue(bytes[index]) {
                index += 1;
                column += 1;
            }
            let mut lookahead = index;
            while bytes.get(lookahead).is_some_and(u8::is_ascii_whitespace) {
                lookahead += 1;
            }
            found.push(SwiftIdentifier {
                text: code[start..index].into(),
                line,
                column: start_column,
                followed_by_call: bytes.get(lookahead) == Some(&b'('),
            });
        } else {
            index += 1;
            column += 1;
        }
    }
    found
}

fn blank(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend(
        bytes
            .iter()
            .map(|byte| if *byte == b'\n' { b'\n' } else { b' ' }),
    );
}

fn is_identifier_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_comments_literals_and_raw_multiline_strings_but_keeps_identifiers() {
        let source = r###"
// URLSession.shared.data()
let prose = #"NWConnection and AVCaptureSession"#
let more = """SFSpeechRecognizer
ARSession"""
let session = ARSession()
"###;
        let lexed = lex(source);
        let names = identifiers(&lexed.code)
            .into_iter()
            .map(|token| token.text)
            .collect::<Vec<_>>();
        assert!(names.contains(&"ARSession".into()));
        assert!(!names.contains(&"URLSession".into()));
        assert!(!names.contains(&"NWConnection".into()));
        assert_eq!(lexed.string_literals.len(), 2);
    }

    #[test]
    fn uses_identifier_boundaries_and_call_shape() {
        let tokens = identifiers("let eyeSocket = 1\nsocket()\nlet violetCurls = true\n");
        assert!(tokens
            .iter()
            .any(|token| token.text == "socket" && token.followed_by_call));
        assert!(tokens.iter().any(|token| token.text == "eyeSocket"));
        assert!(!tokens.iter().any(|token| token.text == "curl"));
    }
}
