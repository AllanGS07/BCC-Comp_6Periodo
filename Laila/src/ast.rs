use crate::{Span, TokenKind};
use std::fmt;

/// Representa o programa completo em Laila (uma coleção de funções ou declarações/comandos).
#[derive(Debug, Clone, PartialEq)]
pub struct Program<'src> {
    pub items: Vec<ProgramItem<'src>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProgramItem<'src> {
    Function(Function<'src>),
    Statement(Statement<'src>),
}

/// Representa uma função na linguagem Laila.
/// Ex.: vazio principal() { ... }
#[derive(Debug, Clone, PartialEq)]
pub struct Function<'src> {
    pub return_type: Type,
    pub name: &'src str,
    pub params: Vec<Param<'src>>,
    pub body: Block<'src>,
    pub span: Span,
}

/// Parâmetro formal de uma função.
/// Ex.: int a
#[derive(Debug, Clone, PartialEq)]
pub struct Param<'src> {
    pub param_type: Type,
    pub name: &'src str,
    pub span: Span,
}

/// Tipos primitivos suportados pela linguagem Laila.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Dec,
    Carac,
    Num,
    Vazio,
}

impl Type {
    pub fn from_token_kind(kind: TokenKind) -> Option<Self> {
        match kind {
            TokenKind::TipoInt => Some(Self::Int),
            TokenKind::TipoDec => Some(Self::Dec),
            TokenKind::TipoCarac => Some(Self::Carac),
            TokenKind::TipoNum => Some(Self::Num),
            TokenKind::TipoVazio => Some(Self::Vazio),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Dec => "dec",
            Self::Carac => "carac",
            Self::Num => "num",
            Self::Vazio => "vazio",
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Bloco de comandos delimitado por chaves `{ ... }`.
#[derive(Debug, Clone, PartialEq)]
pub struct Block<'src> {
    pub statements: Vec<Statement<'src>>,
    pub span: Span,
}

/// Comandos suportados pela linguagem Laila.
#[derive(Debug, Clone, PartialEq)]
pub enum Statement<'src> {
    VarDecl(VarDecl<'src>),
    Assignment(Assignment<'src>),
    If(IfStmt<'src>),
    While(WhileStmt<'src>),
    For(ForStmt<'src>),
    Print(PrintStmt<'src>),
    Read(ReadStmt<'src>),
    Return(ReturnStmt<'src>),
    Break(Span),
    Continue(Span),
    FunctionCall(FunctionCall<'src>),
    Block(Block<'src>),
}

/// Declaração de variável com inicialização opcional.
/// Ex.: dec nota_final = 8,50; ou int i;
#[derive(Debug, Clone, PartialEq)]
pub struct VarDecl<'src> {
    pub var_type: Type,
    pub name: &'src str,
    pub init: Option<Expr<'src>>,
    pub span: Span,
}

/// Atribuição de valor a uma variável.
/// Ex.: taxa = 40;
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment<'src> {
    pub name: &'src str,
    pub value: Expr<'src>,
    pub span: Span,
}

/// Estrutura condicional simples ou composta (`se` / `senao`).
#[derive(Debug, Clone, PartialEq)]
pub struct IfStmt<'src> {
    pub condition: Expr<'src>,
    pub then_branch: Block<'src>,
    pub else_branch: Option<Block<'src>>,
    pub span: Span,
}

/// Laço de repetição condicional (`enquanto`).
#[derive(Debug, Clone, PartialEq)]
pub struct WhileStmt<'src> {
    pub condition: Expr<'src>,
    pub body: Block<'src>,
    pub span: Span,
}

/// Inicializador do laço `para` (atribuição simples ou declaração).
#[derive(Debug, Clone, PartialEq)]
pub enum ForInit<'src> {
    Assignment(Assignment<'src>),
    VarDecl(VarDecl<'src>),
}

/// Laço de repetição determinado (`para`).
#[derive(Debug, Clone, PartialEq)]
pub struct ForStmt<'src> {
    pub init: Option<ForInit<'src>>,
    pub condition: Option<Expr<'src>>,
    pub increment: Option<Assignment<'src>>,
    pub body: Block<'src>,
    pub span: Span,
}

/// Comando de saída (`imprima`).
#[derive(Debug, Clone, PartialEq)]
pub struct PrintStmt<'src> {
    pub args: Vec<Expr<'src>>,
    pub span: Span,
}

/// Comando de entrada (`leia`).
#[derive(Debug, Clone, PartialEq)]
pub struct ReadStmt<'src> {
    pub var_name: &'src str,
    pub span: Span,
}

/// Comando de retorno (`retorna`).
#[derive(Debug, Clone, PartialEq)]
pub struct ReturnStmt<'src> {
    pub value: Option<Expr<'src>>,
    pub span: Span,
}

/// Chamada de função (usada como comando ou dentro de expressões).
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionCall<'src> {
    pub callee: &'src str,
    pub args: Vec<Expr<'src>>,
    pub span: Span,
}

/// Operadores binários (aritméticos, relacionais e lógicos).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    // Aritméticos
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    // Relacionais
    Equal,
    NotEqual,
    LessThan,
    GreaterThan,
    LessEqual,
    GreaterEqual,
    // Lógicos
    And,
    Or,
}

impl BinaryOp {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Mod => "%",
            Self::Equal => "==",
            Self::NotEqual => "!=",
            Self::LessThan => "<",
            Self::GreaterThan => ">",
            Self::LessEqual => "<=",
            Self::GreaterEqual => ">=",
            Self::And => "&&",
            Self::Or => "||",
        }
    }
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Operadores unários.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

impl UnaryOp {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Neg => "-",
            Self::Not => "!",
        }
    }
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Expressões na linguagem Laila.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr<'src> {
    Identifier {
        name: &'src str,
        span: Span,
    },
    IntLiteral {
        raw: &'src str,
        span: Span,
    },
    DecLiteral {
        raw: &'src str,
        span: Span,
    },
    StringLiteral {
        content: &'src str,
        span: Span,
    },
    BoolLiteral {
        value: bool,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr<'src>>,
        span: Span,
    },
    Binary {
        left: Box<Expr<'src>>,
        op: BinaryOp,
        right: Box<Expr<'src>>,
        span: Span,
    },
    Call(FunctionCall<'src>),
    Grouping {
        expr: Box<Expr<'src>>,
        span: Span,
    },
}

impl<'src> Expr<'src> {
    pub fn span(&self) -> Span {
        match self {
            Self::Identifier { span, .. }
            | Self::IntLiteral { span, .. }
            | Self::DecLiteral { span, .. }
            | Self::StringLiteral { span, .. }
            | Self::BoolLiteral { span, .. }
            | Self::Unary { span, .. }
            | Self::Binary { span, .. }
            | Self::Grouping { span, .. } => *span,
            Self::Call(call) => call.span,
        }
    }
}
