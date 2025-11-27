use std::{iter::Peekable, vec};

use squalid::_d;

use crate::{
    Argument, Document, ExecutableDefinition, FragmentDefinition, FragmentSpread, InlineFragment,
    OperationDefinitionBuilder, OperationType, Request, Selection, SelectionFieldBuilder, Value,
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

pub fn lex(request: impl IntoIterator<Item = char>) -> Vec<Token> {
    let mut request = request.into_iter().peekable();
    let mut ret = vec![];
    loop {
        match request.next() {
            Some(ch) => {
                match ch {
                    '!' => ret.push(Token::ExclamationPoint),
                    '$' => ret.push(Token::DollarSign),
                    '&' => ret.push(Token::Ampersand),
                    '(' => ret.push(Token::LeftParen),
                    ')' => ret.push(Token::RightParen),
                    '.' => {
                        assert!(request.next() == Some('.'));
                        assert!(request.next() == Some('.'));
                        ret.push(Token::DotDotDot);
                    }
                    ':' => ret.push(Token::Colon),
                    '=' => ret.push(Token::Equals),
                    '@' => ret.push(Token::AtSymbol),
                    '[' => ret.push(Token::LeftSquareBracket),
                    ']' => ret.push(Token::RightSquareBracket),
                    '{' => ret.push(Token::LeftCurlyBracket),
                    '|' => ret.push(Token::Pipe),
                    '}' => ret.push(Token::RightCurlyBracket),
                    'A'..='Z' | 'a'..='z' | '_' => {
                        let mut chars: Vec<char> = vec![ch];
                        loop {
                            match request.peek() {
                                Some(ch)
                                    if matches!(
                                        ch,
                                        'A'..='Z' | 'a'..='z' | '_' | '0'..='9'
                                    ) =>
                                {
                                    chars.push(request.next().unwrap());
                                }
                                _ => break,
                            }
                        }
                        ret.push(Token::Name(chars.iter().collect()));
                    }
                    '"' => match request.next() {
                        Some('"') => match request.next() {
                            Some('"') => {
                                unimplemented!()
                            }
                            _ => {
                                ret.push(Token::String("".to_owned()));
                            }
                        },
                        Some(ch) => {
                            let mut resolved_chars: Vec<char> = vec![ch];
                            loop {
                                match request.next() {
                                    None => panic!("expected closing double-quote"),
                                    Some(ch) => match ch {
                                        '"' => {
                                            ret.push(Token::String(
                                                resolved_chars.into_iter().collect(),
                                            ));
                                            break;
                                        }
                                        '\\' => match request.next() {
                                            Some('u') => {
                                                let mut unicode_hex: Vec<char> = _d();
                                                while unicode_hex.len() < 4 {
                                                    match request.next() {
                                                        Some(ch)
                                                            if matches!(
                                                                ch,
                                                                '0'..='9' | 'A'..='F' | 'a'..='f'
                                                            ) =>
                                                        {
                                                            unicode_hex.push(ch);
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
                                        },
                                        ch => {
                                            resolved_chars.push(ch);
                                        }
                                    },
                                }
                            }
                        }
                        _ => panic!("Expected end of string"),
                    },
                    ch @ '-' | ch @ '0'..='9' => {
                        let is_negative = ch == '-';
                        let mut integer_part: Vec<char> = _d();
                        if !is_negative {
                            integer_part.push(ch);
                        }
                        while matches!(
                            request.peek(),
                            Some(ch) if matches!(
                                ch,
                                '0'..='9'
                            )
                        ) {
                            integer_part.push(request.next().unwrap());
                            if integer_part.len() == 2
                                && integer_part[0] == '0'
                                && integer_part[1] == '0'
                            {
                                panic!("Can't have leading zero");
                            }
                        }
                        match request.peek() {
                            Some(ch) if matches!(ch, '.' | 'e' | 'E') => {
                                let ch = request.next().unwrap();
                                match ch {
                                    '.' => {
                                        let mut fractional_part: Vec<char> = _d();
                                        loop {
                                            match request.peek() {
                                                Some(ch) if matches!(ch, '0'..='9') => {
                                                    fractional_part.push(request.next().unwrap());
                                                }
                                                _ => {
                                                    if fractional_part.is_empty() {
                                                        panic!("expected fractional part digits");
                                                    }
                                                    break;
                                                }
                                            }
                                        }
                                        match request.peek() {
                                            Some(ch) if matches!(ch, 'e' | 'E') => {
                                                let _ = request.next().unwrap();
                                                // TODO: DRY this up wrt non-fractional-part
                                                // case below
                                                let mut has_exponent_negative_sign = false;
                                                if matches!(request.peek(), Some('-')) {
                                                    let _ = request.next().unwrap();
                                                    has_exponent_negative_sign = true;
                                                }
                                                let mut exponent_digits: Vec<char> = _d();
                                                loop {
                                                    match request.peek() {
                                                        Some(ch) if matches!(ch, '0'..='9') => {
                                                            exponent_digits
                                                                .push(request.next().unwrap());
                                                        }
                                                        _ => {
                                                            if exponent_digits.is_empty() {
                                                                panic!("Expected exponent digits");
                                                            }
                                                            ret.push(Token::Float(
                                                                format!(
                                                                    "{}{}.{}e{}{}",
                                                                    if is_negative {
                                                                        "-"
                                                                    } else {
                                                                        ""
                                                                    },
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
                                                        integer_part
                                                            .into_iter()
                                                            .collect::<String>(),
                                                        fractional_part
                                                            .into_iter()
                                                            .collect::<String>(),
                                                    )
                                                    .parse::<f64>()
                                                    .expect("Couldn't parse float"),
                                                ));
                                            }
                                        }
                                    }
                                    _ => {
                                        let mut has_exponent_negative_sign = false;
                                        if matches!(request.peek(), Some('-')) {
                                            let _ = request.next().unwrap();
                                            has_exponent_negative_sign = true;
                                        }
                                        let mut exponent_digits: Vec<char> = _d();
                                        loop {
                                            match request.peek() {
                                                Some(ch) if matches!(ch, '0'..='9') => {
                                                    exponent_digits.push(request.next().unwrap());
                                                }
                                                _ => {
                                                    if exponent_digits.is_empty() {
                                                        panic!("Expected exponent digits");
                                                    }
                                                    ret.push(Token::Float(
                                                        format!(
                                                            "{}{}e{}{}",
                                                            if is_negative { "-" } else { "" },
                                                            integer_part
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
                                }
                            }
                            _ => {
                                ret.push(Token::Int(
                                    i32::from_str_radix(
                                        &integer_part.into_iter().collect::<String>(),
                                        10,
                                    )
                                    .unwrap(),
                                ));
                            }
                        }
                    }
                    '#' => {
                        while matches!(
                            request.peek(),
                            Some(ch) if !matches!(
                                ch,
                                '\n' | '\r'
                            )
                        ) {
                            let _ = request.next().unwrap();
                        }
                    }
                    ch @ UNICODE_BOM
                    | ch @ '\u{9}'
                    | ch @ '\u{20}'
                    | ch @ ','
                    | ch @ '\n'
                    | ch @ '\r' => {
                        if ch == '\r' {
                            if request.peek() == Some(&'\n') {
                                let _ = request.next().unwrap();
                            }
                        }
                    }
                    _ => panic!("Unsupported char?"),
                }
            }
            None => return ret,
        }
    }
}

pub fn parse(request: impl IntoIterator<Item = char>) -> Request {
    parse_tokens(lex(request))
}

pub fn parse_tokens(tokens: Vec<Token>) -> Request {
    let mut tokens_iter = tokens.into_iter().peekable();
    Request::new(Document::new({
        let mut definitions: Vec<ExecutableDefinition> = _d();
        loop {
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
                    definitions.push({
                        let mut builder = OperationDefinitionBuilder::default();
                        builder = builder.operation_type(OperationType::Query);
                        match token {
                            Token::Name(_parsed_operation_type) => match tokens_iter.next() {
                                Some(Token::LeftCurlyBracket) => {
                                    builder = builder
                                        .selection_set(parse_selection_set(&mut tokens_iter));
                                    ExecutableDefinition::Operation(builder.build().unwrap())
                                }
                                Some(Token::Name(name)) => {
                                    builder = builder.name(name);
                                    match tokens_iter.next() {
                                        Some(Token::LeftCurlyBracket) => {
                                            builder = builder.selection_set(parse_selection_set(
                                                &mut tokens_iter,
                                            ));
                                            ExecutableDefinition::Operation(
                                                builder.build().unwrap(),
                                            )
                                        }
                                        _ => panic!("Expected selection set"),
                                    }
                                }
                                _ => panic!("Expected query"),
                            },
                            _ => {
                                builder =
                                    builder.selection_set(parse_selection_set(&mut tokens_iter));
                                ExecutableDefinition::Operation(builder.build().unwrap())
                            }
                        }
                    });
                }
                Some(Token::Name(name)) if name == "fragment" => {
                    definitions.push(ExecutableDefinition::Fragment(FragmentDefinition::new(
                        match tokens_iter.next() {
                            Some(Token::Name(name)) => {
                                if name == "on" {
                                    panic!("Saw `on` instead of fragment name");
                                }
                                name
                            }
                            _ => panic!("Expected fragment name"),
                        },
                        match tokens_iter.next() {
                            Some(Token::Name(name)) if name == "on" => match tokens_iter.next() {
                                Some(Token::Name(name)) => name,
                                _ => panic!("Expected fragment `on` name"),
                            },
                            _ => panic!("Expected fragment `on`"),
                        },
                        match tokens_iter.next() {
                            Some(Token::LeftCurlyBracket) => parse_selection_set(&mut tokens_iter),
                            _ => panic!("Expected selection set"),
                        },
                    )));
                }
                None => break definitions,
                _ => panic!("Expected definition"),
            }
        }
    }))
}

fn parse_selection_set(tokens_iter: &mut Peekable<vec::IntoIter<Token>>) -> Vec<Selection> {
    let mut ret: Vec<Selection> = _d();

    loop {
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
                ret.push(Selection::Field({
                    let mut builder = SelectionFieldBuilder::default();
                    builder = builder.name(name);
                    match tokens_iter.peek() {
                        Some(Token::LeftParen) => {
                            let _ = tokens_iter.next().unwrap();
                            builder = builder.arguments({
                                let mut arguments: Vec<Argument> = _d();
                                loop {
                                    match tokens_iter.next() {
                                        Some(Token::Name(name)) => {
                                            arguments.push(Argument::new(name, {
                                                if !matches!(tokens_iter.next(), Some(Token::Colon))
                                                {
                                                    panic!("Expected colon");
                                                }
                                                parse_value(tokens_iter, false)
                                            }));
                                        }
                                        Some(Token::RightParen) => {
                                            if arguments.is_empty() {
                                                panic!("Empty arguments");
                                            }
                                            break arguments;
                                        }
                                        _ => panic!("Expected argument"),
                                    }
                                }
                            });
                            match tokens_iter.peek() {
                                Some(Token::LeftCurlyBracket) => {
                                    let _ = tokens_iter.next().unwrap();
                                    builder
                                        .selection_set(parse_selection_set(tokens_iter))
                                        .build()
                                        .unwrap()
                                }
                                _ => builder.build().unwrap(),
                            }
                        }
                        Some(Token::LeftCurlyBracket) => {
                            let _ = tokens_iter.next().unwrap();
                            builder
                                .selection_set(parse_selection_set(tokens_iter))
                                .build()
                                .unwrap()
                        }
                        _ => builder.build().unwrap(),
                    }
                }));
            }
            Some(Token::RightCurlyBracket) => {
                if ret.is_empty() {
                    panic!("Empty selection");
                }
                return ret;
            }
            _ => panic!("Expected selection set"),
        }
    }
}

fn parse_value(tokens_iter: &mut Peekable<vec::IntoIter<Token>>, is_const: bool) -> Value {
    match tokens_iter.next() {
        Some(Token::Int(int)) => Value::Int(int),
        Some(Token::String(string)) => Value::String(string),
        _ => panic!("Expected value"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex_test(request: &str, expected_tokens: impl IntoIterator<Item = Token>) {
        assert_eq!(
            lex(request.chars()),
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
