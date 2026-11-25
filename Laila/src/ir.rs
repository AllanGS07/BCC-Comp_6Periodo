use crate::ast::*;
use crate::number::{NumCompiler, NumRepresentation};
use crate::symbol_table::{ScopeId, SemanticType, SymbolTable};
use crate::{Token, TokenKind};
use std::collections::HashMap;
use std::fmt;

/// Tipos concretos utilizados na Representação Intermediária (RI / Código de Três Endereços).
///
/// Nessa fase, o tipo flexível `num` da linguagem Laila já foi resolvido para o seu
/// tipo físico exato (`Int`, `Long`, `Float` ou `Double`) de acordo com a análise semântica.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IrType {
    Int,
    Long,
    Float,
    Double,
    Char,
    Texto,
    Bool,
    Vazio,
}

impl IrType {
    pub fn from_c_layout(sem_ty: SemanticType, c_type: &str) -> Self {
        match sem_ty {
            SemanticType::Int | SemanticType::Num | SemanticType::Dec => match c_type {
                "int" => Self::Int,
                "long long" => Self::Long,
                "float" => Self::Float,
                "double" => Self::Double,
                _ => match sem_ty {
                    SemanticType::Int => Self::Int,
                    _ => Self::Double,
                },
            },
            SemanticType::Carac => match c_type {
                "char" => Self::Char,
                _ => Self::Texto,
            },
            SemanticType::Texto => Self::Texto,
            SemanticType::Bool => Self::Bool,
            SemanticType::Vazio => Self::Vazio,
        }
    }

    pub const fn is_numeric(self) -> bool {
        matches!(self, Self::Int | Self::Long | Self::Float | Self::Double)
    }

    pub const fn to_num_rep(self) -> Option<NumRepresentation> {
        match self {
            Self::Int => Some(NumRepresentation::Int),
            Self::Long => Some(NumRepresentation::Long),
            Self::Float => Some(NumRepresentation::Float),
            Self::Double => Some(NumRepresentation::Double),
            _ => None,
        }
    }

    pub const fn from_num_rep(rep: NumRepresentation) -> Self {
        match rep {
            NumRepresentation::Int => Self::Int,
            NumRepresentation::Long => Self::Long,
            NumRepresentation::Float => Self::Float,
            NumRepresentation::Double => Self::Double,
        }
    }

    pub const fn unify_numeric(self, other: Self) -> Self {
        match (self.to_num_rep(), other.to_num_rep()) {
            (Some(a), Some(b)) => Self::from_num_rep(a.join(b)),
            _ => Self::Double,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Long => "long",
            Self::Float => "float",
            Self::Double => "double",
            Self::Char => "carac",
            Self::Texto => "texto",
            Self::Bool => "bool",
            Self::Vazio => "vazio",
        }
    }
}

impl fmt::Display for IrType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Variável ou registrador temporário na Representação Intermediária.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IrVar {
    pub name: String,
    pub scope: Option<ScopeId>,
    pub ty: IrType,
    pub is_temp: bool,
}

impl fmt::Display for IrVar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.is_temp, self.scope) {
            (true, _) => write!(f, "{}", self.name),
            (false, Some(scope)) => write!(f, "{}@{}", self.name, scope),
            (false, None) => write!(f, "{}", self.name),
        }
    }
}

/// Constante tipada na Representação Intermediária.
#[derive(Debug, Clone, PartialEq)]
pub enum IrConst {
    Int(i64, bool),
    Float(f64, bool),
    Char(char),
    Texto(String),
    Bool(bool),
}

impl IrConst {
    pub const fn ty(&self) -> IrType {
        match self {
            Self::Int(_, true) => IrType::Long,
            Self::Int(_, false) => IrType::Int,
            Self::Float(_, true) => IrType::Float,
            Self::Float(_, false) => IrType::Double,
            Self::Char(_) => IrType::Char,
            Self::Texto(_) => IrType::Texto,
            Self::Bool(_) => IrType::Bool,
        }
    }

    /// Converte uma constante numérica para o tipo alvo quando compatível.
    pub fn coerce_to(&self, target: IrType) -> Self {
        match (self, target) {
            (Self::Int(v, _), IrType::Int) => Self::Int(*v as i32 as i64, false),
            (Self::Int(v, _), IrType::Long) => Self::Int(*v, true),
            (Self::Int(v, _), IrType::Float) => Self::Float((*v as f32) as f64, true),
            (Self::Int(v, _), IrType::Double) => Self::Float(*v as f64, false),
            (Self::Float(v, _), IrType::Float) => Self::Float((*v as f32) as f64, true),
            (Self::Float(v, _), IrType::Double) => Self::Float(*v, false),
            (Self::Char(c), IrType::Texto) => Self::Texto(c.to_string()),
            _ => self.clone(),
        }
    }
}

impl fmt::Display for IrConst {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(v, true) => write!(f, "{v}L"),
            Self::Int(v, false) => write!(f, "{v}"),
            Self::Float(v, true) => write!(f, "{v}f"),
            Self::Float(v, false) => write!(f, "{v}"),
            Self::Char(c) => write!(f, "'{c}'"),
            Self::Texto(s) => write!(f, "{s:?}"),
            Self::Bool(true) => f.write_str("verdadeiro"),
            Self::Bool(false) => f.write_str("falso"),
        }
    }
}

/// Operando de uma instrução de três endereços.
#[derive(Debug, Clone, PartialEq)]
pub enum IrOperand {
    Const(IrConst),
    Var(IrVar),
}

impl IrOperand {
    pub const fn ty(&self) -> IrType {
        match self {
            Self::Const(c) => c.ty(),
            Self::Var(v) => v.ty,
        }
    }
}

impl fmt::Display for IrOperand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Const(c) => write!(f, "{c}"),
            Self::Var(v) => write!(f, "{v}"),
        }
    }
}

/// Operadores binários da RI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
}

impl IrBinOp {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Mod => "%",
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Gt => ">",
            Self::Le => "<=",
            Self::Ge => ">=",
            Self::And => "&&",
            Self::Or => "||",
        }
    }
}

impl fmt::Display for IrBinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Operadores unários da RI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrUnOp {
    Neg,
    Not,
}

impl fmt::Display for IrUnOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Neg => f.write_str("-"),
            Self::Not => f.write_str("!"),
        }
    }
}

/// Instrução individual no formato de Código de Três Endereços (TAC).
#[derive(Debug, Clone, PartialEq)]
pub enum IrInstruction {
    Label(String),
    Assign {
        dst: IrVar,
        src: IrOperand,
    },
    Binary {
        dst: IrVar,
        op: IrBinOp,
        left: IrOperand,
        right: IrOperand,
    },
    Unary {
        dst: IrVar,
        op: IrUnOp,
        operand: IrOperand,
    },
    Jump(String),
    JumpIfFalse {
        cond: IrOperand,
        target: String,
    },
    JumpIfTrue {
        cond: IrOperand,
        target: String,
    },
    Param(IrOperand),
    Call {
        dst: Option<IrVar>,
        callee: String,
        arg_count: usize,
    },
    Return(Option<IrOperand>),
    Print(Vec<IrOperand>),
    Read(IrVar),
}

impl fmt::Display for IrInstruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Label(name) => write!(f, "{name}:"),
            Self::Assign { dst, src } => write!(f, "  {dst} ({}) = {src}", dst.ty),
            Self::Binary {
                dst,
                op,
                left,
                right,
            } => write!(f, "  {dst} ({}) = {left} {op} {right}", dst.ty),
            Self::Unary { dst, op, operand } => {
                write!(f, "  {dst} ({}) = {op}{operand}", dst.ty)
            }
            Self::Jump(target) => write!(f, "  ir_para {target}"),
            Self::JumpIfFalse { cond, target } => write!(f, "  se_falso {cond} ir_para {target}"),
            Self::JumpIfTrue { cond, target } => write!(f, "  se_verdadeiro {cond} ir_para {target}"),
            Self::Param(op) => write!(f, "  param {op}"),
            Self::Call {
                dst: Some(dst),
                callee,
                arg_count,
            } => write!(
                f,
                "  {dst} ({}) = chamar {callee}, {arg_count}",
                dst.ty
            ),
            Self::Call {
                dst: None,
                callee,
                arg_count,
            } => write!(f, "  chamar {callee}, {arg_count}"),
            Self::Return(Some(val)) => write!(f, "  retorna {val}"),
            Self::Return(None) => f.write_str("  retorna"),
            Self::Print(args) => {
                let items: Vec<String> = args.iter().map(|a| a.to_string()).collect();
                write!(f, "  imprima {}", items.join(", "))
            }
            Self::Read(var) => write!(f, "  leia {var}"),
        }
    }
}

/// Função compilada na Representação Intermediária.
#[derive(Debug, Clone, PartialEq)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<IrVar>,
    pub return_type: IrType,
    pub locals: Vec<IrVar>,
    pub instructions: Vec<IrInstruction>,
}

/// Programa completo na Representação Intermediária (TAC).
#[derive(Debug, Clone, PartialEq)]
pub struct IrProgram {
    pub globals_init: Vec<IrInstruction>,
    pub functions: Vec<IrFunction>,
}

impl IrProgram {
    pub fn instruction_count(&self) -> usize {
        self.globals_init.len()
            + self
                .functions
                .iter()
                .map(|f| f.instructions.len())
                .sum::<usize>()
    }

    pub fn format_ir(&self) -> String {
        let mut out = String::new();
        if !self.globals_init.is_empty() {
            out.push_str("bloco_global (B0):\n");
            for instr in &self.globals_init {
                out.push_str(&format!("{instr}\n"));
            }
            out.push('\n');
        }
        for (i, func) in self.functions.iter().enumerate() {
            let params_str: Vec<String> = func
                .params
                .iter()
                .map(|p| format!("{p}: {}", p.ty))
                .collect();
            out.push_str(&format!(
                "funcao {}({}) -> {}\n",
                func.name,
                params_str.join(", "),
                func.return_type
            ));
            for instr in &func.instructions {
                out.push_str(&format!("{instr}\n"));
            }
            if i + 1 < self.functions.len() {
                out.push('\n');
            }
        }
        out
    }
}

impl fmt::Display for IrProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format_ir())
    }
}

/// Gerador da Representação Intermediária (AST + Tabela de Símbolos -> TAC).
pub struct IrBuilder<'a> {
    table: &'a SymbolTable,
    temp_counter: usize,
    label_counter: usize,
    loop_stack: Vec<(String, String)>,
    func_return_types: HashMap<String, IrType>,
    func_param_types: HashMap<String, Vec<IrType>>,
}

impl<'a> IrBuilder<'a> {
    pub fn new(table: &'a SymbolTable) -> Self {
        let mut func_return_types = HashMap::new();
        let b0 = table.scope(ScopeId(0));
        for name in &b0.symbol_order {
            let sym = &b0.symbols[name];
            func_return_types.insert(name.clone(), IrType::from_c_layout(sym.ty, sym.c_type));
        }
        Self {
            table,
            temp_counter: 0,
            label_counter: 0,
            loop_stack: Vec::new(),
            func_return_types,
            func_param_types: HashMap::new(),
        }
    }

    pub fn build(mut self, program: &Program<'_>) -> IrProgram {
        for item in &program.items {
            if let ProgramItem::Function(func) = item {
                let fn_scope = self
                    .table
                    .scope_for_block(func.body.span)
                    .unwrap_or(ScopeId(0));
                let p_types = func
                    .params
                    .iter()
                    .map(|p| self.symbol_to_ir_var(fn_scope, p.name).ty)
                    .collect();
                self.func_param_types.insert(func.name.to_string(), p_types);
            }
        }

        let mut globals_init = Vec::new();
        let mut functions = Vec::new();

        for item in &program.items {
            match item {
                ProgramItem::Function(func) => {
                    functions.push(self.build_function(func));
                }
                ProgramItem::Statement(stmt) => {
                    let mut locals = Vec::new();
                    self.lower_stmt(stmt, None, &mut globals_init, &mut locals);
                }
            }
        }

        IrProgram {
            globals_init,
            functions,
        }
    }

    fn new_temp(&mut self, ty: IrType) -> IrVar {
        let idx = self.temp_counter;
        self.temp_counter += 1;
        IrVar {
            name: format!("t{idx}"),
            scope: None,
            ty,
            is_temp: true,
        }
    }

    fn new_label(&mut self, prefix: &str) -> String {
        let idx = self.label_counter;
        self.label_counter += 1;
        format!("L{idx}_{prefix}")
    }

    fn symbol_to_ir_var(&self, scope: ScopeId, name: &str) -> IrVar {
        if let Some(sym) = self.table.symbol(scope, name) {
            IrVar {
                name: name.to_string(),
                scope: Some(scope),
                ty: IrType::from_c_layout(sym.ty, sym.c_type),
                is_temp: false,
            }
        } else {
            IrVar {
                name: name.to_string(),
                scope: Some(scope),
                ty: IrType::Double,
                is_temp: false,
            }
        }
    }

    fn build_function(&mut self, func: &Function<'_>) -> IrFunction {
        let fn_scope = self
            .table
            .scope_for_block(func.body.span)
            .unwrap_or(ScopeId(0));

        let params: Vec<IrVar> = func
            .params
            .iter()
            .map(|p| self.symbol_to_ir_var(fn_scope, p.name))
            .collect();

        let return_type = self
            .func_return_types
            .get(func.name)
            .copied()
            .unwrap_or_else(|| {
                IrType::from_c_layout(SemanticType::from_ast_type(func.return_type), "")
            });

        let mut instructions = Vec::new();
        let mut locals = Vec::new();

        for stmt in &func.body.statements {
            self.lower_stmt(stmt, Some(return_type), &mut instructions, &mut locals);
        }

        if !matches!(instructions.last(), Some(IrInstruction::Return(_)))
            && return_type == IrType::Vazio
        {
            instructions.push(IrInstruction::Return(None));
        }

        IrFunction {
            name: func.name.to_string(),
            params,
            return_type,
            locals,
            instructions,
        }
    }

    fn lower_stmt(
        &mut self,
        stmt: &Statement<'_>,
        fn_ret_ty: Option<IrType>,
        out: &mut Vec<IrInstruction>,
        locals: &mut Vec<IrVar>,
    ) {
        match stmt {
            Statement::VarDecl(decl) => {
                let scope_id = self.table.scope_for_decl(decl.span).unwrap_or(ScopeId(0));
                let var = self.symbol_to_ir_var(scope_id, decl.name);
                locals.push(var.clone());
                if let Some(init) = &decl.init {
                    let src = self.lower_expr(init, Some(var.ty), out);
                    out.push(IrInstruction::Assign { dst: var, src });
                }
            }
            Statement::Assignment(assign) => {
                let scope_id = self
                    .table
                    .find_reference_at(assign.span)
                    .map(|r| r.resolved_scope)
                    .unwrap_or(ScopeId(0));
                let var = self.symbol_to_ir_var(scope_id, assign.name);
                let src = self.lower_expr(&assign.value, Some(var.ty), out);
                out.push(IrInstruction::Assign { dst: var, src });
            }
            Statement::If(if_stmt) => {
                let cond = self.lower_expr(&if_stmt.condition, Some(IrType::Bool), out);
                if let Some(else_branch) = &if_stmt.else_branch {
                    let else_label = self.new_label("senao");
                    let end_label = self.new_label("fim_se");
                    out.push(IrInstruction::JumpIfFalse {
                        cond,
                        target: else_label.clone(),
                    });
                    for s in &if_stmt.then_branch.statements {
                        self.lower_stmt(s, fn_ret_ty, out, locals);
                    }
                    out.push(IrInstruction::Jump(end_label.clone()));
                    out.push(IrInstruction::Label(else_label));
                    for s in &else_branch.statements {
                        self.lower_stmt(s, fn_ret_ty, out, locals);
                    }
                    out.push(IrInstruction::Label(end_label));
                } else {
                    let end_label = self.new_label("fim_se");
                    out.push(IrInstruction::JumpIfFalse {
                        cond,
                        target: end_label.clone(),
                    });
                    for s in &if_stmt.then_branch.statements {
                        self.lower_stmt(s, fn_ret_ty, out, locals);
                    }
                    out.push(IrInstruction::Label(end_label));
                }
            }
            Statement::While(while_stmt) => {
                let cond_label = self.new_label("enquanto_cond");
                let end_label = self.new_label("enquanto_fim");
                self.loop_stack
                    .push((cond_label.clone(), end_label.clone()));

                out.push(IrInstruction::Label(cond_label.clone()));
                let cond = self.lower_expr(&while_stmt.condition, Some(IrType::Bool), out);
                out.push(IrInstruction::JumpIfFalse {
                    cond,
                    target: end_label.clone(),
                });
                for s in &while_stmt.body.statements {
                    self.lower_stmt(s, fn_ret_ty, out, locals);
                }
                out.push(IrInstruction::Jump(cond_label));
                out.push(IrInstruction::Label(end_label));

                self.loop_stack.pop();
            }
            Statement::For(for_stmt) => {
                if let Some(init) = &for_stmt.init {
                    match init {
                        ForInit::VarDecl(d) => {
                            let scope_id =
                                self.table.scope_for_decl(d.span).unwrap_or(ScopeId(0));
                            let var = self.symbol_to_ir_var(scope_id, d.name);
                            locals.push(var.clone());
                            if let Some(init_expr) = &d.init {
                                let src = self.lower_expr(init_expr, Some(var.ty), out);
                                out.push(IrInstruction::Assign { dst: var, src });
                            }
                        }
                        ForInit::Assignment(a) => {
                            let scope_id = self
                                .table
                                .find_reference_at(a.span)
                                .map(|r| r.resolved_scope)
                                .unwrap_or(ScopeId(0));
                            let var = self.symbol_to_ir_var(scope_id, a.name);
                            let src = self.lower_expr(&a.value, Some(var.ty), out);
                            out.push(IrInstruction::Assign { dst: var, src });
                        }
                    }
                }

                let cond_label = self.new_label("para_cond");
                let inc_label = self.new_label("para_inc");
                let end_label = self.new_label("para_fim");
                self.loop_stack
                    .push((inc_label.clone(), end_label.clone()));

                out.push(IrInstruction::Label(cond_label.clone()));
                if let Some(cond_expr) = &for_stmt.condition {
                    let cond = self.lower_expr(cond_expr, Some(IrType::Bool), out);
                    out.push(IrInstruction::JumpIfFalse {
                        cond,
                        target: end_label.clone(),
                    });
                }
                for s in &for_stmt.body.statements {
                    self.lower_stmt(s, fn_ret_ty, out, locals);
                }
                out.push(IrInstruction::Label(inc_label));
                if let Some(inc) = &for_stmt.increment {
                    let scope_id = self
                        .table
                        .find_reference_at(inc.span)
                        .map(|r| r.resolved_scope)
                        .unwrap_or(ScopeId(0));
                    let var = self.symbol_to_ir_var(scope_id, inc.name);
                    let src = self.lower_expr(&inc.value, Some(var.ty), out);
                    out.push(IrInstruction::Assign { dst: var, src });
                }
                out.push(IrInstruction::Jump(cond_label));
                out.push(IrInstruction::Label(end_label));

                self.loop_stack.pop();
            }
            Statement::Break(_) => {
                if let Some((_, break_target)) = self.loop_stack.last() {
                    out.push(IrInstruction::Jump(break_target.clone()));
                }
            }
            Statement::Continue(_) => {
                if let Some((continue_target, _)) = self.loop_stack.last() {
                    out.push(IrInstruction::Jump(continue_target.clone()));
                }
            }
            Statement::Print(p) => {
                let mut args = Vec::with_capacity(p.args.len());
                for arg in &p.args {
                    args.push(self.lower_expr(arg, None, out));
                }
                out.push(IrInstruction::Print(args));
            }
            Statement::Read(r) => {
                let scope_id = self
                    .table
                    .find_reference_at(r.span)
                    .map(|ref_info| ref_info.resolved_scope)
                    .unwrap_or(ScopeId(0));
                let var = self.symbol_to_ir_var(scope_id, r.var_name);
                out.push(IrInstruction::Read(var));
            }
            Statement::Return(ret) => {
                let val = ret
                    .value
                    .as_ref()
                    .map(|e| self.lower_expr(e, fn_ret_ty, out));
                out.push(IrInstruction::Return(val));
            }
            Statement::FunctionCall(call) => {
                let _ = self.lower_call(call, false, out);
            }
            Statement::Block(b) => {
                for s in &b.statements {
                    self.lower_stmt(s, fn_ret_ty, out, locals);
                }
            }
        }
    }

    fn lower_call(
        &mut self,
        call: &FunctionCall<'_>,
        need_result: bool,
        out: &mut Vec<IrInstruction>,
    ) -> Option<IrVar> {
        let param_types: Vec<IrType> = self
            .func_param_types
            .get(call.callee)
            .cloned()
            .unwrap_or_default();

        let mut arg_ops = Vec::with_capacity(call.args.len());
        for (i, arg) in call.args.iter().enumerate() {
            let expected = param_types.get(i).copied();
            arg_ops.push(self.lower_expr(arg, expected, out));
        }

        for op in arg_ops {
            out.push(IrInstruction::Param(op));
        }

        let ret_ty = self
            .func_return_types
            .get(call.callee)
            .copied()
            .unwrap_or(IrType::Vazio);

        let dst = if need_result && ret_ty != IrType::Vazio {
            Some(self.new_temp(ret_ty))
        } else {
            None
        };

        out.push(IrInstruction::Call {
            dst: dst.clone(),
            callee: call.callee.to_string(),
            arg_count: call.args.len(),
        });

        dst
    }

    fn lower_expr(
        &mut self,
        expr: &Expr<'_>,
        expected_ty: Option<IrType>,
        out: &mut Vec<IrInstruction>,
    ) -> IrOperand {
        match expr {
            Expr::Identifier { name, span } => {
                let scope_id = self
                    .table
                    .find_reference_at(*span)
                    .map(|r| r.resolved_scope)
                    .unwrap_or(ScopeId(0));
                IrOperand::Var(self.symbol_to_ir_var(scope_id, name))
            }
            Expr::IntLiteral { raw, span } => {
                let compiler = NumCompiler::new();
                let tok = Token {
                    kind: TokenKind::NumInt,
                    lexeme: raw,
                    span: *span,
                };
                if let Ok(compiled) = compiler.lower_literal(&tok) {
                    let val: i64 = raw.replace('.', "").parse().unwrap_or(0);
                    let base = IrConst::Int(val, compiled.representation == NumRepresentation::Long);
                    let coerced = match expected_ty {
                        Some(t) if t.is_numeric() => base.coerce_to(t),
                        _ => base,
                    };
                    IrOperand::Const(coerced)
                } else {
                    IrOperand::Const(IrConst::Int(0, false))
                }
            }
            Expr::DecLiteral { raw, span } => {
                let compiler = NumCompiler::new();
                let tok = Token {
                    kind: TokenKind::NumDec,
                    lexeme: raw,
                    span: *span,
                };
                if let Ok(compiled) = compiler.lower_literal(&tok) {
                    let clean = raw.replace('.', "").replace(',', ".");
                    let val: f64 = clean.parse().unwrap_or(0.0);
                    let is_f32 = matches!(
                        compiled.representation,
                        NumRepresentation::Float | NumRepresentation::Int
                    );
                    let base = IrConst::Float(val, is_f32);
                    let coerced = match expected_ty {
                        Some(t @ (IrType::Float | IrType::Double)) => base.coerce_to(t),
                        _ => base,
                    };
                    IrOperand::Const(coerced)
                } else {
                    IrOperand::Const(IrConst::Float(0.0, false))
                }
            }
            Expr::StringLiteral { content, .. } => {
                let mut chars = content.chars();
                if expected_ty == Some(IrType::Char)
                    && let (Some(c), None) = (chars.next(), chars.next()) {
                        return IrOperand::Const(IrConst::Char(c));
                    }
                IrOperand::Const(IrConst::Texto((*content).to_string()))
            }
            Expr::BoolLiteral { value, .. } => IrOperand::Const(IrConst::Bool(*value)),
            Expr::Grouping { expr, .. } => self.lower_expr(expr, expected_ty, out),
            Expr::Unary { op, operand, .. } => {
                let op_val = self.lower_expr(operand, expected_ty, out);
                let (ir_op, dst_ty) = match op {
                    UnaryOp::Neg => (IrUnOp::Neg, op_val.ty()),
                    UnaryOp::Not => (IrUnOp::Not, IrType::Bool),
                };
                let dst = self.new_temp(dst_ty);
                out.push(IrInstruction::Unary {
                    dst: dst.clone(),
                    op: ir_op,
                    operand: op_val,
                });
                IrOperand::Var(dst)
            }
            Expr::Call(call) => {
                if let Some(dst) = self.lower_call(call, true, out) {
                    IrOperand::Var(dst)
                } else {
                    IrOperand::Const(IrConst::Int(0, false))
                }
            }
            Expr::Binary {
                left, op, right, ..
            } => self.lower_binary(*op, left, right, out),
        }
    }

    fn lower_binary(
        &mut self,
        op: BinaryOp,
        left: &Expr<'_>,
        right: &Expr<'_>,
        out: &mut Vec<IrInstruction>,
    ) -> IrOperand {
        match op {
            BinaryOp::And => {
                let dst = self.new_temp(IrType::Bool);
                let false_label = self.new_label("and_falso");
                let done_label = self.new_label("and_fim");

                let l_op = self.lower_expr(left, Some(IrType::Bool), out);
                out.push(IrInstruction::JumpIfFalse {
                    cond: l_op,
                    target: false_label.clone(),
                });
                let r_op = self.lower_expr(right, Some(IrType::Bool), out);
                out.push(IrInstruction::Assign {
                    dst: dst.clone(),
                    src: r_op,
                });
                out.push(IrInstruction::Jump(done_label.clone()));
                out.push(IrInstruction::Label(false_label));
                out.push(IrInstruction::Assign {
                    dst: dst.clone(),
                    src: IrOperand::Const(IrConst::Bool(false)),
                });
                out.push(IrInstruction::Label(done_label));
                IrOperand::Var(dst)
            }
            BinaryOp::Or => {
                let dst = self.new_temp(IrType::Bool);
                let true_label = self.new_label("or_verd");
                let done_label = self.new_label("or_fim");

                let l_op = self.lower_expr(left, Some(IrType::Bool), out);
                out.push(IrInstruction::JumpIfTrue {
                    cond: l_op,
                    target: true_label.clone(),
                });
                let r_op = self.lower_expr(right, Some(IrType::Bool), out);
                out.push(IrInstruction::Assign {
                    dst: dst.clone(),
                    src: r_op,
                });
                out.push(IrInstruction::Jump(done_label.clone()));
                out.push(IrInstruction::Label(true_label));
                out.push(IrInstruction::Assign {
                    dst: dst.clone(),
                    src: IrOperand::Const(IrConst::Bool(true)),
                });
                out.push(IrInstruction::Label(done_label));
                IrOperand::Var(dst)
            }
            _ => {
                let l_op = self.lower_expr(left, None, out);
                let right_hint = if l_op.ty() == IrType::Char {
                    Some(IrType::Char)
                } else {
                    None
                };
                let mut r_op = self.lower_expr(right, right_hint, out);

                // Se `left` era literal de 1 char e `right` era `Char`, ajusta `left` para `Char`
                let l_op = if r_op.ty() == IrType::Char {
                    if let IrOperand::Const(IrConst::Texto(ref s)) = l_op {
                        let mut cs = s.chars();
                        if let (Some(c), None) = (cs.next(), cs.next()) {
                            IrOperand::Const(IrConst::Char(c))
                        } else {
                            l_op
                        }
                    } else {
                        l_op
                    }
                } else {
                    l_op
                };

                // Se `r_op` é Char e `l_op` é Texto, ajusta ambos para Texto
                if l_op.ty() == IrType::Texto && r_op.ty() == IrType::Char
                    && let IrOperand::Const(IrConst::Char(c)) = r_op {
                        r_op = IrOperand::Const(IrConst::Texto(c.to_string()));
                    }

                let (ir_op, res_ty) = match op {
                    BinaryOp::Add => (IrBinOp::Add, l_op.ty().unify_numeric(r_op.ty())),
                    BinaryOp::Sub => (IrBinOp::Sub, l_op.ty().unify_numeric(r_op.ty())),
                    BinaryOp::Mul => (IrBinOp::Mul, l_op.ty().unify_numeric(r_op.ty())),
                    BinaryOp::Div => (IrBinOp::Div, l_op.ty().unify_numeric(r_op.ty())),
                    BinaryOp::Mod => (IrBinOp::Mod, l_op.ty().unify_numeric(r_op.ty())),
                    BinaryOp::Equal => (IrBinOp::Eq, IrType::Bool),
                    BinaryOp::NotEqual => (IrBinOp::Ne, IrType::Bool),
                    BinaryOp::LessThan => (IrBinOp::Lt, IrType::Bool),
                    BinaryOp::GreaterThan => (IrBinOp::Gt, IrType::Bool),
                    BinaryOp::LessEqual => (IrBinOp::Le, IrType::Bool),
                    BinaryOp::GreaterEqual => (IrBinOp::Ge, IrType::Bool),
                    BinaryOp::And | BinaryOp::Or => unreachable!(),
                };

                let dst = self.new_temp(res_ty);
                out.push(IrInstruction::Binary {
                    dst: dst.clone(),
                    op: ir_op,
                    left: l_op,
                    right: r_op,
                });
                IrOperand::Var(dst)
            }
        }
    }
}

/// Gera a Representação Intermediária (TAC) a partir da AST e Tabela de Símbolos validadas.
pub fn build_ir(program: &Program<'_>, table: &SymbolTable) -> IrProgram {
    IrBuilder::new(table).build(program)
}
