use std::{iter::Peekable, vec};

use squalid::_d;

use crate::{
    Argument, CharsEmitter, Document, ExecutableDefinition, FragmentDefinition, FragmentSpread,
    InlineFragment, Location, OperationDefinitionBuilder, OperationType, PositionsTracker, Request,
    Selection, SelectionFieldBuilder, Value,
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

pub fn lex<TRequest>(request: TRequest) -> Lex<TRequest::IntoIter>
where
    TRequest: IntoIterator<Item = char>,
{
    Lex {
        request: CharsEmitter::new(request.into_iter().peekable()),
        has_errored: false,
    }
}

pub struct Lex<TRequest: Iterator<Item = char>> {
    request: CharsEmitter<TRequest>,
    has_errored: bool,
}

impl<TRequest: Iterator<Item = char>> Lex<TRequest> {
    fn error(&mut self, message: &str) -> LexError {
        self.has_errored = true;
        LexError::new(message.to_owned())
    }
}

impl<TRequest> Iterator for Lex<TRequest>
where
    TRequest: Iterator<Item = char>,
{
    type Item = LexResult<Token>;

    fn next(&mut self) -> Option<LexResult<Token>> {
        if self.has_errored {
            panic!("Shouldn't keep calling lexer after lexing error");
        }

        loop {
            PositionsTracker::emit_token_pre_start();
            match self.request.next() {
                Some(ch) => {
                    let maybe_token = match ch {
                        '!' => Some(Token::ExclamationPoint),
                        '$' => Some(Token::DollarSign),
                        '&' => Some(Token::Ampersand),
                        '(' => Some(Token::LeftParen),
                        ')' => Some(Token::RightParen),
                        '.' => {
                            assert!(self.request.next() == Some('.'));
                            assert!(self.request.next() == Some('.'));
                            Some(Token::DotDotDot)
                        }
                        ':' => Some(Token::Colon),
                        '=' => Some(Token::Equals),
                        '@' => Some(Token::AtSymbol),
                        '[' => Some(Token::LeftSquareBracket),
                        ']' => Some(Token::RightSquareBracket),
                        '{' => Some(Token::LeftCurlyBracket),
                        '|' => Some(Token::Pipe),
                        '}' => Some(Token::RightCurlyBracket),
                        'A'..='Z' | 'a'..='z' | '_' => {
                            let mut chars: Vec<char> = vec![ch];
                            loop {
                                match self.request.peek() {
                                    Some(ch)
                                        if matches!(
                                            ch,
                                            'A'..='Z' | 'a'..='z' | '_' | '0'..='9'
                                        ) =>
                                    {
                                        chars.push(self.request.next().unwrap());
                                    }
                                    _ => break,
                                }
                            }
                            Some(Token::Name(chars.iter().collect()))
                        }
                        '"' => match self.request.next() {
                            Some('"') => match self.request.next() {
                                Some('"') => {
                                    unimplemented!()
                                }
                                _ => Some(Token::String("".to_owned())),
                            },
                            Some(ch) => {
                                let mut resolved_chars: Vec<char> = vec![ch];
                                loop {
                                    match self.request.next() {
                                        None => {
                                            return Some(Err(
                                                self.error("expected closing double-quote")
                                            ))
                                        }
                                        Some(ch) => match ch {
                                            '"' => {
                                                break Some(Token::String(
                                                    resolved_chars.into_iter().collect(),
                                                ));
                                            }
                                            '\\' => match self.request.next() {
                                                Some('u') => {
                                                    let mut unicode_hex: Vec<char> = _d();
                                                    while unicode_hex.len() < 4 {
                                                        match self.request.next() {
                                                            Some(ch)
                                                                if matches!(
                                                                    ch,
                                                                    '0'..='9' | 'A'..='F' | 'a'..='f'
                                                                ) =>
                                                            {
                                                                unicode_hex.push(ch);
                                                            }
                                                            _ => {
                                                                return Some(Err(self.error(
                                                                    "Unexpected hex digit",
                                                                )))
                                                            }
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
                                                _ => {
                                                    return Some(Err(
                                                        self.error("Unexpected escape")
                                                    ))
                                                }
                                            },
                                            ch => {
                                                resolved_chars.push(ch);
                                            }
                                        },
                                    }
                                }
                            }
                            _ => return Some(Err(self.error("Expected end of string"))),
                        },
                        ch @ '-' | ch @ '0'..='9' => {
                            let is_negative = ch == '-';
                            let mut integer_part: Vec<char> = _d();
                            if !is_negative {
                                integer_part.push(ch);
                            }
                            while matches!(
                                self.request.peek(),
                                Some(ch) if matches!(
                                    ch,
                                    '0'..='9'
                                )
                            ) {
                                integer_part.push(self.request.next().unwrap());
                                if integer_part.len() == 2
                                    && integer_part[0] == '0'
                                    && integer_part[1] == '0'
                                {
                                    return Some(Err(self.error("Can't have leading zero")));
                                }
                            }
                            match self.request.peek() {
                                Some(ch) if matches!(ch, '.' | 'e' | 'E') => {
                                    let ch = self.request.next().unwrap();
                                    match ch {
                                        '.' => {
                                            let mut fractional_part: Vec<char> = _d();
                                            loop {
                                                match self.request.peek() {
                                                    Some(ch) if matches!(ch, '0'..='9') => {
                                                        fractional_part
                                                            .push(self.request.next().unwrap());
                                                    }
                                                    _ => {
                                                        if fractional_part.is_empty() {
                                                            return Some(Err(self.error(
                                                                "expected fractional part digits",
                                                            )));
                                                        }
                                                        break;
                                                    }
                                                }
                                            }
                                            match self.request.peek() {
                                                Some(ch) if matches!(ch, 'e' | 'E') => {
                                                    let _ = self.request.next().unwrap();
                                                    // TODO: DRY this up wrt non-fractional-part
                                                    // case below
                                                    let mut has_exponent_negative_sign = false;
                                                    if matches!(self.request.peek(), Some('-')) {
                                                        let _ = self.request.next().unwrap();
                                                        has_exponent_negative_sign = true;
                                                    }
                                                    let mut exponent_digits: Vec<char> = _d();
                                                    loop {
                                                        match self.request.peek() {
                                                            Some(ch) if matches!(ch, '0'..='9') => {
                                                                exponent_digits.push(
                                                                    self.request.next().unwrap(),
                                                                );
                                                            }
                                                            _ => {
                                                                if exponent_digits.is_empty() {
                                                                    return Some(Err(self.error(
                                                                        "Expected exponent digits",
                                                                    )));
                                                                }
                                                                break Some(Token::Float(
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
                                                            }
                                                        }
                                                    }
                                                }
                                                _ => Some(Token::Float(
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
                                                )),
                                            }
                                        }
                                        _ => {
                                            let mut has_exponent_negative_sign = false;
                                            if matches!(self.request.peek(), Some('-')) {
                                                let _ = self.request.next().unwrap();
                                                has_exponent_negative_sign = true;
                                            }
                                            let mut exponent_digits: Vec<char> = _d();
                                            loop {
                                                match self.request.peek() {
                                                    Some(ch) if matches!(ch, '0'..='9') => {
                                                        exponent_digits
                                                            .push(self.request.next().unwrap());
                                                    }
                                                    _ => {
                                                        if exponent_digits.is_empty() {
                                                            return Some(Err(self.error(
                                                                "Expected exponent digits",
                                                            )));
                                                        }
                                                        break Some(Token::Float(
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
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => Some(Token::Int(
                                    i32::from_str_radix(
                                        &integer_part.into_iter().collect::<String>(),
                                        10,
                                    )
                                    .unwrap(),
                                )),
                            }
                        }
                        '#' => {
                            while matches!(
                                self.request.peek(),
                                Some(ch) if !matches!(
                                    ch,
                                    '\n' | '\r'
                                )
                            ) {
                                let _ = self.request.next().unwrap();
                            }
                            None
                        }
                        ch @ UNICODE_BOM
                        | ch @ '\u{9}'
                        | ch @ '\u{20}'
                        | ch @ ','
                        | ch @ '\n'
                        | ch @ '\r' => {
                            if ch == '\r' {
                                if self.request.peek() == Some(&'\n') {
                                    let _ = self.request.next().unwrap();
                                }
                            }
                            None
                        }
                        ch => {
                            return Some(Err(self.error(&format!(
                                "Unsupported char: Unicode code point `{}`",
                                u32::from(ch)
                            ))))
                        }
                    };
                    match maybe_token {
                        Some(token) => return Some(Ok(token)),
                        None => {}
                    }
                }
                None => return None,
            }
        }
    }
}

pub fn parse(request: impl IntoIterator<Item = char>) -> ParseResult<Request> {
    parse_tokens(lex(request))
}

pub fn parse_tokens(tokens: impl IntoIterator<Item = LexResult<Token>>) -> ParseResult<Request> {
    let mut tokens = tokens.into_iter().peekable();
    Ok(Request::new(Document::new({
        let mut definitions: Vec<ExecutableDefinition> = _d();
        loop {
            match tokens.next().transpose()? {
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
                    PositionsTracker::emit_operation();
                    definitions.push({
                        let mut builder = OperationDefinitionBuilder::default();
                        builder = builder.operation_type(OperationType::Query);
                        match token {
                            Token::Name(_parsed_operation_type) => match tokens
                                .next()
                                .transpose()?
                            {
                                Some(Token::LeftCurlyBracket) => ExecutableDefinition::Operation(
                                    builder
                                        .selection_set(parse_selection_set(&mut tokens)?)
                                        .build()
                                        .unwrap(),
                                ),
                                Some(Token::Name(name)) => {
                                    builder = builder.name(name);
                                    match tokens.next().transpose()? {
                                        Some(Token::LeftCurlyBracket) => {
                                            ExecutableDefinition::Operation(
                                                builder
                                                    .selection_set(parse_selection_set(
                                                        &mut tokens,
                                                    )?)
                                                    .build()
                                                    .unwrap(),
                                            )
                                        }
                                        _ => panic!("Expected selection set"),
                                    }
                                }
                                _ => panic!("Expected query"),
                            },
                            _ => ExecutableDefinition::Operation(
                                builder
                                    .selection_set(parse_selection_set(&mut tokens)?)
                                    .build()
                                    .unwrap(),
                            ),
                        }
                    });
                }
                Some(Token::Name(name)) if name == "fragment" => {
                    PositionsTracker::emit_fragment_definition();
                    definitions.push(ExecutableDefinition::Fragment(FragmentDefinition::new(
                        match tokens.next().transpose()? {
                            Some(Token::Name(name)) => {
                                if name == "on" {
                                    panic!("Saw `on` instead of fragment name");
                                }
                                name
                            }
                            _ => panic!("Expected fragment name"),
                        },
                        match tokens.next().transpose()? {
                            Some(Token::Name(name)) if name == "on" => {
                                match tokens.next().transpose()? {
                                    Some(Token::Name(name)) => name,
                                    _ => panic!("Expected fragment `on` name"),
                                }
                            }
                            _ => panic!("Expected fragment `on`"),
                        },
                        match tokens.next().transpose()? {
                            Some(Token::LeftCurlyBracket) => parse_selection_set(&mut tokens)?,
                            _ => panic!("Expected selection set"),
                        },
                    )));
                }
                None => break definitions,
                _ => panic!("Expected definition"),
            }
        }
    })))
}

fn parse_selection_set<TIterator>(tokens: &mut Peekable<TIterator>) -> ParseResult<Vec<Selection>>
where
    TIterator: Iterator<Item = LexResult<Token>>,
{
    PositionsTracker::emit_selection_set();
    let mut ret: Vec<Selection> = _d();

    loop {
        match tokens.next().transpose()? {
            Some(Token::DotDotDot) => {
                PositionsTracker::emit_selection_inline_fragment_or_fragment_spread();
                ret.push(match tokens.next().transpose()? {
                    Some(token) => {
                        if matches!(
                            &token,
                            Token::Name(name) if name != "on"
                        ) {
                            PositionsTracker::emit_selection_fragment_spread();
                            Selection::FragmentSpread(FragmentSpread::new(token.into_name()))
                        } else if matches!(&token, Token::Name(_) | Token::LeftCurlyBracket) {
                            PositionsTracker::emit_selection_inline_fragment();
                            match token {
                                Token::Name(_) => {
                                    let on = match tokens.next().transpose()? {
                                        Some(Token::Name(on)) => on,
                                        _ => panic!("Expected on"),
                                    };
                                    match tokens.next().transpose()? {
                                        Some(Token::LeftCurlyBracket) => {
                                            Selection::InlineFragment(InlineFragment::new(
                                                Some(on),
                                                parse_selection_set(tokens)?,
                                            ))
                                        }
                                        _ => panic!("Expected selection set"),
                                    }
                                }
                                Token::LeftCurlyBracket => Selection::InlineFragment(
                                    InlineFragment::new(None, parse_selection_set(tokens)?),
                                ),
                                _ => unreachable!(),
                            }
                        } else {
                            panic!("Expected fragment selection");
                        }
                    }
                    _ => {
                        panic!("Expected fragment selection");
                    }
                });
            }
            Some(Token::Name(name)) => {
                PositionsTracker::emit_selection_field();
                ret.push(Selection::Field({
                    let mut builder = SelectionFieldBuilder::default();
                    builder = builder.name(name);
                    match tokens.peek() {
                        Some(Ok(Token::LeftParen)) => {
                            let _ = tokens.next().unwrap();
                            builder = builder.arguments({
                                let mut arguments: Vec<Argument> = _d();
                                loop {
                                    match tokens.next().transpose()? {
                                        Some(Token::Name(name)) => {
                                            PositionsTracker::emit_argument();
                                            arguments.push(Argument::new(name, {
                                                if !matches!(
                                                    tokens.next().transpose()?,
                                                    Some(Token::Colon)
                                                ) {
                                                    panic!("Expected colon");
                                                }
                                                parse_value(tokens, false)?
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
                            match tokens.peek() {
                                Some(Ok(Token::LeftCurlyBracket)) => {
                                    let _ = tokens.next().unwrap();
                                    builder
                                        .selection_set(parse_selection_set(tokens)?)
                                        .build()
                                        .unwrap()
                                }
                                _ => builder.build().unwrap(),
                            }
                        }
                        Some(Ok(Token::LeftCurlyBracket)) => {
                            let _ = tokens.next().unwrap();
                            builder
                                .selection_set(parse_selection_set(tokens)?)
                                .build()
                                .unwrap()
                        }
                        _ => builder.build().unwrap(),
                    }
                }));
            }
            Some(Token::RightCurlyBracket) => {
                PositionsTracker::emit_end_selection_set();
                if ret.is_empty() {
                    panic!("Empty selection");
                }
                return Ok(ret);
            }
            _ => panic!("Expected selection set"),
        }
    }
}

fn parse_value<TIterator>(tokens: &mut Peekable<TIterator>, is_const: bool) -> ParseResult<Value>
where
    TIterator: Iterator<Item = LexResult<Token>>,
{
    Ok(match tokens.next().transpose()? {
        Some(Token::Int(int)) => Value::Int(int),
        Some(Token::String(string)) => Value::String(string),
        _ => panic!("Expected value"),
    })
}

#[derive(Debug)]
pub struct LexError {
    pub message: String,
    pub location: Option<Location>,
}

impl LexError {
    pub fn new(message: String) -> Self {
        Self {
            message,
            location: _d(),
        }
    }
}

#[derive(Debug)]
pub enum ParseOrLexError {
    Lex(LexError),
    Parse(ParseError),
}

impl ParseOrLexError {
    pub fn message(&self) -> &str {
        match self {
            Self::Lex(error) => &error.message,
            Self::Parse(error) => &error.message,
        }
    }

    pub fn location(&self) -> Option<Location> {
        match self {
            Self::Lex(error) => error.location,
            Self::Parse(error) => error.location,
        }
    }
}

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub location: Option<Location>,
}

impl ParseError {
    pub fn new(message: String) -> Self {
        Self {
            message,
            location: _d(),
        }
    }
}

impl From<LexError> for ParseOrLexError {
    fn from(value: LexError) -> Self {
        Self::Lex(value)
    }
}

impl From<ParseError> for ParseOrLexError {
    fn from(value: ParseError) -> Self {
        Self::Parse(value)
    }
}

type LexResult<TValue> = Result<TValue, LexError>;

type ParseResult<TValue> = Result<TValue, ParseOrLexError>;

#[cfg(test)]
mod tests {
    use super::*;

    fn lex_test(request: &str, expected_tokens: impl IntoIterator<Item = Token>) {
        assert_eq!(
            lex(request.chars()).map(Result::unwrap).collect::<Vec<_>>(),
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
