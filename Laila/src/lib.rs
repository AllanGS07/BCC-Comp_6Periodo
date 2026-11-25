pub mod ast;
pub mod codegen_c;
pub mod interpreter;
pub mod ir;
mod lexer;
pub mod number;
pub mod optimizer;
pub mod parser;
pub mod semantic;
pub mod symbol_table;
mod token;

pub use ast::*;
pub use codegen_c::{compile_c_file, generate_c};
pub use interpreter::{
    interpret_ir, interpret_ir_with_io, ExecutionStats, RuntimeError, RuntimeValue,
};
pub use ir::{
    build_ir, IrBinOp, IrBuilder, IrConst, IrFunction, IrInstruction, IrOperand, IrProgram, IrType,
    IrUnOp, IrVar,
};
pub use lexer::{tokenize, LexError, LexErrorKind, Lexer};
pub use optimizer::{optimize_ir, OptimizationReport};
pub use parser::{parse, ParseError, Parser};
pub use semantic::{analyze_semantics, SemanticAnalyzer, SemanticError, SemanticErrorKind};
pub use symbol_table::{
    ResolvedReference, Scope, ScopeId, SemanticType, Symbol, SymbolKind, SymbolTable,
};
pub use token::{Position, Span, Token, TokenKind};
