use std::vec;

use squalid::_d;

use crate::{
    Document, ExecutableDefinition, FragmentSpread, InlineFragment, OperationType, Request,
    Selection, SelectionFieldBuilder,
};

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

impl Token {
    pub fn into_name(self) -> String {
        match self {
            Self::Name(name) => name,
            _ => panic!("Expected name"),
        }
    }
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
                                Some(ch) => match ch {
                                    '"' => {
                                        current_index += 1;
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
                                        current_index += 1;
                                        resolved_chars.push(*ch);
                                    }
                                },
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

pub fn parse(request: &[char]) -> Request {
    parse_tokens(lex(request))
}

pub fn parse_tokens(tokens: Vec<Token>) -> Request {
    let mut tokens_iter = tokens.into_iter();
    Request::new(Document::new({
        let mut definitions: Vec<ExecutableDefinition> = _d();
        match tokens_iter.next() {
            Some(token)
                if matches!(token, Token::LeftCurlyBracket)
                    || matches!(
                        &token,
                        Token::Name(name) if matches!(
                            &**name,
                            "query" // | "mutation" | "subscription"
                        )
                    ) =>
            {
                let operation_type = OperationType::Query;
                let mut name: Option<String> = _d();
                match token {
                    Token::Name(_parsed_operation_type) => match tokens_iter.next() {
                        Some(Token::LeftCurlyBracket) => {
                            parse_selection_set(&mut tokens_iter);
                        }
                        Some(Token::Name(parsed_name)) => {
                            name = Some(parsed_name);
                            match tokens_iter.next() {
                                Some(Token::LeftCurlyBracket) => {
                                    parse_selection_set(&mut tokens_iter);
                                }
                                _ => panic!("Expected selection set"),
                            }
                        }
                        _ => panic!("Expected query"),
                    },
                    _ => {
                        parse_selection_set(&mut tokens_iter);
                    }
                }
            }
            Some(Token::Name(name)) if name == "fragment" => {}
            _ => panic!("Expected definition"),
        }
        definitions
    }))
}

fn parse_selection_set(tokens_iter: &mut vec::IntoIter<Token>) -> Vec<Selection> {
    let mut ret: Vec<Selection> = _d();

    match tokens_iter.next() {
        Some(Token::DotDotDot) => match tokens_iter.next() {
            Some(token) => {
                if matches!(
                    &token,
                    Token::Name(name) if name != "on"
                ) {
                    ret.push(Selection::FragmentSpread(FragmentSpread::new(
                        token.into_name(),
                    )));
                } else if matches!(&token, Token::Name(_) | Token::LeftCurlyBracket) {
                    match token {
                        Token::Name(_) => {
                            let on = match tokens_iter.next() {
                                Some(Token::Name(on)) => on,
                                _ => panic!("Expected on"),
                            };
                            match tokens_iter.next() {
                                Some(Token::LeftCurlyBracket) => {
                                    ret.push(Selection::InlineFragment(InlineFragment::new(
                                        Some(on),
                                        parse_selection_set(tokens_iter),
                                    )));
                                }
                                _ => panic!("Expected selection set"),
                            }
                        }
                        Token::LeftCurlyBracket => {
                            ret.push(Selection::InlineFragment(InlineFragment::new(
                                None,
                                parse_selection_set(tokens_iter),
                            )));
                        }
                        _ => unreachable!(),
                    }
                } else {
                    panic!("Expected fragment selection");
                }
            }
            _ => {
                panic!("Expected fragment selection");
            }
        },
        Some(Token::Name(name)) => {
            ret.push(Selection::Field(
                SelectionFieldBuilder::default().build().unwrap(),
            ));
        }
        _ => panic!("Expected selection set"),
    }

    ret
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

    #[test]
    fn test_string() {
        lex_test(r#""""#, [Token::String("".to_owned())]);
        lex_test(r#""abc""#, [Token::String("abc".to_owned())]);
    }

    #[test]
    fn test_name() {
        lex_test("Foo", [Token::Name("Foo".to_owned())]);
    }

    #[test]
    fn test_int() {
        lex_test("35", [Token::Int(35)]);
    }

    #[test]
    fn test_dot_dot_dot() {
        lex_test("...a", [Token::DotDotDot, Token::Name("a".to_owned())]);
    }
}
