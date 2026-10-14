use crate::{Position, Span, Token, TokenKind};
use std::fmt;
use std::iter::FusedIterator;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexErrorKind {
    UnexpectedCharacter(char),
    UnterminatedText,
    MultipleDecimalCommas,
    InvalidNumericSuffix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub kind: LexErrorKind,
    pub span: Span,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: ", self.span.start.line, self.span.start.column)?;
        match self.kind {
            LexErrorKind::UnexpectedCharacter('.') => {
                f.write_str("ponto inválido: use vírgula em números decimais (ex.: 3,14)")
            }
            LexErrorKind::UnexpectedCharacter(c) => write!(f, "caractere inesperado {c:?}"),
            LexErrorKind::UnterminatedText => f.write_str("texto sem aspas de fechamento"),
            LexErrorKind::MultipleDecimalCommas => f.write_str(
                "número com mais de uma vírgula decimal; use espaço após a vírgula que separa elementos (ex.: [1, 2, 3])",
            ),
            LexErrorKind::InvalidNumericSuffix => {
                f.write_str("número seguido de identificador; expoentes e sufixos numéricos não são definidos em Laila")
            }
        }
    }
}

impl std::error::Error for LexError {}

pub struct Lexer<'src> {
    source: &'src str,
    position: Position,
    previous_was_cr: bool,
    finished: bool,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            position: Position {
                byte: 0,
                line: 1,
                column: 1,
            },
            previous_was_cr: false,
            finished: false,
        }
    }

    fn peek(&self) -> Option<char> {
        self.source[self.position.byte..].chars().next()
    }

    fn peek_second(&self) -> Option<char> {
        self.source[self.position.byte..].chars().nth(1)
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.position.byte += c.len_utf8();
        match c {
            '\r' => {
                self.position.line += 1;
                self.position.column = 1;
            }
            '\n' => {
                if !self.previous_was_cr {
                    self.position.line += 1;
                }
                self.position.column = 1;
            }
            _ => self.position.column += 1,
        }
        self.previous_was_cr = c == '\r';
        Some(c)
    }

    fn take(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.advance();
        }
    }

    fn digits(&mut self) {
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.advance();
        }
    }

    fn decimal_comma(&self) -> bool {
        self.peek() == Some(',') && self.peek_second().is_some_and(|c| c.is_ascii_digit())
    }

    fn thousand_dot(&self) -> bool {
        if self.peek() != Some('.') {
            return false;
        }
        let rest = &self.source[self.position.byte + 1..];
        let mut chars = rest.chars();
        let d1 = chars.next();
        let d2 = chars.next();
        let d3 = chars.next();
        if !d1.is_some_and(|c| c.is_ascii_digit())
            || !d2.is_some_and(|c| c.is_ascii_digit())
            || !d3.is_some_and(|c| c.is_ascii_digit())
        {
            return false;
        }
        let d4 = chars.next();
        if d4.is_some_and(|c| c.is_ascii_digit()) {
            return false;
        }
        true
    }

    fn identifier_tail(&mut self) {
        while self.peek().is_some_and(is_identifier_continue) {
            self.advance();
        }
    }

    fn number(&mut self) -> Result<TokenKind, LexErrorKind> {
        self.digits();
        while self.thousand_dot() {
            self.advance();
            self.advance();
            self.advance();
            self.advance();
        }

        let kind = if self.decimal_comma() {
            self.advance();
            self.digits();
            TokenKind::NumDec
        } else {
            TokenKind::NumInt
        };

        if self.decimal_comma() {
            while self.decimal_comma() {
                self.advance();
                self.digits();
            }
            self.identifier_tail();
            return Err(LexErrorKind::MultipleDecimalCommas);
        }
        if self.peek().is_some_and(is_identifier_start) {
            self.identifier_tail();
            return Err(LexErrorKind::InvalidNumericSuffix);
        }
        Ok(kind)
    }

    fn text(&mut self) -> Result<TokenKind, LexErrorKind> {
        self.advance(); // Aspas de abertura.
        while let Some(c) = self.advance() {
            if c == '"' {
                return Ok(TokenKind::Texto);
            }
        }
        Err(LexErrorKind::UnterminatedText)
    }

    fn scan(&mut self) -> Result<TokenKind, LexErrorKind> {
        let Some(c) = self.peek() else {
            return Ok(TokenKind::Eof);
        };
        if is_identifier_start(c) {
            let start = self.position.byte;
            self.identifier_tail();
            return Ok(TokenKind::keyword(&self.source[start..self.position.byte])
                .unwrap_or(TokenKind::Id));
        }
        if c.is_ascii_digit() {
            return self.number();
        }
        if c == '"' {
            return self.text();
        }

        self.advance();
        Ok(match c {
            '=' => {
                if self.take('=') {
                    TokenKind::Igualdade
                } else {
                    TokenKind::Atrib
                }
            }
            '!' => {
                if self.take('=') {
                    TokenKind::Diferente
                } else {
                    TokenKind::Nao
                }
            }
            '>' => {
                if self.take('=') {
                    TokenKind::MaiorIgual
                } else {
                    TokenKind::MaiorQue
                }
            }
            '<' => {
                if self.take('=') {
                    TokenKind::MenorIgual
                } else {
                    TokenKind::MenorQue
                }
            }
            '&' if self.take('&') => TokenKind::E,
            '|' if self.take('|') => TokenKind::Ou,
            '+' => TokenKind::Soma,
            '-' => TokenKind::Subtracao,
            '*' => TokenKind::Multiplica,
            '/' => TokenKind::Divisao,
            '%' => TokenKind::Modulo,
            ';' => TokenKind::PontoVirg,
            '{' => TokenKind::AbreChaves,
            '}' => TokenKind::FechaChaves,
            '(' => TokenKind::AbrePar,
            ')' => TokenKind::FechaPar,
            '[' => TokenKind::AbreColch,
            ']' => TokenKind::FechaColch,
            ',' => TokenKind::Virgula,
            _ => return Err(LexErrorKind::UnexpectedCharacter(c)),
        })
    }
}

impl<'src> Iterator for Lexer<'src> {
    type Item = Result<Token<'src>, LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        self.skip_whitespace();
        let start = self.position;
        let result = self.scan();
        let span = Span {
            start,
            end: self.position,
        };
        Some(match result {
            Ok(kind) => {
                self.finished = kind == TokenKind::Eof;
                Ok(Token {
                    kind,
                    lexeme: &self.source[start.byte..self.position.byte],
                    span,
                })
            }
            Err(kind) => Err(LexError { kind, span }),
        })
    }
}

impl FusedIterator for Lexer<'_> {}

fn is_identifier_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_identifier_continue(c: char) -> bool {
    is_identifier_start(c) || c.is_ascii_digit()
}

/// Retorna os tokens e TK_EOF, ou o primeiro erro léxico.
pub fn tokenize(source: &str) -> Result<Vec<Token<'_>>, LexError> {
    Lexer::new(source).collect()
}
