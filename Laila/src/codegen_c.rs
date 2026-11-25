use crate::ast::*;
use crate::ir::IrType;
use crate::number::{NumCompiler, NumRepresentation};
use crate::symbol_table::{ScopeId, SemanticType, SymbolTable};
use crate::{Token, TokenKind};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

/// Gera código-fonte C11 limpo e compilável a partir da AST e da Tabela de Símbolos validadas.
pub fn generate_c(program: &Program<'_>, table: &SymbolTable) -> String {
    CGenerator::new(table).generate(program)
}

/// Compila código-fonte C para um arquivo executável utilizando `gcc` (ou `cc`).
pub fn compile_c_file(c_path: &Path, exe_path: &Path) -> Result<(), String> {
    let compiler = if Command::new("gcc").arg("--version").output().is_ok() {
        "gcc"
    } else if Command::new("cc").arg("--version").output().is_ok() {
        "cc"
    } else {
        return Err(
            "compilador C ('gcc' ou 'cc') não encontrado no PATH do sistema".to_string(),
        );
    };

    let output = Command::new(compiler)
        .arg("-std=c11")
        .arg("-O2")
        .arg("-Wall")
        .arg("-Wextra")
        .arg(c_path)
        .arg("-o")
        .arg(exe_path)
        .output()
        .map_err(|e| format!("falha ao executar '{compiler}': {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "falha na compilação C ({compiler}):\n{}",
            stderr.trim()
        ));
    }

    Ok(())
}

struct CGenerator<'a> {
    table: &'a SymbolTable,
    out: String,
    indent: usize,
    func_param_kinds: HashMap<String, Vec<IrType>>,
}

impl<'a> CGenerator<'a> {
    fn new(table: &'a SymbolTable) -> Self {
        Self {
            table,
            out: String::new(),
            indent: 0,
            func_param_kinds: HashMap::new(),
        }
    }

    fn generate(mut self, program: &Program<'_>) -> String {
        self.out.push_str("/* Código C gerado automaticamente pelo compilador Laila */\n");
        self.out.push_str("#include <stdio.h>\n");
        self.out.push_str("#include <stdbool.h>\n");
        self.out.push_str("#include <string.h>\n\n");

        let mut top_decls = Vec::new();
        let mut top_stmts = Vec::new();
        let mut functions = Vec::new();
        let mut has_principal = false;

        for item in &program.items {
            match item {
                ProgramItem::Function(func) => {
                    if func.name == "principal" {
                        has_principal = true;
                    }
                    let fn_scope = self
                        .table
                        .scope_for_block(func.body.span)
                        .unwrap_or(ScopeId(0));
                    let p_kinds = func
                        .params
                        .iter()
                        .map(|p| {
                            self.table
                                .symbol(fn_scope, p.name)
                                .map(|s| IrType::from_c_layout(s.ty, s.c_type))
                                .unwrap_or_else(|| {
                                    IrType::from_c_layout(
                                        SemanticType::from_ast_type(p.param_type),
                                        "",
                                    )
                                })
                        })
                        .collect();
                    self.func_param_kinds.insert(func.name.to_string(), p_kinds);
                    functions.push(func);
                }
                ProgramItem::Statement(stmt) => match stmt.as_ref() {
                    Statement::VarDecl(decl) => top_decls.push(decl),
                    other => top_stmts.push(other),
                },
            }
        }

        // Variáveis globais (escopo B0)
        for decl in &top_decls {
            self.emit_var_decl(decl, true);
        }
        if !top_decls.is_empty() {
            self.out.push('\n');
        }

        // Protótipos de funções auxiliares (exceto principal -> main)
        let mut emitted_proto = false;
        for func in &functions {
            if func.name != "principal" {
                self.emit_function_signature(func);
                self.out.push_str(";\n");
                emitted_proto = true;
            }
        }
        if emitted_proto {
            self.out.push('\n');
        }

        // Definições das funções
        for (idx, func) in functions.iter().enumerate() {
            if func.name == "principal" {
                self.emit_principal(func, &top_stmts);
            } else {
                self.emit_function(func);
            }
            if idx + 1 < functions.len() {
                self.out.push('\n');
            }
        }

        // Caso não exista função `principal`, encapsula comandos de topo em `int main(void)`
        if !has_principal {
            if !functions.is_empty() {
                self.out.push('\n');
            }
            self.out.push_str("int main(void) {\n");
            self.indent = 1;
            for stmt in &top_stmts {
                self.emit_stmt(stmt, false, None);
            }
            self.push_indent();
            self.out.push_str("return 0;\n");
            self.indent = 0;
            self.out.push_str("}\n");
        }

        self.out
    }

    fn push_indent(&mut self) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
    }

    fn emit_function_signature(&mut self, func: &Function<'_>) {
        let fn_scope = self
            .table
            .scope_for_block(func.body.span)
            .unwrap_or(ScopeId(0));
        let ret_c = self
            .table
            .symbol(ScopeId(0), func.name)
            .map(|s| s.c_type)
            .unwrap_or_else(|| default_c_from_ast(func.return_type));

        let _ = write!(self.out, "{ret_c} {}(", c_ident(func.name));
        if func.params.is_empty() {
            self.out.push_str("void");
        } else {
            for (i, param) in func.params.iter().enumerate() {
                if i > 0 {
                    self.out.push_str(", ");
                }
                let p_c_type = self
                    .table
                    .symbol(fn_scope, param.name)
                    .map(|s| s.c_type)
                    .unwrap_or_else(|| default_c_from_ast(param.param_type));
                let _ = write!(self.out, "{p_c_type} {}", c_ident(param.name));
            }
        }
        self.out.push(')');
    }

    fn emit_function(&mut self, func: &Function<'_>) {
        let ret_kind = self
            .table
            .symbol(ScopeId(0), func.name)
            .map(|s| IrType::from_c_layout(s.ty, s.c_type));
        self.emit_function_signature(func);
        self.out.push_str(" {\n");
        self.indent += 1;
        for stmt in &func.body.statements {
            self.emit_stmt(stmt, false, ret_kind);
        }
        self.indent -= 1;
        self.out.push_str("}\n");
    }

    fn emit_principal(&mut self, func: &Function<'_>, top_stmts: &[&Statement<'_>]) {
        let ret_kind = self
            .table
            .symbol(ScopeId(0), func.name)
            .map(|s| IrType::from_c_layout(s.ty, s.c_type));
        self.out.push_str("int main(void) {\n");
        self.indent += 1;
        for stmt in top_stmts {
            self.emit_stmt(stmt, false, ret_kind);
        }
        let is_void = func.return_type == Type::Vazio;
        for stmt in &func.body.statements {
            self.emit_stmt(stmt, is_void, ret_kind);
        }
        if !matches!(func.body.statements.last(), Some(Statement::Return(_))) {
            self.push_indent();
            self.out.push_str("return 0;\n");
        }
        self.indent -= 1;
        self.out.push_str("}\n");
    }

    fn emit_block(&mut self, block: &Block<'_>, void_main: bool, ret_kind: Option<IrType>) {
        self.out.push_str("{\n");
        self.indent += 1;
        for stmt in &block.statements {
            self.emit_stmt(stmt, void_main, ret_kind);
        }
        self.indent -= 1;
        self.push_indent();
        self.out.push('}');
    }

    fn emit_stmt(&mut self, stmt: &Statement<'_>, void_main: bool, ret_kind: Option<IrType>) {
        match stmt {
            Statement::VarDecl(decl) => self.emit_var_decl(decl, false),
            Statement::Assignment(assign) => {
                self.push_indent();
                self.emit_assignment_inline(assign);
                self.out.push_str(";\n");
            }
            Statement::If(if_stmt) => {
                self.push_indent();
                let cond_str = self.format_expr(&if_stmt.condition, Some(IrType::Bool));
                let _ = write!(self.out, "if ({cond_str}) ");
                self.emit_block(&if_stmt.then_branch, void_main, ret_kind);
                if let Some(else_branch) = &if_stmt.else_branch {
                    self.out.push_str(" else ");
                    self.emit_block(else_branch, void_main, ret_kind);
                }
                self.out.push('\n');
            }
            Statement::While(while_stmt) => {
                self.push_indent();
                let cond_str = self.format_expr(&while_stmt.condition, Some(IrType::Bool));
                let _ = write!(self.out, "while ({cond_str}) ");
                self.emit_block(&while_stmt.body, void_main, ret_kind);
                self.out.push('\n');
            }
            Statement::For(for_stmt) => {
                self.push_indent();
                self.out.push_str("for (");
                if let Some(init) = &for_stmt.init {
                    match init {
                        ForInit::VarDecl(d) => self.emit_var_decl_inline(d),
                        ForInit::Assignment(a) => self.emit_assignment_inline(a),
                    }
                }
                self.out.push_str("; ");
                if let Some(cond) = &for_stmt.condition {
                    let cond_str = self.format_expr(cond, Some(IrType::Bool));
                    self.out.push_str(&cond_str);
                }
                self.out.push_str("; ");
                if let Some(inc) = &for_stmt.increment {
                    self.emit_assignment_inline(inc);
                }
                self.out.push_str(") ");
                self.emit_block(&for_stmt.body, void_main, ret_kind);
                self.out.push('\n');
            }
            Statement::Break(_) => {
                self.push_indent();
                self.out.push_str("break;\n");
            }
            Statement::Continue(_) => {
                self.push_indent();
                self.out.push_str("continue;\n");
            }
            Statement::Return(ret) => {
                self.push_indent();
                if void_main {
                    self.out.push_str("return 0;\n");
                } else if let Some(val) = &ret.value {
                    let val_str = self.format_expr(val, ret_kind);
                    let _ = writeln!(self.out, "return {val_str};");
                } else {
                    self.out.push_str("return;\n");
                }
            }
            Statement::Print(p) => self.emit_print(p),
            Statement::Read(r) => self.emit_read(r),
            Statement::FunctionCall(call) => {
                self.push_indent();
                let call_str = self.format_call(call);
                let _ = writeln!(self.out, "{call_str};");
            }
            Statement::Block(b) => {
                self.push_indent();
                self.emit_block(b, void_main, ret_kind);
                self.out.push('\n');
            }
        }
    }

    fn resolve_decl_layout(&self, decl: &VarDecl<'_>) -> (&'static str, IrType) {
        if let Some(scope_id) = self.table.scope_for_decl(decl.span)
            && let Some(sym) = self.table.symbol(scope_id, decl.name) {
                return (sym.c_type, IrType::from_c_layout(sym.ty, sym.c_type));
            }
        let c_type = default_c_from_ast(decl.var_type);
        (
            c_type,
            IrType::from_c_layout(SemanticType::from_ast_type(decl.var_type), c_type),
        )
    }

    fn emit_var_decl(&mut self, decl: &VarDecl<'_>, _is_global: bool) {
        self.push_indent();
        self.emit_var_decl_inline(decl);
        self.out.push_str(";\n");
    }

    fn emit_var_decl_inline(&mut self, decl: &VarDecl<'_>) {
        let (c_type, expr_kind) = self.resolve_decl_layout(decl);
        let name = c_ident(decl.name);

        if c_type == "char[256]" {
            if let Some(init) = &decl.init {
                let init_str = self.format_expr(init, Some(IrType::Texto));
                let _ = write!(self.out, "char {name}[256] = {init_str}");
            } else {
                let _ = write!(self.out, "char {name}[256] = \"\"");
            }
            return;
        }

        if let Some(init) = &decl.init {
            let init_str = self.format_expr(init, Some(expr_kind));
            let _ = write!(self.out, "{c_type} {name} = {init_str}");
        } else {
            let _ = write!(self.out, "{c_type} {name}");
        }
    }

    fn emit_assignment_inline(&mut self, assign: &Assignment<'_>) {
        let ref_info = self.table.find_reference_at(assign.span);
        let (c_type, expr_kind) = match ref_info {
            Some(r) => (r.c_type, IrType::from_c_layout(r.ty, r.c_type)),
            None => ("double", IrType::Double),
        };
        let name = c_ident(assign.name);

        if c_type == "char[256]" {
            let val_str = self.format_expr(&assign.value, Some(IrType::Texto));
            let _ = write!(self.out, "snprintf({name}, sizeof({name}), \"%s\", {val_str})");
        } else {
            let val_str = self.format_expr(&assign.value, Some(expr_kind));
            let _ = write!(self.out, "{name} = {val_str}");
        }
    }

    fn emit_print(&mut self, print_stmt: &PrintStmt<'_>) {
        self.push_indent();
        if print_stmt.args.is_empty() {
            self.out.push_str("printf(\"\\n\");\n");
            return;
        }

        let mut fmt_str = String::new();
        let mut c_args = Vec::with_capacity(print_stmt.args.len());

        for (idx, arg) in print_stmt.args.iter().enumerate() {
            if idx > 0 {
                fmt_str.push(' ');
            }
            let kind = self.infer_expr_kind(arg);
            let formatted = self.format_expr(arg, Some(kind));
            match kind {
                IrType::Int => {
                    fmt_str.push_str("%d");
                    c_args.push(formatted);
                }
                IrType::Long => {
                    fmt_str.push_str("%lld");
                    c_args.push(formatted);
                }
                IrType::Float => {
                    fmt_str.push_str("%.7g");
                    c_args.push(formatted);
                }
                IrType::Double => {
                    fmt_str.push_str("%.16g");
                    c_args.push(formatted);
                }
                IrType::Char => {
                    fmt_str.push_str("%c");
                    c_args.push(formatted);
                }
                IrType::Texto => {
                    fmt_str.push_str("%s");
                    c_args.push(formatted);
                }
                IrType::Bool => {
                    fmt_str.push_str("%s");
                    c_args.push(format!("(({formatted}) ? \"verdadeiro\" : \"falso\")"));
                }
                IrType::Vazio => {
                    fmt_str.push_str("%s");
                    c_args.push("\"vazio\"".to_string());
                }
            }
        }
        fmt_str.push_str("\\n");

        let _ = writeln!(self.out, "printf(\"{fmt_str}\", {});", c_args.join(", "));
    }

    fn emit_read(&mut self, read_stmt: &ReadStmt<'_>) {
        self.push_indent();
        let name = c_ident(read_stmt.var_name);
        let ref_info = self.table.find_reference_at(read_stmt.span);
        let (c_type, kind) = match ref_info {
            Some(r) => (r.c_type, IrType::from_c_layout(r.ty, r.c_type)),
            None => ("double", IrType::Double),
        };

        if c_type == "char[256]" {
            let _ = writeln!(self.out, "scanf(\"%255s\", {name});");
            return;
        }

        match kind {
            IrType::Int => {
                let _ = writeln!(self.out, "scanf(\"%d\", &{name});");
            }
            IrType::Long => {
                let _ = writeln!(self.out, "scanf(\"%lld\", &{name});");
            }
            IrType::Float => {
                let _ = writeln!(self.out, "scanf(\"%f\", &{name});");
            }
            IrType::Double => {
                let _ = writeln!(self.out, "scanf(\"%lf\", &{name});");
            }
            IrType::Char => {
                let _ = writeln!(self.out, "scanf(\" %c\", &{name});");
            }
            _ => {
                let _ = writeln!(self.out, "scanf(\"%d\", &{name});");
            }
        }
    }

    fn infer_expr_kind(&self, expr: &Expr<'_>) -> IrType {
        let compiler = NumCompiler::new();
        match expr {
            Expr::IntLiteral { raw, span } => {
                let tok = Token {
                    kind: TokenKind::NumInt,
                    lexeme: raw,
                    span: *span,
                };
                match compiler.lower_literal(&tok).map(|c| c.representation) {
                    Ok(NumRepresentation::Long) => IrType::Long,
                    _ => IrType::Int,
                }
            }
            Expr::DecLiteral { raw, span } => {
                let tok = Token {
                    kind: TokenKind::NumDec,
                    lexeme: raw,
                    span: *span,
                };
                match compiler.lower_literal(&tok).map(|c| c.representation) {
                    Ok(NumRepresentation::Float) => IrType::Float,
                    Ok(NumRepresentation::Int) => IrType::Float,
                    _ => IrType::Double,
                }
            }
            Expr::StringLiteral { content, .. } => {
                if content.chars().count() == 1 {
                    IrType::Char
                } else {
                    IrType::Texto
                }
            }
            Expr::BoolLiteral { .. } => IrType::Bool,
            Expr::Grouping { expr, .. } => self.infer_expr_kind(expr),
            Expr::Identifier { span, .. } => match self.table.find_reference_at(*span) {
                Some(r) => IrType::from_c_layout(r.ty, r.c_type),
                None => IrType::Double,
            },
            Expr::Call(call) => match self.table.find_reference_at(call.span) {
                Some(r) => IrType::from_c_layout(r.ty, r.c_type),
                None => IrType::Double,
            },
            Expr::Unary { op, operand, .. } => match op {
                UnaryOp::Neg => self.infer_expr_kind(operand),
                UnaryOp::Not => IrType::Bool,
            },
            Expr::Binary {
                left, op, right, ..
            } => match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                    let l = self.infer_expr_kind(left);
                    let r = self.infer_expr_kind(right);
                    l.unify_numeric(r)
                }
                _ => IrType::Bool,
            },
        }
    }

    fn format_call(&self, call: &FunctionCall<'_>) -> String {
        let param_kinds = self
            .func_param_kinds
            .get(call.callee)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        let args: Vec<String> = call
            .args
            .iter()
            .enumerate()
            .map(|(i, arg)| self.format_expr(arg, param_kinds.get(i).copied()))
            .collect();

        format!("{}({})", c_ident(call.callee), args.join(", "))
    }

    fn format_expr(&self, expr: &Expr<'_>, expected: Option<IrType>) -> String {
        let compiler = NumCompiler::new();
        match expr {
            Expr::Identifier { name, .. } => c_ident(name).to_string(),
            Expr::IntLiteral { raw, span } => {
                let tok = Token {
                    kind: TokenKind::NumInt,
                    lexeme: raw,
                    span: *span,
                };
                if let Ok(compiled) = compiler.lower_literal(&tok) {
                    match expected {
                        Some(IrType::Float) => {
                            let clean = raw.replace('.', "");
                            format!("{clean}.0f")
                        }
                        Some(IrType::Double) => {
                            let clean = raw.replace('.', "");
                            format!("{clean}.0")
                        }
                        Some(IrType::Long) if compiled.representation == NumRepresentation::Int => {
                            format!("{}LL", compiled.c_literal)
                        }
                        _ => compiled.c_literal,
                    }
                } else {
                    raw.replace('.', "")
                }
            }
            Expr::DecLiteral { raw, span } => {
                let tok = Token {
                    kind: TokenKind::NumDec,
                    lexeme: raw,
                    span: *span,
                };
                if let Ok(compiled) = compiler.lower_literal(&tok) {
                    let mut lit = compiled.c_literal;
                    if !lit.contains('.') {
                        lit.push_str(".0");
                        if expected == Some(IrType::Float)
                            || matches!(
                                compiled.representation,
                                NumRepresentation::Float | NumRepresentation::Int
                            )
                        {
                            lit.push('f');
                        }
                    } else if expected == Some(IrType::Double) && lit.ends_with('f') {
                        lit.pop();
                    } else if expected == Some(IrType::Float) && !lit.ends_with('f') {
                        lit.push('f');
                    }
                    lit
                } else {
                    raw.replace('.', "").replace(',', ".")
                }
            }
            Expr::StringLiteral { content, .. } => {
                if expected == Some(IrType::Char) {
                    let mut chars = content.chars();
                    if let (Some(c), None) = (chars.next(), chars.next()) {
                        return format!("'{}'", escape_c_char(c));
                    }
                }
                format!("\"{}\"", escape_c_str(content))
            }
            Expr::BoolLiteral { value, .. } => {
                if *value {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            Expr::Grouping { expr, .. } => format!("({})", self.format_expr(expr, expected)),
            Expr::Unary { op, operand, .. } => {
                let inner = match operand.as_ref() {
                    Expr::Binary { .. } | Expr::Unary { .. } => {
                        format!("({})", self.format_expr(operand, expected))
                    }
                    _ => self.format_expr(operand, expected),
                };
                match op {
                    UnaryOp::Neg => format!("-{inner}"),
                    UnaryOp::Not => format!("!{inner}"),
                }
            }
            Expr::Call(call) => self.format_call(call),
            Expr::Binary {
                left, op, right, ..
            } => {
                let l_kind = self.infer_expr_kind(left);
                let r_kind = self.infer_expr_kind(right);

                if matches!(op, BinaryOp::Equal | BinaryOp::NotEqual)
                    && (l_kind == IrType::Texto || r_kind == IrType::Texto)
                {
                    let l_str = self.format_expr(left, Some(IrType::Texto));
                    let r_str = self.format_expr(right, Some(IrType::Texto));
                    let cmp = if *op == BinaryOp::Equal { "==" } else { "!=" };
                    return format!("(strcmp({l_str}, {r_str}) {cmp} 0)");
                }

                let arg_hint = if matches!(op, BinaryOp::Equal | BinaryOp::NotEqual)
                    && (l_kind == IrType::Char || r_kind == IrType::Char)
                {
                    Some(IrType::Char)
                } else if l_kind.is_numeric() && r_kind.is_numeric() {
                    Some(l_kind.unify_numeric(r_kind))
                } else {
                    None
                };

                let l_str = self.format_expr_child(left, arg_hint);
                let r_str = self.format_expr_child(right, arg_hint);
                format!("{l_str} {} {r_str}", op.as_str())
            }
        }
    }

    fn format_expr_child(&self, expr: &Expr<'_>, expected: Option<IrType>) -> String {
        match expr {
            Expr::Binary { .. } => format!("({})", self.format_expr(expr, expected)),
            _ => self.format_expr(expr, expected),
        }
    }
}

const fn default_c_from_ast(ty: Type) -> &'static str {
    match ty {
        Type::Int => "int",
        Type::Dec => "double",
        Type::Carac => "char",
        Type::Num => "double",
        Type::Vazio => "void",
    }
}

fn c_ident(name: &str) -> String {
    const C_RESERVED: &[&str] = &[
        "auto", "break", "case", "char", "const", "continue", "default", "do", "double", "else",
        "enum", "extern", "float", "for", "goto", "if", "inline", "int", "long", "register",
        "restrict", "return", "short", "signed", "sizeof", "static", "struct", "switch",
        "typedef", "union", "unsigned", "void", "volatile", "while", "bool", "true", "false",
        "main", "printf", "scanf", "strcmp", "snprintf",
    ];
    if C_RESERVED.contains(&name) {
        format!("laila_{name}")
    } else {
        name.to_string()
    }
}

fn escape_c_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

fn escape_c_char(c: char) -> String {
    match c {
        '\\' => "\\\\".to_string(),
        '\'' => "\\'".to_string(),
        '\n' => "\\n".to_string(),
        '\r' => "\\r".to_string(),
        '\t' => "\\t".to_string(),
        _ => c.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{analyze_semantics, parse, tokenize};

    fn to_c(code: &str) -> String {
        let tokens = tokenize(code).unwrap();
        let program = parse(&tokens).unwrap();
        let table = analyze_semantics(&program).unwrap();
        generate_c(&program, &table)
    }

    #[test]
    fn test_codegen_aprovacao_para_c() {
        let code = include_str!("../examples/aprovacao.laila");
        let c = to_c(code);
        assert!(c.contains("int main(void)"));
        assert!(c.contains("float nota_final = 8.5f;"));
        assert!(c.contains("if (nota_final >= 7.0f)"));
        assert!(c.contains("printf(\"%s\\n\", \"Aprovado\");"));
    }

    #[test]
    fn test_codegen_numeros_automaticos() {
        let code = include_str!("../examples/numeros.laila");
        let c = to_c(code);
        assert!(c.contains("int idade = 21;"));
        assert!(c.contains("float preco = 19.9f;"));
        assert!(c.contains("double pi_preciso = 3.141592653589793;"));
        assert!(c.contains("int populacao = 1000000;"));
        assert!(c.contains("float saldo = 1500.75f;"));
    }

    #[test]
    fn test_codegen_num_promovido_entre_atribuicoes() {
        let code = r#"
vazio principal() {
    num x = 10;
    x = 3,50;
    imprima(x);
}
"#;
        let c = to_c(code);
        assert!(c.contains("float x = 10.0f;"));
        assert!(c.contains("x = 3.5f;"));
        assert!(c.contains("printf(\"%.7g\\n\", x);"));
    }

    #[test]
    fn test_paridade_gcc_e_interpretador_ri() {
        if Command::new("gcc").arg("--version").output().is_err() {
            return;
        }

        let code = r#"
int fib(int n) {
    se (n <= 1) {
        retorna n;
    }
    retorna fib(n - 1) + fib(n - 2);
}

vazio principal() {
    num a = 10;
    num b = 2,50;
    num soma = a + b;
    carac letra = "L";
    carac lang = "Laila";
    int k = 0;

    enquanto (k < 6) {
        k = k + 1;
        se (k == 2) {
            continue;
        }
        se (k == 5) {
            pare;
        }
        imprima(k, fib(k));
    }

    se (letra == "L" && lang == "Laila" && !(a < 0)) {
        imprima(soma, -(-4), verdadeiro || falso);
    }
}
"#;
        let tokens = tokenize(code).unwrap();
        let program = parse(&tokens).unwrap();
        let table = analyze_semantics(&program).unwrap();

        // 1. Execução via Interpretador da RI Otimizada
        let ir = crate::ir::build_ir(&program, &table);
        let (opt_ir, _) = crate::optimizer::optimize_ir(&ir);
        let mut vm_out = Vec::new();
        let mut empty_in = std::io::BufReader::new(&[][..]);
        crate::interpreter::interpret_ir_with_io(&opt_ir, &mut empty_in, &mut vm_out).unwrap();
        let vm_text = String::from_utf8(vm_out).unwrap();

        // 2. Transpilação para C + Compilação GCC + Execução Nativa
        let c_code = generate_c(&program, &table);
        let dir = std::env::temp_dir();
        let pid = std::process::id();
        let c_path = dir.join(format!("laila_test_{pid}.c"));
        let exe_path = dir.join(if cfg!(windows) {
            format!("laila_test_{pid}.exe")
        } else {
            format!("laila_test_{pid}")
        });

        std::fs::write(&c_path, &c_code).unwrap();
        compile_c_file(&c_path, &exe_path).unwrap();
        let run_out = Command::new(&exe_path).output().unwrap();
        let _ = std::fs::remove_file(&c_path);
        let _ = std::fs::remove_file(&exe_path);

        assert!(run_out.status.success());
        let gcc_text = String::from_utf8(run_out.stdout).unwrap().replace("\r\n", "\n");
        assert_eq!(vm_text, gcc_text);
        assert!(gcc_text.contains("1 1\n3 2\n4 3\n12.5 4 verdadeiro\n"));
    }
}
