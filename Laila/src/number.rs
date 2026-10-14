use crate::{Span, Token, TokenKind};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumRepresentation {
    Int,
    Long,
    Float,
    Double,
}

impl NumRepresentation {
    pub const fn c_type(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Long => "long long",
            Self::Float => "float",
            Self::Double => "double",
        }
    }

    pub const fn bytes(self) -> usize {
        match self {
            Self::Int => 4,
            Self::Long => 8,
            Self::Float => 4,
            Self::Double => 8,
        }
    }

    pub const fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Int, Self::Int) => Self::Int,
            (Self::Int, Self::Long) | (Self::Long, Self::Int) | (Self::Long, Self::Long) => Self::Long,
            (Self::Float, Self::Float) => Self::Float,
            (Self::Int, Self::Float) | (Self::Float, Self::Int) => Self::Float,
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
            literals[0].infer().unwrap_or(NumRepresentation::Int)
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
        let c_literal = match representation {
            NumRepresentation::Int => literal.normalized.clone(),
            NumRepresentation::Long => format!("{}LL", literal.normalized),
            NumRepresentation::Float => {
                let mut s = literal.normalized.clone();
                if literal.integral {
                    s.push_str(".0");
                }
                s.push('f');
                s
            }
            NumRepresentation::Double => {
                let mut s = literal.normalized.clone();
                if literal.integral {
                    s.push_str(".0");
                }
                s
            }
        };
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
            TokenKind::NumInt => {
                let clean = token.lexeme.replace('.', "");
                if !digits(&clean) {
                    return Err(error(NumErrorKind::InvalidLiteral));
                }
            }
            TokenKind::NumDec => {
                let (whole, fraction) = token
                    .lexeme
                    .split_once(',')
                    .ok_or_else(|| error(NumErrorKind::InvalidLiteral))?;
                let clean_whole = whole.replace('.', "");
                if !digits(&clean_whole) || !digits(fraction) {
                    return Err(error(NumErrorKind::InvalidLiteral));
                }
            }
            _ => return Err(error(NumErrorKind::ExpectedNumericLiteral)),
        }
        let clean = token.lexeme.replace('.', "");
        let normalized = normalized_decimal(&clean.replace(',', "."));
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
        if self.integral {
            if let Ok(val) = self.normalized.parse::<i64>() {
                if val >= i32::MIN as i64 && val <= i32::MAX as i64 {
                    return Ok(NumRepresentation::Int);
                } else {
                    return Ok(NumRepresentation::Long);
                }
            } else if self.normalized.parse::<u64>().is_ok() {
                return Ok(NumRepresentation::Long);
            } else {
                return Err(self.error(NumErrorKind::Overflow));
            }
        }

        let small = self
            .normalized
            .parse::<f32>()
            .map_err(|_| self.error(NumErrorKind::InvalidLiteral))?;
        if small.is_finite() && (small != 0.0 || self.normalized == "0") {
            let admissible = normalized_decimal(&small.to_string()) == self.normalized;
            if admissible {
                return Ok(NumRepresentation::Float);
            }
        }
        let large = self.double()?;
        if !large.is_finite() {
            return Err(self.error(NumErrorKind::Overflow));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenize;

    #[test]
    fn test_inteiro_4_bytes() {
        let tokens = tokenize("40").expect("falha ao tokenizar");
        let compiler = NumCompiler::new();
        let compiled = compiler.lower_literal(&tokens[0]).expect("falha ao compilar");
        assert_eq!(compiled.representation, NumRepresentation::Int);
        assert_eq!(compiled.representation.bytes(), 4);
        assert_eq!(compiled.representation.c_type(), "int");
        assert_eq!(compiled.c_literal, "40");
    }

    #[test]
    fn test_decimal_float_4_bytes() {
        let tokens = tokenize("8,50").expect("falha ao tokenizar");
        let compiler = NumCompiler::new();
        let compiled = compiler.lower_literal(&tokens[0]).expect("falha ao compilar");
        assert_eq!(compiled.representation, NumRepresentation::Float);
        assert_eq!(compiled.representation.bytes(), 4);
        assert_eq!(compiled.representation.c_type(), "float");
        assert_eq!(compiled.c_literal, "8.5f");
    }

    #[test]
    fn test_decimal_double_8_bytes() {
        let tokens = tokenize("3,141592653589793238").expect("falha ao tokenizar");
        let compiler = NumCompiler::new();
        let compiled = compiler.lower_literal(&tokens[0]).expect("falha ao compilar");
        assert_eq!(compiled.representation, NumRepresentation::Double);
        assert_eq!(compiled.representation.bytes(), 8);
        assert_eq!(compiled.representation.c_type(), "double");
    }

    #[test]
    fn test_milhar_com_ponto_inteiro() {
        let tokens = tokenize("1.000").expect("falha ao tokenizar");
        assert_eq!(tokens[0].kind, TokenKind::NumInt);
        let compiler = NumCompiler::new();
        let compiled = compiler.lower_literal(&tokens[0]).expect("falha ao compilar");
        assert_eq!(compiled.representation, NumRepresentation::Int);
        assert_eq!(compiled.representation.bytes(), 4);
        assert_eq!(compiled.c_literal, "1000");
    }

    #[test]
    fn test_milhar_com_ponto_e_virgula_decimal() {
        let tokens = tokenize("1.000,50").expect("falha ao tokenizar");
        assert_eq!(tokens[0].kind, TokenKind::NumDec);
        let compiler = NumCompiler::new();
        let compiled = compiler.lower_literal(&tokens[0]).expect("falha ao compilar");
        assert_eq!(compiled.representation, NumRepresentation::Float);
        assert_eq!(compiled.representation.bytes(), 4);
        assert_eq!(compiled.c_literal, "1000.5f");
    }

    #[test]
    fn test_ponto_invalido_como_decimal() {
        let res = tokenize("3.14");
        assert!(res.is_err());
    }

    #[test]
    fn test_join_promocao() {
        assert_eq!(
            NumRepresentation::Int.join(NumRepresentation::Int),
            NumRepresentation::Int
        );
        assert_eq!(
            NumRepresentation::Int.join(NumRepresentation::Float),
            NumRepresentation::Float
        );
        assert_eq!(
            NumRepresentation::Float.join(NumRepresentation::Double),
            NumRepresentation::Double
        );
    }
}
