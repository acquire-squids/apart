use crate::{Span, Spanned};

use std::{error, fmt};

pub struct Lexer {
    source: String,
    source_id: usize,
    byte_offset: usize,
    start_byte_offset: usize,
    lookahead: Option<<Self as Iterator>::Item>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Token {
    Integer(u32),
    Float(u32),
    OpenParenthesis,
    CloseParenthesis,
    OpenSquareBracket,
    CloseSquareBracket,
    OpenBracket,
    CloseBracket,
    Dot,
    Star,
    Slash,
    Percent,
    Plus,
    Minus,
    Identifier,
    Equal,
    Semicolon,
    Comma,
    Bang,
    Less,
    Greater,
    Ampersand,
    Pipe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Error {
    UnexpectedCharacter,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedCharacter => write!(f, "unexpected character in input"),
        }
    }
}

impl error::Error for Error {}

impl Lexer {
    #[must_use]
    pub const fn new(source_id: usize) -> Self {
        Self {
            source: String::new(),
            source_id,
            byte_offset: 0,
            start_byte_offset: 0,
            lookahead: None,
        }
    }

    pub fn push_source(&mut self, source: &str) {
        self.source.push_str(source);
    }

    pub const fn source_id(&self) -> usize {
        self.source_id
    }

    pub const fn source(&self) -> &str {
        self.source.as_str()
    }

    pub fn peek(&mut self) -> Option<&<Self as Iterator>::Item> {
        if self.lookahead.is_none() {
            self.lookahead = self.next();
        }

        self.lookahead.as_ref()
    }

    pub const fn restore(&mut self, from: Span) {
        self.byte_offset = from.start();

        self.lookahead = None;
    }

    const fn at(&self) -> usize {
        self.byte_offset
    }

    const fn start(&self) -> usize {
        self.start_byte_offset
    }

    fn ahead(&self) -> Option<char> {
        self.source
            .get((self.at())..)
            .and_then(|text| text.chars().next())
    }

    fn advance(&mut self) {
        self.byte_offset = self.source.ceil_char_boundary(self.at() + 1);
    }

    fn peek_char(&self, distance: usize) -> Option<char> {
        self.source
            .get((self.at())..)
            .and_then(|text| text.chars().nth(distance))
    }

    const fn single_char_token(&self, kind: Token) -> Spanned<Token> {
        Spanned::new(kind, Span::new(self.source_id(), self.start(), self.at()))
    }

    fn int_or_float(&mut self, radix: u32) -> Spanned<Token> {
        while let Some(n) = self.ahead()
            && n.is_digit(radix)
        {
            self.advance();
        }

        if self.ahead() == Some('.')
            && let Some(n) = self.peek_char(1)
            && n.is_digit(radix)
        {
            self.advance();
            self.advance();

            while let Some(n) = self.ahead()
                && n.is_digit(radix)
            {
                self.advance();
            }

            Spanned::new(
                Token::Float(radix),
                Span::new(self.source_id(), self.start(), self.at()),
            )
        } else {
            Spanned::new(
                Token::Integer(radix),
                Span::new(self.source_id(), self.start(), self.at()),
            )
        }
    }
}

impl Iterator for Lexer {
    type Item = Result<Spanned<Token>, Spanned<Error>>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(lookahead) = self.lookahead.take() {
            return Some(lookahead);
        }

        while let Some(ch) = self.ahead() {
            self.start_byte_offset = self.at();

            self.advance();

            match ch {
                _ if ch.is_ascii_whitespace() => {}
                '.' => return Some(Ok(self.single_char_token(Token::Dot))),
                '*' => return Some(Ok(self.single_char_token(Token::Star))),
                '/' => return Some(Ok(self.single_char_token(Token::Slash))),
                '%' => return Some(Ok(self.single_char_token(Token::Percent))),
                '+' => return Some(Ok(self.single_char_token(Token::Plus))),
                '-' => return Some(Ok(self.single_char_token(Token::Minus))),
                '(' => return Some(Ok(self.single_char_token(Token::OpenParenthesis))),
                ')' => return Some(Ok(self.single_char_token(Token::CloseParenthesis))),
                '[' => return Some(Ok(self.single_char_token(Token::OpenSquareBracket))),
                ']' => return Some(Ok(self.single_char_token(Token::CloseSquareBracket))),
                '{' => return Some(Ok(self.single_char_token(Token::OpenBracket))),
                '}' => return Some(Ok(self.single_char_token(Token::CloseBracket))),
                '=' => return Some(Ok(self.single_char_token(Token::Equal))),
                ';' => return Some(Ok(self.single_char_token(Token::Semicolon))),
                ',' => return Some(Ok(self.single_char_token(Token::Comma))),
                '!' => return Some(Ok(self.single_char_token(Token::Bang))),
                '<' => return Some(Ok(self.single_char_token(Token::Less))),
                '>' => return Some(Ok(self.single_char_token(Token::Greater))),
                '&' => return Some(Ok(self.single_char_token(Token::Ampersand))),
                '|' => return Some(Ok(self.single_char_token(Token::Pipe))),
                '0' if self.peek_char(0) == Some('x') => {
                    self.advance();

                    self.start_byte_offset = self.at();

                    return Some(Ok(self.int_or_float(16)));
                }
                '0' if self.peek_char(0) == Some('o') => {
                    self.advance();

                    self.start_byte_offset = self.at();

                    return Some(Ok(self.int_or_float(8)));
                }
                '0' if self.peek_char(0) == Some('b') => {
                    self.advance();

                    self.start_byte_offset = self.at();

                    return Some(Ok(self.int_or_float(2)));
                }
                '0' if self.peek_char(0) == Some('s') => {
                    self.advance();

                    self.start_byte_offset = self.at();

                    return Some(Ok(self.int_or_float(12)));
                }
                _ if ch.is_ascii_digit() => {
                    return Some(Ok(self.int_or_float(10)));
                }
                _ if ch.is_ascii_alphabetic() || ch == '_' => {
                    while let Some(n) = self.ahead()
                        && (n.is_ascii_alphanumeric() || n == '_')
                    {
                        self.advance();
                    }

                    return Some(Ok(Spanned::new(
                        Token::Identifier,
                        Span::new(self.source_id(), self.start(), self.at()),
                    )));
                }
                _ => {
                    return Some(Err(Spanned::new(
                        Error::UnexpectedCharacter,
                        Span::new(self.source_id(), self.start(), self.at()),
                    )));
                }
            }
        }

        None
    }
}
