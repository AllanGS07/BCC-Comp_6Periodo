use crate::{Span, Token, TokenKind};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumRepresentation {
    Float,
    Double,
}

impl NumRepresentation {
    pub const fn c_type(self) -> &'static str {
        match self {
            Self::Float => "float",
            Self::Double => "double",
        }
    }

    pub const fn bytes(self) -> usize {
        match self {
            Self::Float => 4,
            Self::Double => 8,
        }
    }

    pub const fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Float, Self::Float) => Self::Float,
            _ => Self::Double,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralPrecision {
    ExactInteger,
    DecimalRoundTrip,
    RoundedToDouble,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledNumber {
    pub representation: NumRepresentation,
    pub c_literal: String,
    pub precision: LiteralPrecision,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedNum {
    pub representation: NumRepresentation,
    pub constants: Vec<CompiledNumber>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumErrorKind {
    ExpectedNumericLiteral,
    InvalidLiteral,
    Overflow,
    UnderflowToZero,
    IntegerPrecisionLoss,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumError {
    pub kind: NumErrorKind,
    pub span: Span,
}

impl fmt::Display for NumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: ", self.span.start.line, self.span.start.column)?;
        f.write_str(match self.kind {
            NumErrorKind::ExpectedNumericLiteral => "esperado TK_NUM_INT ou TK_NUM_DEC",
            NumErrorKind::InvalidLiteral => "literal numérico inválido",
            NumErrorKind::Overflow => "literal excede o alcance de double (8 bytes)",
            NumErrorKind::UnderflowToZero => "literal não nulo seria arredondado para zero até em double",
            NumErrorKind::IntegerPrecisionLoss => "valor inteiro não é representável exatamente em float ou double; seria necessária precisão arbitrária",
        })
    }
}

impl std::error::Error for NumError {}

#[derive(Debug, Clone, Copy, Default)]
pub struct NumCompiler;

impl NumCompiler {
    pub const fn new() -> Self {
        Self
    }

    pub fn lower_literal(&self, token: &Token<'_>) -> Result<CompiledNumber, NumError> {
        let literal = NumericLiteral::parse(token)?;
        self.compile(&literal, literal.infer()?)
    }

    pub fn resolve_binding(
        &self,
        tokens: &[Token<'_>],
        has_nonliteral_values: bool,
    ) -> Result<ResolvedNum, NumError> {
        let literals = tokens
            .iter()
            .map(NumericLiteral::parse)
            .collect::<Result<Vec<_>, _>>()?;
        let mut representation = if has_nonliteral_values || tokens.is_empty() {
            NumRepresentation::Double
        } else {
            NumRepresentation::Float
        };
        for literal in &literals {
            representation = representation.join(literal.infer()?);
        }
        let constants = literals
            .iter()
            .map(|literal| self.compile(literal, representation))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ResolvedNum {
            representation,
            constants,
        })
    }

    fn compile(
        &self,
        literal: &NumericLiteral,
        representation: NumRepresentation,
    ) -> Result<CompiledNumber, NumError> {
        let precision = if literal.integral {
            LiteralPrecision::ExactInteger
        } else if representation == NumRepresentation::Float
            || normalized_decimal(&literal.double()?.to_string()) == literal.normalized
        {
            LiteralPrecision::DecimalRoundTrip
        } else {
            LiteralPrecision::RoundedToDouble
        };
        let mut c_literal = literal.normalized.clone();
        if literal.integral {
            c_literal.push_str(".0");
        }
        if representation == NumRepresentation::Float {
            c_literal.push('f');
        }
        Ok(CompiledNumber {
            representation,
            c_literal,
            precision,
            span: literal.span,
        })
    }
}

struct NumericLiteral {
    normalized: String,
    integral: bool,
    span: Span,
}

impl NumericLiteral {
    fn error(&self, kind: NumErrorKind) -> NumError {
        NumError {
            kind,
            span: self.span,
        }
    }

    fn parse(token: &Token<'_>) -> Result<Self, NumError> {
        let error = |kind| NumError {
            kind,
            span: token.span,
        };
        let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        match token.kind {
            TokenKind::NumInt if digits(token.lexeme) => {}
            TokenKind::NumDec => {
                let (whole, fraction) = token
                    .lexeme
                    .split_once(',')
                    .ok_or_else(|| error(NumErrorKind::InvalidLiteral))?;
                if !digits(whole) || !digits(fraction) {
                    return Err(error(NumErrorKind::InvalidLiteral));
                }
            }
            TokenKind::NumInt => return Err(error(NumErrorKind::InvalidLiteral)),
            _ => return Err(error(NumErrorKind::ExpectedNumericLiteral)),
        }
        let normalized = normalized_decimal(&token.lexeme.replace(',', "."));
        let integral = !normalized.contains('.');
        Ok(Self {
            normalized,
            integral,
            span: token.span,
        })
    }

    fn double(&self) -> Result<f64, NumError> {
        let value = self
            .normalized
            .parse::<f64>()
            .map_err(|_| self.error(NumErrorKind::InvalidLiteral))?;
        if !value.is_finite() {
            return Err(self.error(NumErrorKind::Overflow));
        }
        if value == 0.0 && self.normalized != "0" {
            return Err(self.error(NumErrorKind::UnderflowToZero));
        }
        Ok(value)
    }

    fn infer(&self) -> Result<NumRepresentation, NumError> {
        let small = self
            .normalized
            .parse::<f32>()
            .map_err(|_| self.error(NumErrorKind::InvalidLiteral))?;
        if small.is_finite() && (small != 0.0 || self.normalized == "0") {
            let admissible = if self.integral {
                format!("{small:.0}") == self.normalized
            } else {
                normalized_decimal(&small.to_string()) == self.normalized
            };
            if admissible {
                return Ok(NumRepresentation::Float);
            }
        }
        let large = self.double()?;
        if self.integral && format!("{large:.0}") != self.normalized {
            return Err(self.error(NumErrorKind::IntegerPrecisionLoss));
        }
        Ok(NumRepresentation::Double)
    }
}

fn normalized_decimal(source: &str) -> String {
    let (whole, fraction) = source.split_once('.').unwrap_or((source, ""));
    let whole = whole.trim_start_matches('0');
    let whole = if whole.is_empty() { "0" } else { whole };
    let fraction = fraction.trim_end_matches('0');
    if fraction.is_empty() {
        whole.to_owned()
    } else {
        format!("{whole}.{fraction}")
    }
}
