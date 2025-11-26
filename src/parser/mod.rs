use squalid::_d;

use crate::Request;

const UNICODE_BOM: char = '\u{feff}';

#[derive(Debug, PartialEq)]
pub enum Token {
    ExclamationPoint,
    DollarSign,
    Ampersand,
    LeftParen,
    RightParen,
    DotDotDot,
    Colon,
    Equals,
    AtSymbol,
    LeftSquareBracket,
    RightSquareBracket,
    LeftCurlyBracket,
    Pipe,
    RightCurlyBracket,
    Name(String),
    String(String),
    Int(i32),
    Float(f64),
}

pub fn lex(request: &[char]) -> Vec<Token> {
    let mut ret = vec![];
    let mut current_index = 0;
    while current_index < request.len() {
        match request[current_index] {
            '!' => {
                current_index += 1;
                ret.push(Token::ExclamationPoint);
            }
            '$' => {
                current_index += 1;
                ret.push(Token::DollarSign);
            }
            '&' => {
                current_index += 1;
                ret.push(Token::Ampersand);
            }
            '(' => {
                current_index += 1;
                ret.push(Token::LeftParen);
            }
            ')' => {
                current_index += 1;
                ret.push(Token::RightParen);
            }
            '.' => {
                current_index += 1;
                assert!(request.get(current_index) == Some(&'.'));
                current_index += 1;
                assert!(request.get(current_index) == Some(&'.'));
                current_index += 1;
                ret.push(Token::DotDotDot);
            }
            ':' => {
                current_index += 1;
                ret.push(Token::Colon);
            }
            '=' => {
                current_index += 1;
                ret.push(Token::Equals);
            }
            '@' => {
                current_index += 1;
                ret.push(Token::AtSymbol);
            }
            '[' => {
                current_index += 1;
                ret.push(Token::LeftSquareBracket);
            }
            ']' => {
                current_index += 1;
                ret.push(Token::RightSquareBracket);
            }
            '{' => {
                current_index += 1;
                ret.push(Token::LeftCurlyBracket);
            }
            '|' => {
                current_index += 1;
                ret.push(Token::Pipe);
            }
            '}' => {
                current_index += 1;
                ret.push(Token::RightCurlyBracket);
            }
            'A'..='Z' | 'a'..='z' | '_' => {
                let initial_index = current_index;
                current_index += 1;
                while matches!(
                    request.get(current_index),
                    Some(ch) if matches!(
                        ch,
                        'A'..='Z' | 'a'..='z' | '_' | '0'..='9'
                    )
                ) {
                    current_index += 1;
                }
                ret.push(Token::Name(
                    request[initial_index..current_index].iter().collect(),
                ));
            }
            '"' => {
                current_index += 1;
                match request.get(current_index) {
                    Some('"') => {
                        current_index += 1;
                        match request.get(current_index) {
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
                            match request.get(current_index) {
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
                                            match request.get(current_index) {
                                                Some('u') => {
                                                    current_index += 1;
                                                    let mut unicode_hex: Vec<char> = _d();
                                                    while unicode_hex.len() < 4 {
                                                        match request.get(current_index) {
                                                            Some(ch)
                                                                if matches!(
                                                                    ch,
                                                                    '0'..='9' | 'A'..='F' | 'a'..='f'
                                                                ) =>
                                                            {
                                                                unicode_hex.push(*ch);
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
                                            resolved_chars.push(*ch);
                                        }
                                    }
                                    current_index += 1;
                                }
                            }
                        }
                    }
                }
            }
            ch @ '-' | ch @ '0'..='9' => {
                current_index += 1;
                let is_negative = ch == '-';
                let mut integer_part: Vec<char> = _d();
                if !is_negative {
                    integer_part.push(ch);
                }
                while matches!(
                    request.get(current_index),
                    Some(ch) if matches!(
                        ch,
                        '0'..='9'
                    )
                ) {
                    integer_part.push(request[current_index]);
                    if integer_part.len() == 2 && integer_part[0] == '0' && integer_part[1] == '0' {
                        panic!("Can't have leading zero");
                    }
                    current_index += 1;
                }
                match request.get(current_index) {
                    Some(ch) if matches!(ch, '.' | 'e' | 'E') => {
                        match ch {
                            '.' => {
                                current_index += 1;
                                let mut fractional_part: Vec<char> = _d();
                                loop {
                                    match request.get(current_index) {
                                        Some(ch) if matches!(ch, '0'..='9') => {
                                            fractional_part.push(*ch);
                                        }
                                        _ => {
                                            if fractional_part.is_empty() {
                                                panic!("expected fractional part digits");
                                            }
                                            break;
                                        }
                                    }
                                    current_index += 1;
                                }
                                match request.get(current_index) {
                                    Some(ch) if matches!(ch, 'e' | 'E') => {
                                        current_index += 1;
                                        // TODO: DRY this up wrt non-fractional-part
                                        // case below
                                        let mut has_exponent_negative_sign = false;
                                        if matches!(request.get(current_index), Some('-')) {
                                            has_exponent_negative_sign = true;
                                            current_index += 1;
                                        }
                                        let mut exponent_digits: Vec<char> = _d();
                                        loop {
                                            match request.get(current_index) {
                                                Some(ch) if matches!(ch, '0'..='9') => {
                                                    exponent_digits.push(*ch);
                                                    current_index += 1;
                                                }
                                                _ => {
                                                    if exponent_digits.is_empty() {
                                                        panic!("Expected exponent digits");
                                                    }
                                                    ret.push(Token::Float(
                                                        format!(
                                                            "{}{}.{}e{}{}",
                                                            if is_negative { "-" } else { "" },
                                                            integer_part
                                                                .into_iter()
                                                                .collect::<String>(),
                                                            fractional_part
                                                                .into_iter()
                                                                .collect::<String>(),
                                                            if has_exponent_negative_sign {
                                                                "-"
                                                            } else {
                                                                ""
                                                            },
                                                            exponent_digits
                                                                .into_iter()
                                                                .collect::<String>(),
                                                        )
                                                        .parse::<f64>()
                                                        .expect("Couldn't parse float"),
                                                    ));
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    _ => {
                                        ret.push(Token::Float(
                                            format!(
                                                "{}{}.{}",
                                                if is_negative { "-" } else { "" },
                                                integer_part.into_iter().collect::<String>(),
                                                fractional_part.into_iter().collect::<String>(),
                                            )
                                            .parse::<f64>()
                                            .expect("Couldn't parse float"),
                                        ));
                                    }
                                }
                            }
                            _ => {
                                current_index += 1;
                                let mut has_exponent_negative_sign = false;
                                if matches!(request.get(current_index), Some('-')) {
                                    has_exponent_negative_sign = true;
                                    current_index += 1;
                                }
                                let mut exponent_digits: Vec<char> = _d();
                                loop {
                                    match request.get(current_index) {
                                        Some(ch) if matches!(ch, '0'..='9') => {
                                            exponent_digits.push(*ch);
                                            current_index += 1;
                                        }
                                        _ => {
                                            if exponent_digits.is_empty() {
                                                panic!("Expected exponent digits");
                                            }
                                            ret.push(Token::Float(
                                                format!(
                                                    "{}{}e{}{}",
                                                    if is_negative { "-" } else { "" },
                                                    integer_part.into_iter().collect::<String>(),
                                                    if has_exponent_negative_sign {
                                                        "-"
                                                    } else {
                                                        ""
                                                    },
                                                    exponent_digits.into_iter().collect::<String>(),
                                                )
                                                .parse::<f64>()
                                                .expect("Couldn't parse float"),
                                            ));
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        ret.push(Token::Int(
                            i32::from_str_radix(&integer_part.into_iter().collect::<String>(), 10)
                                .unwrap(),
                        ));
                    }
                }
            }
            '#' => {
                current_index += 1;
                while matches!(
                    request.get(current_index),
                    Some(ch) if !matches!(
                        ch,
                        '\n' | '\r'
                    )
                ) {
                    current_index += 1;
                }
            }
            ch @ UNICODE_BOM | ch @ '\u{9}' | ch @ '\u{20}' | ch @ ',' | ch @ '\n' | ch @ '\r' => {
                current_index += 1;
                if ch == '\r' {
                    if request.get(current_index) == Some(&'\n') {
                        current_index += 1;
                    }
                }
            }
            _ => panic!("Unsupported char?"),
        }
    }
    ret
}

pub fn parse(tokens: &[Token]) -> Request {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex_test(request: &str, expected_tokens: impl IntoIterator<Item = Token>) {
        assert_eq!(
            lex(&request.chars().collect::<Vec<_>>(),),
            expected_tokens.into_iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_lex_float() {
        lex_test("10.1", [Token::Float(10.1)]);
        lex_test("-10.1", [Token::Float(-10.1)]);
        lex_test("10.1e53", [Token::Float(10.1e53)]);
        lex_test("10.1e-53", [Token::Float(10.1e-53)]);
        lex_test("10e53", [Token::Float(10e53)]);
        lex_test("10e-53", [Token::Float(10e-53)]);
    }
}
