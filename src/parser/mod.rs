use std::{iter::Peekable, vec};

use squalid::_d;

use crate::{
    Argument, CharsEmitter, Directive, Document, ExecutableDefinition, FragmentDefinition,
    FragmentSpread, InlineFragment, Location, OperationDefinitionBuilder, OperationType,
    PositionsTracker, Request, Selection, SelectionFieldBuilder, Value,
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
        LexError::new(
            message.to_owned(),
            PositionsTracker::current().map(|positions_tracker| positions_tracker.last_char()),
        )
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
                                let mut resolved_chars: Vec<char> =
                                    vec![match regular_or_escaped_string_char(ch, self) {
                                        Ok(ch) => ch,
                                        Err(error) => return Some(Err(error)),
                                    }];
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
                                            ch => resolved_chars.push(
                                                match regular_or_escaped_string_char(ch, self) {
                                                    Ok(ch) => ch,
                                                    Err(error) => return Some(Err(error)),
                                                },
                                            ),
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
                                if integer_part.len() == 2 && integer_part[0] == '0' {
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
                                                                    // TODO: look for .expect()'s
                                                                    // /.unwrap()'s also as panic
                                                                    // sites that need to be
                                                                    // converted to Result's
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

fn regular_or_escaped_string_char<TRequest>(ch: char, lex: &mut Lex<TRequest>) -> LexResult<char>
where
    TRequest: Iterator<Item = char>,
{
    Ok(match ch {
        '"' => unreachable!(),
        '\\' => match lex.request.next() {
            Some('u') => {
                let mut unicode_hex: Vec<char> = _d();
                while unicode_hex.len() < 4 {
                    match lex.request.next() {
                        Some(ch)
                            if matches!(
                                ch,
                                '0'..='9' | 'A'..='F' | 'a'..='f'
                            ) =>
                        {
                            unicode_hex.push(ch);
                        }
                        _ => return Err(lex.error("Unexpected hex digit")),
                    }
                }
                char::from_u32(
                    u32::from_str_radix(&unicode_hex.into_iter().collect::<String>(), 16).unwrap(),
                )
                .expect("Couldn't convert hex to char")
            }
            Some('"') => '"',
            Some('\\') => '\\',
            Some('/') => '/',
            Some('b') => '\u{8}',
            Some('f') => '\u{C}',
            Some('n') => '\n',
            Some('r') => '\r',
            Some('t') => '\t',
            _ => return Err(lex.error("Unexpected escape")),
        },
        ch => ch,
    })
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
                                Some(Token::AtSymbol) => {
                                    builder =
                                        builder.directives(parse_directives(&mut tokens, false)?);
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
                                        _ => {
                                            return Err(parse_error("Expected selection set").into())
                                        }
                                    }
                                }
                                Some(Token::Name(name)) => {
                                    builder = builder.name(name);
                                    match tokens.next().transpose()? {
                                        Some(Token::AtSymbol) => {
                                            builder = builder
                                                .directives(parse_directives(&mut tokens, false)?);
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
                                                _ => {
                                                    return Err(parse_error(
                                                        "Expected selection set",
                                                    )
                                                    .into())
                                                }
                                            }
                                        }
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
                                        _ => {
                                            return Err(parse_error("Expected selection set").into())
                                        }
                                    }
                                }
                                _ => return Err(parse_error("Expected query").into()),
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
                                    return Err(
                                        parse_error("Saw `on` instead of fragment name").into()
                                    );
                                }
                                name
                            }
                            _ => return Err(parse_error("Expected fragment name").into()),
                        },
                        match tokens.next().transpose()? {
                            Some(Token::Name(name)) if name == "on" => {
                                match tokens.next().transpose()? {
                                    Some(Token::Name(name)) => name,
                                    _ => {
                                        return Err(
                                            parse_error("Expected fragment `on` name").into()
                                        )
                                    }
                                }
                            }
                            _ => return Err(parse_error("Expected fragment `on`").into()),
                        },
                        match tokens.peek() {
                            Some(Ok(Token::AtSymbol)) => {
                                let _ = tokens.next().unwrap().unwrap();
                                parse_directives(&mut tokens, false)?
                            }
                            _ => _d(),
                        },
                        match tokens.next().transpose()? {
                            Some(Token::LeftCurlyBracket) => parse_selection_set(&mut tokens)?,
                            _ => return Err(parse_error("Expected selection set").into()),
                        },
                    )));
                }
                None => break definitions,
                _ => return Err(parse_error("Expected definition").into()),
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
                            Selection::FragmentSpread(FragmentSpread::new(
                                token.into_name(),
                                match tokens.peek() {
                                    Some(Ok(Token::AtSymbol)) => {
                                        let _ = tokens.next().unwrap().unwrap();
                                        parse_directives(tokens, false)?
                                    }
                                    _ => _d(),
                                },
                            ))
                        } else if matches!(
                            &token,
                            Token::Name(_) | Token::LeftCurlyBracket | Token::AtSymbol
                        ) {
                            PositionsTracker::emit_selection_inline_fragment();
                            Selection::InlineFragment(InlineFragment::new(
                                if matches!(token, Token::Name(_)) {
                                    Some(match tokens.next().transpose()? {
                                        Some(Token::Name(on)) => on,
                                        _ => return Err(parse_error("Expected on").into()),
                                    })
                                } else {
                                    None
                                },
                                if matches!(token, Token::AtSymbol)
                                    || matches!(token, Token::Name(_))
                                        && matches!(tokens.peek(), Some(Ok(Token::AtSymbol)))
                                {
                                    if matches!(token, Token::Name(_)) {
                                        let _ = tokens.next().unwrap().unwrap();
                                    }
                                    parse_directives(tokens, false)?
                                } else {
                                    _d()
                                },
                                if matches!(token, Token::LeftCurlyBracket) {
                                    parse_selection_set(tokens)?
                                } else {
                                    match tokens.next().transpose()? {
                                        Some(Token::LeftCurlyBracket) => {
                                            parse_selection_set(tokens)?
                                        }
                                        _ => {
                                            return Err(parse_error("Expected selection set").into())
                                        }
                                    }
                                },
                            ))
                        } else {
                            return Err(parse_error("Expected fragment selection").into());
                        }
                    }
                    _ => {
                        return Err(parse_error("Expected fragment selection").into());
                    }
                });
            }
            Some(Token::Name(name)) => {
                PositionsTracker::emit_selection_field();
                ret.push(Selection::Field({
                    let mut builder = SelectionFieldBuilder::default();
                    builder = builder.name(name);
                    if matches!(tokens.peek(), Some(Ok(Token::LeftParen))) {
                        builder = builder.arguments(parse_arguments(tokens, false)?);
                    }
                    if matches!(tokens.peek(), Some(Ok(Token::AtSymbol))) {
                        let _ = tokens.next().unwrap().unwrap();
                        builder = builder.directives(parse_directives(tokens, false)?);
                    }
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
                }));
            }
            Some(Token::RightCurlyBracket) => {
                PositionsTracker::emit_end_selection_set();
                if ret.is_empty() {
                    return Err(parse_error("Empty selection").into());
                }
                return Ok(ret);
            }
            _ => return Err(parse_error("Expected selection set").into()),
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
        Some(Token::Name(name)) if name == "null" => Value::Null,
        _ => return Err(parse_error("Expected value").into()),
    })
}

fn parse_directives<TIterator>(
    tokens: &mut Peekable<TIterator>,
    is_const: bool,
) -> ParseResult<Vec<Directive>>
where
    TIterator: Iterator<Item = LexResult<Token>>,
{
    PositionsTracker::emit_start_directives();
    let mut ret = vec![parse_directive(tokens, is_const)?];
    while let Some(Ok(Token::AtSymbol)) = tokens.peek() {
        let _ = tokens.next().unwrap().unwrap();
        ret.push(parse_directive(tokens, is_const)?);
    }
    PositionsTracker::emit_end_directives();
    Ok(ret)
}

fn parse_directive<TIterator>(
    tokens: &mut Peekable<TIterator>,
    is_const: bool,
) -> ParseResult<Directive>
where
    TIterator: Iterator<Item = LexResult<Token>>,
{
    PositionsTracker::emit_directive();
    Ok(Directive::new(
        match tokens.next().transpose()? {
            Some(Token::Name(name)) => name,
            _ => return Err(parse_error("Expected directive name").into()),
        },
        match tokens.peek() {
            Some(Ok(Token::LeftParen)) => Some(parse_arguments(tokens, is_const)?),
            _ => _d(),
        },
    ))
}

fn parse_arguments<TIterator>(
    tokens: &mut Peekable<TIterator>,
    is_const: bool,
) -> ParseResult<Vec<Argument>>
where
    TIterator: Iterator<Item = LexResult<Token>>,
{
    let _ = tokens.next().unwrap().unwrap();
    let mut ret: Vec<Argument> = _d();
    loop {
        match tokens.next().transpose()? {
            Some(Token::Name(name)) => {
                PositionsTracker::emit_argument();
                ret.push(Argument::new(name, {
                    if !matches!(tokens.next().transpose()?, Some(Token::Colon)) {
                        return Err(parse_error("Expected colon").into());
                    }
                    parse_value(tokens, is_const)?
                }));
            }
            Some(Token::RightParen) => {
                if ret.is_empty() {
                    return Err(parse_error("Empty arguments").into());
                }
                return Ok(ret);
            }
            _ => return Err(parse_error("Expected argument").into()),
        }
    }
}

fn parse_error(message: &str) -> ParseError {
    ParseError::new(
        message.to_owned(),
        PositionsTracker::current().map(|positions_tracker| {
            positions_tracker
                .maybe_last_token()
                .unwrap_or_else(|| positions_tracker.last_char())
        }),
    )
}

#[derive(Debug, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub location: Option<Location>,
}

impl LexError {
    pub fn new(message: String, location: Option<Location>) -> Self {
        Self { message, location }
    }
}

#[derive(Debug, PartialEq, Eq)]
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

#[derive(Debug, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub location: Option<Location>,
}

impl ParseError {
    pub fn new(message: String, location: Option<Location>) -> Self {
        Self { message, location }
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

    use indoc::indoc;

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
        lex_test("0.0", [Token::Float(0.0)]);
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
        lex_test("0", [Token::Int(0)]);
    }

    #[test]
    fn test_dot_dot_dot() {
        lex_test("...a", [Token::DotDotDot, Token::Name("a".to_owned())]);
    }

    fn lex_error_test(request: &str, expected_error_message: &str, location: Location) {
        assert_eq!(
            illicit::Layer::new()
                .offer(PositionsTracker::default())
                .enter(|| parse(request.chars()).unwrap_err()),
            ParseOrLexError::Lex(LexError::new(
                expected_error_message.to_owned(),
                Some(location)
            )),
        );
    }

    #[test]
    fn test_lex_unterminated_string() {
        lex_error_test(
            r#""abc"#,
            "expected closing double-quote",
            Location::new(1, 4),
        );
    }

    #[test]
    fn test_lex_unicode_escape_non_hex_digit() {
        lex_error_test(r#""\u123z""#, "Unexpected hex digit", Location::new(1, 7));
    }

    #[test]
    fn test_lex_leading_zero() {
        lex_error_test(r#"01"#, "Can't have leading zero", Location::new(1, 2));
        lex_error_test(r#"01.1"#, "Can't have leading zero", Location::new(1, 2));
    }

    #[test]
    fn test_lex_fractional_part() {
        lex_error_test(
            r#"1.e1"#,
            "expected fractional part digits",
            Location::new(1, 2),
        );
    }

    #[test]
    fn test_lex_exponent_digits() {
        lex_error_test(
            r#"1.0e name"#,
            "Expected exponent digits",
            Location::new(1, 4),
        );
        lex_error_test(
            r#"1e name"#,
            "Expected exponent digits",
            Location::new(1, 2),
        );
    }

    fn parse_error_test(request: &str, expected_error_message: &str, location: Location) {
        assert_eq!(
            illicit::Layer::new()
                .offer(PositionsTracker::default())
                .enter(|| parse(request.chars()).unwrap_err()),
            ParseOrLexError::Parse(ParseError::new(
                expected_error_message.to_owned(),
                Some(location)
            )),
        );
    }

    #[test]
    fn test_parse_query_selection_set() {
        parse_error_test(
            r#"query abc 1"#,
            "Expected selection set",
            Location::new(1, 11),
        );
        parse_error_test(r#"query 1"#, "Expected query", Location::new(1, 7));
    }

    #[test]
    fn test_parse_fragment_definition_name() {
        parse_error_test(
            r#"fragment on"#,
            "Saw `on` instead of fragment name",
            Location::new(1, 10),
        );
        parse_error_test(
            r#"fragment 1"#,
            "Expected fragment name",
            Location::new(1, 10),
        );
        parse_error_test(
            r#"fragment fooFragment on 1"#,
            "Expected fragment `on` name",
            Location::new(1, 25),
        );
        parse_error_test(
            r#"fragment fooFragment { name }"#,
            "Expected fragment `on`",
            Location::new(1, 22),
        );
        parse_error_test(
            r#"fragment fooFragment on Actor 1"#,
            "Expected selection set",
            Location::new(1, 31),
        );
    }

    #[test]
    fn test_parse_definition() {
        parse_error_test(
            indoc!(
                r#"
              {
                name
              }

              1
            "#
            ),
            "Expected definition",
            Location::new(5, 1),
        );
    }

    #[test]
    fn test_parse_inline_fragment_on() {
        parse_error_test(
            indoc!(
                r#"
              {
                ... on {
                  name
                }
              }
            "#
            ),
            "Expected on",
            Location::new(2, 10),
        );
        parse_error_test(
            indoc!(
                r#"
              {
                ... on Actor 1
              }
            "#
            ),
            "Expected selection set",
            Location::new(2, 16),
        );
        parse_error_test(
            indoc!(
                r#"
              {
                ...1
              }
            "#
            ),
            "Expected fragment selection",
            Location::new(2, 6),
        );
    }

    #[test]
    fn test_parse_argument() {
        parse_error_test(
            indoc!(
                r#"
              {
                actor(id 1) {
                  name
                }
              }
            "#
            ),
            "Expected colon",
            Location::new(2, 12),
        );
        parse_error_test(
            indoc!(
                r#"
              {
                actor() {
                  name
                }
              }
            "#
            ),
            "Empty arguments",
            Location::new(2, 9),
        );
        parse_error_test(
            indoc!(
                r#"
              {
                actor(1) {
                  name
                }
              }
            "#
            ),
            "Expected argument",
            Location::new(2, 9),
        );
    }

    #[test]
    fn test_parse_selection_set() {
        parse_error_test(
            indoc!(
                r#"
              {
                actorKatie { }
              }
            "#
            ),
            "Empty selection",
            Location::new(2, 16),
        );
    }

    #[test]
    fn test_parse_value() {
        parse_error_test(
            indoc!(
                r#"
              {
                actor(id: !) {
                  name
                }
              }
            "#
            ),
            "Expected value",
            Location::new(2, 13),
        );
    }
}
