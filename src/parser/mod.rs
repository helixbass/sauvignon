use squalid::_d;

use crate::Request;

const UnicodeBOM: char = '\u{feff}';

pub fn lex(request: &[char]) -> Vec<Token> {
    let mut ret = vec![];
    let mut current_index = 0;
    while current_index < request.len() {
        match request[current_index] {
            '!' => {
                ret.push(Token::ExclamationPoint);
            }
            '$' => {
                ret.push(Token::DollarSign);
            }
            '&' => {
                ret.push(Token::Ampersand);
            }
            '(' => {
                ret.push(Token::LeftParen);
            }
            ')' => {
                ret.push(Token::RightParen);
            }
            '.' => {
                assert!(request.get(current_index + 1) == '.');
                current_index += 1;
                assert!(request.get(current_index + 1) == '.');
                current_index += 1;
                ret.push(Token::DotDotDot);
            }
            ':' => {
                ret.push(Token::Colon);
            }
            '=' => {
                ret.push(Token::Equals);
            }
            '@' => {
                ret.push(Token::AtSymbol);
            }
            '[' => {
                ret.push(Token::LeftSquareBracket);
            }
            ']' => {
                ret.push(Token::RightSquareBracket);
            }
            '{' => {
                ret.push(Token::LeftCurlyBracket);
            }
            '|' => {
                ret.push(Token::Pipe);
            }
            '}' => {
                ret.push(Token::RightCurlyBracket);
            }
            'A'..='Z' | 'a'..='z' | '_' => {
                let initial_index = current_index;
                while matches!(
                    request.get(current_index + 1),
                    Some(ch) if matches!(
                        ch,
                        'A'..='Z' | 'a'..='z' | '_' | '0'..='9'
                    )
                ) {
                    current_index += 1;
                }
                ret.push(Token::Name(
                    request[initial_index..=current_index].iter().collect(),
                ));
            }
            '"' => {
                let initial_index = current_index;
                match request.get(current_index + 1) {
                    Some('"') => {
                        current_index += 1;
                        match request.get(current_index + 1) {
                            Some('"') => {
                                unimplemented!()
                            }
                            _ => {
                                ret.push(Token::String("".to_owned()));
                            }
                        }
                    }
                    _ => {
                        let mut resolved_chars: Vec<char> = _d();
                        loop {
                            match request.get(current_index + 1) {
                                None => panic!("expected closing double-quote"),
                                Some(ch) => {
                                    match ch {
                                        '"' => {
                                            ret.push(Token::String(
                                                resolved_chars.into_iter().collect(),
                                            ));
                                            break;
                                        }
                                        '\\' => {
                                            current_index += 1;
                                            match request.get(current_index + 1) {
                                                Some('u') => {
                                                    current_index += 1;
                                                    let mut unicode_hex: Vec<char> = _d();
                                                    while unicode_hex.len() < 4 {
                                                        match request.get(current_index + 1) {
                                                            Some(ch)
                                                                if matches!(
                                                                    ch,
                                                                    '0'..='9' | 'A'..='F' | 'a'..='f'
                                                                ) =>
                                                            {
                                                                unicode_hex.push(ch);
                                                                current_index += 1;
                                                            }
                                                            _ => panic!("Unexpected hex digit"),
                                                        }
                                                    }
                                                    resolved_chars.push(
                                                        char::from_u32(
                                                            u32::from_str_radix(
                                                                &unicode_hex
                                                                    .into_iter()
                                                                    .collect::<String>(),
                                                                16,
                                                            )
                                                            .unwrap(),
                                                        )
                                                        .expect("Couldn't convert hex to char"),
                                                    );
                                                }
                                                Some('"') => {
                                                    resolved_chars.push('"');
                                                }
                                                Some('\\') => {
                                                    resolved_chars.push('\\');
                                                }
                                                Some('/') => {
                                                    resolved_chars.push('/');
                                                }
                                                Some('b') => {
                                                    resolved_chars.push('\u{8}');
                                                }
                                                Some('f') => {
                                                    resolved_chars.push('\u{C}');
                                                }
                                                Some('n') => {
                                                    resolved_chars.push('\n');
                                                }
                                                Some('r') => {
                                                    resolved_chars.push('\r');
                                                }
                                                Some('t') => {
                                                    resolved_chars.push('\t');
                                                }
                                                _ => panic!("Unexpected escape"),
                                            }
                                        }
                                        ch => {
                                            resolved_chars.push(ch);
                                        }
                                    }
                                    current_index += 1;
                                }
                            }
                        }
                    }
                }
            }
            '-' | '0'..='9' => {
                let is_negative = ch == '-';
                let mut integer_part: Vec<char> = _d();
                if !is_negative {
                    integer_part.push(ch);
                }
                current_index += 1;
                while matches!(
                    request.get(current_index + 1),
                    Some(ch) if matches!(
                        ch,
                        '0'..='9'
                    )
                ) {
                    integer_part.push(request[current_index + 1]);
                    if integer_part.len() == 2 && integer_part[0] == '0' && integer_part[1] == '0' {
                        panic!("Can't have leading zero");
                    }
                    current_index += 1;
                }
                // HERE
            }
            UnicodeBOM => {}
        }

        current_index += 1;
    }
    ret
}

pub fn parse(tokens: &[Token]) -> Request {}
