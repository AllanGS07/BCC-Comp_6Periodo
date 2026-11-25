use crate::ast::*;
use crate::number::{NumCompiler, NumRepresentation};
use crate::symbol_table::{ScopeId, SemanticType, Symbol, SymbolKind, SymbolTable};
use crate::{Span, Token, TokenKind};
use std::collections::HashMap;
use std::fmt;

/// Categorias de erros semânticos detectados na linguagem Laila.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticErrorKind {
    UndeclaredIdentifier { name: String },
    Redeclaration {
        name: String,
        scope: ScopeId,
        previous_span: Span,
    },
    TypeMismatch {
        expected: SemanticType,
        found: SemanticType,
        context: String,
    },
    InvalidOperation {
        op: String,
        left: SemanticType,
        right: Option<SemanticType>,
    },
    NonBooleanCondition {
        found: SemanticType,
        construct: &'static str,
    },
    InvalidSymbolKind {
        name: String,
        expected: &'static str,
        found: &'static str,
    },
    ArityMismatch {
        callee: String,
        expected: usize,
        found: usize,
    },
    ReturnMismatch {
        function_name: String,
        expected: SemanticType,
        found: SemanticType,
    },
    LoopControlOutsideLoop { keyword: &'static str },
    VoidVariable { name: String },
}

/// Erro semântico com posição exata no código-fonte (`Span`) e mensagem explicativa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticError {
    pub kind: SemanticErrorKind,
    pub message: String,
    pub span: Span,
}

impl fmt::Display for SemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: erro semântico: {}",
            self.span.start.line, self.span.start.column, self.message
        )
    }
}

impl std::error::Error for SemanticError {}

#[derive(Debug, Clone)]
struct FunctionContext {
    name: String,
    return_type: SemanticType,
}

/// Analisador Semântico da linguagem Laila.
pub struct SemanticAnalyzer {
    pub table: SymbolTable,
    errors: Vec<SemanticError>,
    current_function: Option<FunctionContext>,
    loop_depth: usize,
}

impl Default for SemanticAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self {
            table: SymbolTable::new(),
            errors: Vec::new(),
            current_function: None,
            loop_depth: 0,
        }
    }

    pub fn analyze(mut self, program: &Program<'_>) -> Result<SymbolTable, Vec<SemanticError>> {
        for item in &program.items {
            match item {
                ProgramItem::Function(func) => self.check_function(func),
                ProgramItem::Statement(stmt) => self.check_statement(stmt),
            }
        }
        self.refine_layouts_and_check_mod(program);

        if self.errors.is_empty() {
            Ok(self.table)
        } else {
            Err(self.errors)
        }
    }

    fn check_function(&mut self, func: &Function<'_>) {
        let ret_ty = SemanticType::from_ast_type(func.return_type);
        let param_types: Vec<SemanticType> = func
            .params
            .iter()
            .map(|p| SemanticType::from_ast_type(p.param_type))
            .collect();

        if let Err(existing) = self.table.insert(
            func.name,
            ret_ty,
            SymbolKind::Function {
                params: param_types,
            },
            func.span,
        ) {
            self.push_redeclaration_error(func.name, func.span, &existing);
        }

        self.table
            .enter_scope_for(format!("função '{}'", func.name), func.body.span);

        for param in &func.params {
            let p_ty = SemanticType::from_ast_type(param.param_type);
            if p_ty == SemanticType::Vazio {
                self.errors.push(SemanticError {
                    kind: SemanticErrorKind::VoidVariable {
                        name: param.name.to_string(),
                    },
                    message: format!(
                        "o parâmetro '{}' não pode ser declarado com o tipo 'vazio'",
                        param.name
                    ),
                    span: param.span,
                });
            }
            if let Err(existing) =
                self.table
                    .insert(param.name, p_ty, SymbolKind::Parameter, param.span)
            {
                self.push_redeclaration_error(param.name, param.span, &existing);
            }
        }

        let prev_func = self.current_function.replace(FunctionContext {
            name: func.name.to_string(),
            return_type: ret_ty,
        });

        for stmt in &func.body.statements {
            self.check_statement(stmt);
        }

        self.current_function = prev_func;
        self.table.exit_scope();
    }

    fn check_block(&mut self, block: &Block<'_>, description: impl Into<String>) {
        self.table.enter_scope_for(description, block.span);
        for stmt in &block.statements {
            self.check_statement(stmt);
        }
        self.table.exit_scope();
    }

    fn check_statement(&mut self, stmt: &Statement<'_>) {
        match stmt {
            Statement::VarDecl(decl) => self.check_var_decl(decl),
            Statement::Assignment(assign) => self.check_assignment(assign),
            Statement::If(if_stmt) => self.check_if(if_stmt),
            Statement::While(while_stmt) => self.check_while(while_stmt),
            Statement::For(for_stmt) => self.check_for(for_stmt),
            Statement::Print(print_stmt) => self.check_print(print_stmt),
            Statement::Read(read_stmt) => self.check_read(read_stmt),
            Statement::Return(ret_stmt) => self.check_return(ret_stmt),
            Statement::Break(span) => self.check_loop_control("pare", *span),
            Statement::Continue(span) => self.check_loop_control("continue", *span),
            Statement::FunctionCall(call) => {
                let _ = self.check_function_call(call);
            }
            Statement::Block(block) => {
                self.check_block(
                    block,
                    format!("bloco aninhado (linha {})", block.span.start.line),
                );
            }
        }
    }

    fn check_loop_control(&mut self, keyword: &'static str, span: Span) {
        if self.loop_depth == 0 {
            self.errors.push(SemanticError {
                kind: SemanticErrorKind::LoopControlOutsideLoop { keyword },
                message: format!(
                    "o comando '{keyword}' só pode ser utilizado dentro de um laço ('enquanto' ou 'para')"
                ),
                span,
            });
        }
    }

    fn check_var_decl(&mut self, decl: &VarDecl<'_>) {
        let var_ty = SemanticType::from_ast_type(decl.var_type);
        if var_ty == SemanticType::Vazio {
            self.errors.push(SemanticError {
                kind: SemanticErrorKind::VoidVariable {
                    name: decl.name.to_string(),
                },
                message: format!(
                    "a variável '{}' não pode ser declarada com o tipo 'vazio'",
                    decl.name
                ),
                span: decl.span,
            });
        }

        let init_ty = decl.init.as_ref().and_then(|expr| self.check_expr(expr));

        if let Err(existing) =
            self.table
                .insert(decl.name, var_ty, SymbolKind::Variable, decl.span)
        {
            self.push_redeclaration_error(decl.name, decl.span, &existing);
        }

        if let (Some(expr), Some(found_ty)) = (&decl.init, init_ty)
            && !found_ty.is_assignable_to(var_ty) {
                self.errors.push(SemanticError {
                    kind: SemanticErrorKind::TypeMismatch {
                        expected: var_ty,
                        found: found_ty,
                        context: format!("inicialização da variável '{}'", decl.name),
                    },
                    message: format!(
                        "tipos incompatíveis na declaração de '{}': variável é do tipo '{}', mas recebeu expressão do tipo '{}'",
                        decl.name, var_ty, found_ty
                    ),
                    span: expr.span(),
                });
            }
    }

    fn check_assignment(&mut self, assign: &Assignment<'_>) {
        let value_ty = self.check_expr(&assign.value);

        match self.table.resolve_and_record(assign.name, assign.span) {
            Some(sym) => {
                if matches!(sym.kind, SymbolKind::Function { .. }) {
                    self.errors.push(SemanticError {
                        kind: SemanticErrorKind::InvalidSymbolKind {
                            name: assign.name.to_string(),
                            expected: "variável",
                            found: "função",
                        },
                        message: format!(
                            "não é possível atribuir um valor a '{}' pois é uma função, não uma variável",
                            assign.name
                        ),
                        span: assign.span,
                    });
                    return;
                }

                if let Some(found_ty) = value_ty
                    && !found_ty.is_assignable_to(sym.ty) {
                        self.errors.push(SemanticError {
                            kind: SemanticErrorKind::TypeMismatch {
                                expected: sym.ty,
                                found: found_ty,
                                context: format!("atribuição à variável '{}'", assign.name),
                            },
                            message: format!(
                                "tipos incompatíveis na atribuição a '{}' (declarada em {}): esperado '{}', mas foi encontrado '{}'",
                                assign.name, sym.scope_id, sym.ty, found_ty
                            ),
                            span: assign.value.span(),
                        });
                    }
            }
            None => self.push_undeclared_error(assign.name, assign.span),
        }
    }

    fn check_condition(&mut self, cond: &Expr<'_>, construct: &'static str) {
        if let Some(cond_ty) = self.check_expr(cond)
            && cond_ty != SemanticType::Bool {
                self.errors.push(SemanticError {
                    kind: SemanticErrorKind::NonBooleanCondition {
                        found: cond_ty,
                        construct,
                    },
                    message: format!(
                        "a condição do comando '{construct}' deve ser lógica ('bool'), mas foi encontrado '{cond_ty}'"
                    ),
                    span: cond.span(),
                });
            }
    }

    fn check_if(&mut self, if_stmt: &IfStmt<'_>) {
        self.check_condition(&if_stmt.condition, "se");
        self.check_block(
            &if_stmt.then_branch,
            format!("bloco 'se' (linha {})", if_stmt.then_branch.span.start.line),
        );
        if let Some(else_branch) = &if_stmt.else_branch {
            self.check_block(
                else_branch,
                format!("bloco 'senao' (linha {})", else_branch.span.start.line),
            );
        }
    }

    fn check_while(&mut self, while_stmt: &WhileStmt<'_>) {
        self.check_condition(&while_stmt.condition, "enquanto");
        self.loop_depth += 1;
        self.check_block(
            &while_stmt.body,
            format!(
                "bloco 'enquanto' (linha {})",
                while_stmt.body.span.start.line
            ),
        );
        self.loop_depth -= 1;
    }

    fn check_for(&mut self, for_stmt: &ForStmt<'_>) {
        self.table.enter_scope_for(
            format!("laço 'para' (linha {})", for_stmt.span.start.line),
            for_stmt.span,
        );

        if let Some(init) = &for_stmt.init {
            match init {
                ForInit::Assignment(assign) => self.check_assignment(assign),
                ForInit::VarDecl(decl) => self.check_var_decl(decl),
            }
        }

        if let Some(cond) = &for_stmt.condition {
            self.check_condition(cond, "para");
        }

        if let Some(inc) = &for_stmt.increment {
            self.check_assignment(inc);
        }

        self.loop_depth += 1;
        for stmt in &for_stmt.body.statements {
            self.check_statement(stmt);
        }
        self.loop_depth -= 1;

        self.table.exit_scope();
    }

    fn check_print(&mut self, print_stmt: &PrintStmt<'_>) {
        for arg in &print_stmt.args {
            if self.check_expr(arg) == Some(SemanticType::Vazio) {
                self.errors.push(SemanticError {
                    kind: SemanticErrorKind::TypeMismatch {
                        expected: SemanticType::Texto,
                        found: SemanticType::Vazio,
                        context: "argumento de 'imprima'".to_string(),
                    },
                    message: "não é possível imprimir uma expressão do tipo 'vazio'".to_string(),
                    span: arg.span(),
                });
            }
        }
    }

    fn check_read(&mut self, read_stmt: &ReadStmt<'_>) {
        match self.table.resolve_and_record(read_stmt.var_name, read_stmt.span) {
            Some(sym) if matches!(sym.kind, SymbolKind::Function { .. }) => {
                self.errors.push(SemanticError {
                    kind: SemanticErrorKind::InvalidSymbolKind {
                        name: read_stmt.var_name.to_string(),
                        expected: "variável",
                        found: "função",
                    },
                    message: format!(
                        "o comando 'leia' exige uma variável, mas '{}' é uma função",
                        read_stmt.var_name
                    ),
                    span: read_stmt.span,
                });
            }
            Some(_) => {}
            None => self.push_undeclared_error(read_stmt.var_name, read_stmt.span),
        }
    }

    fn check_return(&mut self, ret_stmt: &ReturnStmt<'_>) {
        let val_ty = ret_stmt.value.as_ref().and_then(|e| self.check_expr(e));

        if let Some(ctx) = &self.current_function {
            let expected = ctx.return_type;
            let func_name = ctx.name.clone();

            match (expected, &ret_stmt.value, val_ty) {
                (SemanticType::Vazio, Some(expr), Some(found)) => {
                    self.errors.push(SemanticError {
                        kind: SemanticErrorKind::ReturnMismatch {
                            function_name: func_name.clone(),
                            expected: SemanticType::Vazio,
                            found,
                        },
                        message: format!(
                            "a função '{func_name}' tem retorno 'vazio' e não pode retornar um valor do tipo '{found}'"
                        ),
                        span: expr.span(),
                    });
                }
                (expected_ty, None, _) if expected_ty != SemanticType::Vazio => {
                    self.errors.push(SemanticError {
                        kind: SemanticErrorKind::ReturnMismatch {
                            function_name: func_name.clone(),
                            expected: expected_ty,
                            found: SemanticType::Vazio,
                        },
                        message: format!(
                            "a função '{func_name}' deve retornar um valor do tipo '{expected_ty}', mas 'retorna;' foi usado sem expressão"
                        ),
                        span: ret_stmt.span,
                    });
                }
                (expected_ty, Some(expr), Some(found))
                    if expected_ty != SemanticType::Vazio && !found.is_assignable_to(expected_ty) =>
                {
                    self.errors.push(SemanticError {
                        kind: SemanticErrorKind::ReturnMismatch {
                            function_name: func_name.clone(),
                            expected: expected_ty,
                            found,
                        },
                        message: format!(
                            "tipo de retorno incompatível na função '{func_name}': esperado '{expected_ty}', mas foi retornado '{found}'"
                        ),
                        span: expr.span(),
                    });
                }
                _ => {}
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr<'_>) -> Option<SemanticType> {
        match expr {
            Expr::IntLiteral { .. } => Some(SemanticType::Int),
            Expr::DecLiteral { .. } => Some(SemanticType::Dec),
            Expr::BoolLiteral { .. } => Some(SemanticType::Bool),
            Expr::StringLiteral { content, .. } => {
                if content.chars().count() == 1 {
                    Some(SemanticType::Carac)
                } else {
                    Some(SemanticType::Texto)
                }
            }
            Expr::Grouping { expr: inner, .. } => self.check_expr(inner),
            Expr::Identifier { name, span } => match self.table.resolve_and_record(name, *span) {
                Some(sym) if matches!(sym.kind, SymbolKind::Function { .. }) => {
                    self.errors.push(SemanticError {
                        kind: SemanticErrorKind::InvalidSymbolKind {
                            name: (*name).to_string(),
                            expected: "variável",
                            found: "função",
                        },
                        message: format!("'{name}' é uma função e deve ser chamada com parênteses"),
                        span: *span,
                    });
                    None
                }
                Some(sym) => Some(sym.ty),
                None => {
                    self.push_undeclared_error(name, *span);
                    None
                }
            },
            Expr::Call(call) => self.check_function_call(call),
            Expr::Unary { op, operand, span } => {
                let operand_ty = self.check_expr(operand)?;
                match op {
                    UnaryOp::Neg if operand_ty.is_numeric() => Some(operand_ty),
                    UnaryOp::Not if operand_ty == SemanticType::Bool => Some(SemanticType::Bool),
                    _ => {
                        self.errors.push(SemanticError {
                            kind: SemanticErrorKind::InvalidOperation {
                                op: op.to_string(),
                                left: operand_ty,
                                right: None,
                            },
                            message: format!(
                                "operador unário '{op}' inválido para o tipo '{operand_ty}'"
                            ),
                            span: *span,
                        });
                        None
                    }
                }
            }
            Expr::Binary {
                left,
                op,
                right,
                span,
            } => {
                let left_ty = self.check_expr(left);
                let right_ty = self.check_expr(right);
                let (l_ty, r_ty) = (left_ty?, right_ty?);
                self.check_binary_types(*op, l_ty, r_ty, *span)
            }
        }
    }

    fn check_binary_types(
        &mut self,
        op: BinaryOp,
        l_ty: SemanticType,
        r_ty: SemanticType,
        span: Span,
    ) -> Option<SemanticType> {
        match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                if let Some(res) = l_ty.unify_numeric(r_ty) {
                    return Some(res);
                }
            }
            BinaryOp::Mod => {
                if matches!(l_ty, SemanticType::Int | SemanticType::Num)
                    && matches!(r_ty, SemanticType::Int | SemanticType::Num)
                {
                    return Some(SemanticType::Int);
                }
            }
            BinaryOp::LessThan
            | BinaryOp::GreaterThan
            | BinaryOp::LessEqual
            | BinaryOp::GreaterEqual => {
                if l_ty.unify_numeric(r_ty).is_some() {
                    return Some(SemanticType::Bool);
                }
            }
            BinaryOp::Equal | BinaryOp::NotEqual => {
                let compatible = l_ty.unify_numeric(r_ty).is_some()
                    || (l_ty == r_ty && l_ty != SemanticType::Vazio)
                    || l_ty.is_assignable_to(r_ty)
                    || r_ty.is_assignable_to(l_ty);
                if compatible {
                    return Some(SemanticType::Bool);
                }
            }
            BinaryOp::And | BinaryOp::Or => {
                if l_ty == SemanticType::Bool && r_ty == SemanticType::Bool {
                    return Some(SemanticType::Bool);
                }
            }
        }

        self.errors.push(SemanticError {
            kind: SemanticErrorKind::InvalidOperation {
                op: op.to_string(),
                left: l_ty,
                right: Some(r_ty),
            },
            message: format!("operação '{op}' inválida entre os tipos '{l_ty}' e '{r_ty}'"),
            span,
        });
        None
    }

    fn check_function_call(&mut self, call: &FunctionCall<'_>) -> Option<SemanticType> {
        let arg_types: Vec<Option<SemanticType>> =
            call.args.iter().map(|a| self.check_expr(a)).collect();

        match self.table.resolve_and_record(call.callee, call.span) {
            Some(sym) => match sym.kind {
                SymbolKind::Function { params } => {
                    if call.args.len() != params.len() {
                        self.errors.push(SemanticError {
                            kind: SemanticErrorKind::ArityMismatch {
                                callee: call.callee.to_string(),
                                expected: params.len(),
                                found: call.args.len(),
                            },
                            message: format!(
                                "a função '{}' espera {} argumento(s), mas recebeu {}",
                                call.callee,
                                params.len(),
                                call.args.len()
                            ),
                            span: call.span,
                        });
                    } else {
                        for (idx, (arg_expr, (arg_ty_opt, &expected_ty))) in call
                            .args
                            .iter()
                            .zip(arg_types.iter().zip(params.iter()))
                            .enumerate()
                        {
                            if let Some(found_ty) = *arg_ty_opt
                                && !found_ty.is_assignable_to(expected_ty) {
                                    self.errors.push(SemanticError {
                                        kind: SemanticErrorKind::TypeMismatch {
                                            expected: expected_ty,
                                            found: found_ty,
                                            context: format!(
                                                "argumento {} da chamada a '{}'",
                                                idx + 1,
                                                call.callee
                                            ),
                                        },
                                        message: format!(
                                            "tipo incompatível no {}º argumento de '{}': esperado '{expected_ty}', mas foi encontrado '{found_ty}'",
                                            idx + 1,
                                            call.callee
                                        ),
                                        span: arg_expr.span(),
                                    });
                                }
                        }
                    }
                    Some(sym.ty)
                }
                _ => {
                    self.errors.push(SemanticError {
                        kind: SemanticErrorKind::InvalidSymbolKind {
                            name: call.callee.to_string(),
                            expected: "função",
                            found: sym.kind.label(),
                        },
                        message: format!(
                            "'{}' não é uma função (foi declarado como {})",
                            call.callee,
                            sym.kind.label()
                        ),
                        span: call.span,
                    });
                    None
                }
            },
            None => {
                self.errors.push(SemanticError {
                    kind: SemanticErrorKind::UndeclaredIdentifier {
                        name: call.callee.to_string(),
                    },
                    message: format!("função '{}' não foi declarada", call.callee),
                    span: call.span,
                });
                None
            }
        }
    }

    fn push_undeclared_error(&mut self, name: &str, span: Span) {
        self.errors.push(SemanticError {
            kind: SemanticErrorKind::UndeclaredIdentifier {
                name: name.to_string(),
            },
            message: format!(
                "variável '{name}' não foi declarada neste escopo ({}) nem em escopos externos",
                self.table.current_scope_id()
            ),
            span,
        });
    }

    fn push_redeclaration_error(&mut self, name: &str, span: Span, existing: &Symbol) {
        self.errors.push(SemanticError {
            kind: SemanticErrorKind::Redeclaration {
                name: name.to_string(),
                scope: existing.scope_id,
                previous_span: existing.decl_span,
            },
            message: format!(
                "redeclaração de '{name}' no mesmo escopo ({}): já declarado anteriormente na linha {}:{}",
                existing.scope_id,
                existing.decl_span.start.line,
                existing.decl_span.start.column
            ),
            span,
        });
    }

    /// Refina o layout C de `num`, `dec`, `int` e `carac` em ponto fixo e valida `%` sobre decimais.
    fn refine_layouts_and_check_mod(&mut self, program: &Program<'_>) {
        let mut collector = LayoutCollector::new(&self.table);
        collector.visit_program(program);

        let mut num_reps: HashMap<(ScopeId, String), Option<NumRepresentation>> = HashMap::new();
        let mut carac_is_text = collector.carac_text_vars;
        let mut carac_is_read: HashMap<(ScopeId, String), bool> = HashMap::new();
        let mut func_param_keys: HashMap<String, Vec<(ScopeId, String)>> = HashMap::new();
        let mut func_return_text: HashMap<String, bool> = HashMap::new();

        for item in &program.items {
            if let ProgramItem::Function(func) = item
                && let Some(fn_scope) = self.table.scope_for_block(func.body.span) {
                    func_param_keys.insert(
                        func.name.to_string(),
                        func.params
                            .iter()
                            .map(|p| (fn_scope, p.name.to_string()))
                            .collect(),
                    );
                }
        }

        for scope in self.table.scopes() {
            for name in &scope.symbol_order {
                let sym = &scope.symbols[name];
                let key = (scope.id, name.clone());
                match sym.ty {
                    SemanticType::Int => {
                        num_reps.insert(key, Some(NumRepresentation::Int));
                    }
                    SemanticType::Dec => {
                        num_reps.insert(key, None);
                    }
                    SemanticType::Num => {
                        let init = matches!(
                            sym.kind,
                            SymbolKind::Parameter | SymbolKind::Function { .. }
                        )
                        .then_some(NumRepresentation::Double);
                        num_reps.insert(key, init);
                    }
                    SemanticType::Carac => {
                        carac_is_text.entry(key).or_insert(false);
                    }
                    _ => {}
                }
            }
        }

        for (key, sym_ty) in &collector.reads {
            if *sym_ty == SemanticType::Carac {
                carac_is_read.insert(key.clone(), true);
            }
        }

        for (callee, args) in &collector.call_sites {
            if let Some(param_keys) = func_param_keys.get(callee.as_str()) {
                for (arg_expr, p_key) in args.iter().zip(param_keys.iter()) {
                    if expr_contains_text(arg_expr, &carac_is_text, &self.table) {
                        carac_is_text.insert(p_key.clone(), true);
                    }
                }
            }
        }
        for (func_name, expr) in &collector.returns {
            if expr_contains_text(expr, &carac_is_text, &self.table) {
                func_return_text.insert(func_name.clone(), true);
            }
        }

        for _ in 0..num_reps.len().saturating_add(4) {
            let mut changed = false;
            for (target_key, target_ty, expr) in &collector.assignments {
                if !target_ty.is_numeric() {
                    continue;
                }
                let expr_rep = infer_expr_rep(expr, &num_reps, &self.table);
                let joined = match (target_ty, expr_rep) {
                    (SemanticType::Dec, Some(NumRepresentation::Int | NumRepresentation::Long))
                    | (SemanticType::Dec, None) => Some(NumRepresentation::Double),
                    (_, rep) => rep,
                };
                if let Some(new_rep) = joined {
                    let entry = num_reps.entry(target_key.clone()).or_insert(None);
                    let merged = entry.map_or(new_rep, |cur| cur.join(new_rep));
                    if *entry != Some(merged) {
                        *entry = Some(merged);
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }

        let updates: Vec<(ScopeId, String, &'static str, usize)> = self
            .table
            .scopes()
            .iter()
            .flat_map(|scope| {
                scope.symbol_order.iter().filter_map(|name| {
                    let sym = &scope.symbols[name];
                    let key = (scope.id, name.clone());
                    match sym.ty {
                        SemanticType::Int | SemanticType::Dec | SemanticType::Num => {
                            let rep = num_reps.get(&key).copied().flatten().unwrap_or(
                                if sym.ty == SemanticType::Int {
                                    NumRepresentation::Int
                                } else {
                                    NumRepresentation::Double
                                },
                            );
                            Some((scope.id, name.clone(), rep.c_type(), rep.bytes()))
                        }
                        SemanticType::Carac => {
                            let is_text = if matches!(sym.kind, SymbolKind::Function { .. }) {
                                func_return_text.get(name).copied().unwrap_or(false)
                            } else {
                                carac_is_text.get(&key).copied().unwrap_or(false)
                            };
                            let is_read = carac_is_read.get(&key).copied().unwrap_or(false);
                            let (c_type, size_bytes) = match (is_text, is_read) {
                                (true, true) => ("char[256]", 256),
                                (true, false) => ("const char*", 8),
                                (false, _) => ("char", 1),
                            };
                            Some((scope.id, name.clone(), c_type, size_bytes))
                        }
                        _ => None,
                    }
                })
            })
            .collect();

        for (left, right, span) in &collector.mod_checks {
            let is_dec = |e: &Expr<'_>| {
                matches!(
                    infer_expr_rep(e, &num_reps, &self.table),
                    Some(NumRepresentation::Float | NumRepresentation::Double)
                )
            };
            let (l_dec, r_dec) = (is_dec(left), is_dec(right));
            if l_dec || r_dec {
                self.errors.push(SemanticError {
                    kind: SemanticErrorKind::InvalidOperation {
                        op: "%".to_string(),
                        left: if l_dec {
                            SemanticType::Dec
                        } else {
                            SemanticType::Int
                        },
                        right: Some(if r_dec {
                            SemanticType::Dec
                        } else {
                            SemanticType::Int
                        }),
                    },
                    message: "o operador de resto '%' não pode ser usado com valores decimais ('num' inferido como decimal)".to_string(),
                    span: *span,
                });
            }
        }

        for (scope_id, name, c_type, size_bytes) in updates {
            self.table
                .update_symbol_layout(scope_id, &name, c_type, size_bytes);
        }
    }
}

struct LayoutCollector<'a, 'src> {
    table: &'a SymbolTable,
    assignments: Vec<((ScopeId, String), SemanticType, &'a Expr<'src>)>,
    reads: Vec<((ScopeId, String), SemanticType)>,
    mod_checks: Vec<(&'a Expr<'src>, &'a Expr<'src>, Span)>,
    call_sites: Vec<(String, Vec<&'a Expr<'src>>)>,
    returns: Vec<(String, &'a Expr<'src>)>,
    carac_text_vars: HashMap<(ScopeId, String), bool>,
}

impl<'a, 'src> LayoutCollector<'a, 'src> {
    fn new(table: &'a SymbolTable) -> Self {
        Self {
            table,
            assignments: Vec::new(),
            reads: Vec::new(),
            mod_checks: Vec::new(),
            call_sites: Vec::new(),
            returns: Vec::new(),
            carac_text_vars: HashMap::new(),
        }
    }

    fn visit_program(&mut self, program: &'a Program<'src>) {
        for item in &program.items {
            match item {
                ProgramItem::Function(func) => {
                    for stmt in &func.body.statements {
                        self.visit_stmt(stmt, Some(func.name));
                    }
                }
                ProgramItem::Statement(stmt) => self.visit_stmt(stmt, None),
            }
        }
    }

    fn record_var_decl(&mut self, decl: &'a VarDecl<'src>) {
        if let Some(init) = &decl.init {
            if let Some(scope_id) = self.table.scope_for_decl(decl.span)
                && let Some(sym) = self.table.symbol(scope_id, decl.name) {
                    self.assignments
                        .push(((scope_id, decl.name.to_string()), sym.ty, init));
                    if sym.ty == SemanticType::Carac
                        && expr_contains_text(init, &self.carac_text_vars, self.table)
                    {
                        self.carac_text_vars
                            .insert((scope_id, decl.name.to_string()), true);
                    }
                }
            self.visit_expr(init);
        }
    }

    fn record_assignment(&mut self, assign: &'a Assignment<'src>) {
        if let Some(r) = self.table.find_reference_at(assign.span) {
            self.assignments.push((
                (r.resolved_scope, assign.name.to_string()),
                r.ty,
                &assign.value,
            ));
            if r.ty == SemanticType::Carac
                && expr_contains_text(&assign.value, &self.carac_text_vars, self.table)
            {
                self.carac_text_vars
                    .insert((r.resolved_scope, assign.name.to_string()), true);
            }
        }
        self.visit_expr(&assign.value);
    }

    fn visit_stmt(&mut self, stmt: &'a Statement<'src>, func_name: Option<&str>) {
        match stmt {
            Statement::VarDecl(decl) => self.record_var_decl(decl),
            Statement::Assignment(assign) => self.record_assignment(assign),
            Statement::If(if_stmt) => {
                self.visit_expr(&if_stmt.condition);
                for s in &if_stmt.then_branch.statements {
                    self.visit_stmt(s, func_name);
                }
                if let Some(eb) = &if_stmt.else_branch {
                    for s in &eb.statements {
                        self.visit_stmt(s, func_name);
                    }
                }
            }
            Statement::While(while_stmt) => {
                self.visit_expr(&while_stmt.condition);
                for s in &while_stmt.body.statements {
                    self.visit_stmt(s, func_name);
                }
            }
            Statement::For(for_stmt) => {
                if let Some(init) = &for_stmt.init {
                    match init {
                        ForInit::Assignment(a) => self.record_assignment(a),
                        ForInit::VarDecl(d) => self.record_var_decl(d),
                    }
                }
                if let Some(cond) = &for_stmt.condition {
                    self.visit_expr(cond);
                }
                if let Some(inc) = &for_stmt.increment {
                    self.record_assignment(inc);
                }
                for s in &for_stmt.body.statements {
                    self.visit_stmt(s, func_name);
                }
            }
            Statement::Print(p) => {
                for arg in &p.args {
                    self.visit_expr(arg);
                }
            }
            Statement::Read(r) => {
                if let Some(ref_info) = self.table.find_reference_at(r.span) {
                    self.reads
                        .push(((ref_info.resolved_scope, r.var_name.to_string()), ref_info.ty));
                }
            }
            Statement::Return(ret) => {
                if let Some(val) = &ret.value {
                    if let Some(fn_name) = func_name {
                        self.returns.push((fn_name.to_string(), val));
                    }
                    self.visit_expr(val);
                }
            }
            Statement::FunctionCall(call) => {
                self.call_sites
                    .push((call.callee.to_string(), call.args.iter().collect()));
                for arg in &call.args {
                    self.visit_expr(arg);
                }
            }
            Statement::Block(b) => {
                for s in &b.statements {
                    self.visit_stmt(s, func_name);
                }
            }
            Statement::Break(_) | Statement::Continue(_) => {}
        }
    }

    fn visit_expr(&mut self, expr: &'a Expr<'src>) {
        match expr {
            Expr::Unary { operand, .. } => self.visit_expr(operand),
            Expr::Grouping { expr, .. } => self.visit_expr(expr),
            Expr::Binary {
                left,
                op,
                right,
                span,
            } => {
                if *op == BinaryOp::Mod {
                    self.mod_checks.push((left, right, *span));
                }
                self.visit_expr(left);
                self.visit_expr(right);
            }
            Expr::Call(call) => {
                self.call_sites
                    .push((call.callee.to_string(), call.args.iter().collect()));
                for arg in &call.args {
                    self.visit_expr(arg);
                }
            }
            _ => {}
        }
    }
}

fn expr_contains_text(
    expr: &Expr<'_>,
    carac_text_vars: &HashMap<(ScopeId, String), bool>,
    table: &SymbolTable,
) -> bool {
    match expr {
        Expr::StringLiteral { content, .. } => content.chars().count() != 1,
        Expr::Grouping { expr, .. } => expr_contains_text(expr, carac_text_vars, table),
        Expr::Identifier { name, span } => table.find_reference_at(*span).is_some_and(|r| {
            r.ty == SemanticType::Texto
                || carac_text_vars
                    .get(&(r.resolved_scope, (*name).to_string()))
                    .copied()
                    .unwrap_or(false)
        }),
        Expr::Call(call) => table
            .find_reference_at(call.span)
            .is_some_and(|r| r.ty == SemanticType::Texto || r.c_type == "const char*"),
        _ => false,
    }
}

fn infer_expr_rep(
    expr: &Expr<'_>,
    num_reps: &HashMap<(ScopeId, String), Option<NumRepresentation>>,
    table: &SymbolTable,
) -> Option<NumRepresentation> {
    let compiler = NumCompiler::new();
    match expr {
        Expr::IntLiteral { raw, span } => {
            let tok = Token {
                kind: TokenKind::NumInt,
                lexeme: raw,
                span: *span,
            };
            compiler.lower_literal(&tok).ok().map(|c| c.representation)
        }
        Expr::DecLiteral { raw, span } => {
            let tok = Token {
                kind: TokenKind::NumDec,
                lexeme: raw,
                span: *span,
            };
            compiler.lower_literal(&tok).ok().map(|c| c.representation)
        }
        Expr::Unary {
            op: UnaryOp::Neg,
            operand,
            ..
        } => infer_expr_rep(operand, num_reps, table),
        Expr::Grouping { expr, .. } => infer_expr_rep(expr, num_reps, table),
        Expr::Binary {
            left,
            op: BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod,
            right,
            ..
        } => match (
            infer_expr_rep(left, num_reps, table),
            infer_expr_rep(right, num_reps, table),
        ) {
            (Some(l), Some(r)) => Some(l.join(r)),
            (l, r) => l.or(r),
        },
        Expr::Identifier { name, span } => table.find_reference_at(*span).and_then(|r| {
            num_reps
                .get(&(r.resolved_scope, (*name).to_string()))
                .copied()
                .flatten()
                .or(match r.ty {
                    SemanticType::Int => Some(NumRepresentation::Int),
                    SemanticType::Dec | SemanticType::Num => Some(NumRepresentation::Double),
                    _ => None,
                })
        }),
        Expr::Call(call) => table.find_reference_at(call.span).and_then(|r| match r.ty {
            SemanticType::Int => Some(NumRepresentation::Int),
            SemanticType::Dec | SemanticType::Num => Some(NumRepresentation::Double),
            _ => None,
        }),
        _ => None,
    }
}

/// Ponto de entrada para analisar semanticamente uma AST já construída.
pub fn analyze_semantics(program: &Program<'_>) -> Result<SymbolTable, Vec<SemanticError>> {
    SemanticAnalyzer::new().analyze(program)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse, tokenize};

    fn analyze_str(src: &str) -> Result<SymbolTable, Vec<SemanticError>> {
        let tokens = tokenize(src).expect("erro léxico inesperado no teste");
        let program = parse(&tokens).expect("erro sintático inesperado no teste");
        analyze_semantics(&program)
    }

    #[test]
    fn test_slide_escopos_aninhados_b1_b2_b3_b4() {
        let code = include_str!("../examples/escopos.laila");
        let table = analyze_str(code).expect("deveria passar na análise semântica");
        assert_eq!(table.scopes().len(), 5);

        let refs = table.references();
        assert_eq!(refs.len(), 8);
        assert_eq!((refs[0].use_scope, refs[0].resolved_scope), (ScopeId(3), ScopeId(3)));
        assert_eq!((refs[1].use_scope, refs[1].resolved_scope), (ScopeId(3), ScopeId(2)));
        assert_eq!((refs[2].use_scope, refs[2].resolved_scope), (ScopeId(4), ScopeId(1)));
        assert_eq!((refs[3].use_scope, refs[3].resolved_scope), (ScopeId(4), ScopeId(4)));
        assert_eq!((refs[4].use_scope, refs[4].resolved_scope), (ScopeId(2), ScopeId(1)));
        assert_eq!((refs[5].use_scope, refs[5].resolved_scope), (ScopeId(2), ScopeId(2)));
        assert_eq!((refs[6].use_scope, refs[6].resolved_scope), (ScopeId(1), ScopeId(1)));
        assert_eq!((refs[7].use_scope, refs[7].resolved_scope), (ScopeId(1), ScopeId(1)));
    }

    #[test]
    fn test_shadowing_muda_tipo_em_bloco_interno() {
        let code = "vazio principal() { int y = 10; { dec y = 3,14; y = 7,00; } y = 20; }";
        assert!(analyze_str(code).is_ok());
    }

    #[test]
    fn test_erro_variavel_nao_declarada() {
        let errs = analyze_str("vazio principal() { taxa = 40; }").unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(matches!(
            errs[0].kind,
            SemanticErrorKind::UndeclaredIdentifier { ref name } if name == "taxa"
        ));
    }

    #[test]
    fn test_erro_variavel_fora_do_escopo_interno() {
        let errs =
            analyze_str("vazio principal() { { int interna = 5; } imprima(interna); }").unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(matches!(
            errs[0].kind,
            SemanticErrorKind::UndeclaredIdentifier { ref name } if name == "interna"
        ));
    }

    #[test]
    fn test_erro_redeclaracao_mesmo_bloco() {
        let errs = analyze_str("vazio principal() { int a = 1; int a = 2; }").unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(matches!(
            errs[0].kind,
            SemanticErrorKind::Redeclaration { ref name, scope: ScopeId(1), .. } if name == "a"
        ));
    }

    #[test]
    fn test_erro_redeclaracao_parametro_no_corpo_da_funcao() {
        let errs = analyze_str("int dobro(int x) { int x = 10; retorna x; }").unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(matches!(
            errs[0].kind,
            SemanticErrorKind::Redeclaration { ref name, .. } if name == "x"
        ));
    }

    #[test]
    fn test_erro_incompatibilidade_de_tipos_int_e_dec() {
        let errs = analyze_str("vazio principal() { int x = 3,14; dec y = 10; }").unwrap_err();
        assert_eq!(errs.len(), 2);
        assert!(matches!(
            errs[0].kind,
            SemanticErrorKind::TypeMismatch {
                expected: SemanticType::Int,
                found: SemanticType::Dec,
                ..
            }
        ));
        assert!(matches!(
            errs[1].kind,
            SemanticErrorKind::TypeMismatch {
                expected: SemanticType::Dec,
                found: SemanticType::Int,
                ..
            }
        ));
    }

    #[test]
    fn test_tipo_num_compativel_com_int_e_dec() {
        let code = "vazio principal() { num a = 10; num b = 8,50; int i = a; dec d = b; }";
        assert!(analyze_str(code).is_ok());
    }

    #[test]
    fn test_erro_condicao_nao_booleana() {
        let errs =
            analyze_str("vazio principal() { int x = 10; se (x + 5) { imprima(x); } }").unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(matches!(
            errs[0].kind,
            SemanticErrorKind::NonBooleanCondition {
                found: SemanticType::Int,
                construct: "se"
            }
        ));
    }

    #[test]
    fn test_chamada_funcao_e_retorno() {
        let code =
            "int soma(int a, int b) { retorna a + b; } vazio principal() { int r = soma(10, 20); imprima(r); }";
        assert!(analyze_str(code).is_ok());
    }

    #[test]
    fn test_erro_retorno_incompativel() {
        let errs = analyze_str("int calcula() { retorna 3,14; }").unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(matches!(
            errs[0].kind,
            SemanticErrorKind::ReturnMismatch {
                expected: SemanticType::Int,
                found: SemanticType::Dec,
                ..
            }
        ));
    }

    #[test]
    fn test_num_promocao_automatica_entre_atribuicoes() {
        let code = "vazio principal() { num x = 10; x = 3,50; num y = -5; num z = y + 1; }";
        let table = analyze_str(code).expect("deveria inferir tipos automaticamente");
        let b1 = table.scope(ScopeId(1));
        assert_eq!((b1.symbols["x"].c_type, b1.symbols["x"].size_bytes), ("float", 4));
        assert_eq!((b1.symbols["y"].c_type, b1.symbols["y"].size_bytes), ("int", 4));
        assert_eq!((b1.symbols["z"].c_type, b1.symbols["z"].size_bytes), ("int", 4));
    }

    #[test]
    fn test_erro_modulo_com_num_promovido_a_decimal() {
        let errs = analyze_str("vazio principal() { num x = 10; x = 2,50; num r = x % 2; }")
            .unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(matches!(
            errs[0].kind,
            SemanticErrorKind::InvalidOperation { .. }
        ));
    }

    #[test]
    fn test_carac_letra_vs_texto() {
        let table =
            analyze_str("vazio principal() { carac letra = \"A\"; carac palavra = \"azul\"; }")
                .expect("carac válido para char e texto");
        let b1 = table.scope(ScopeId(1));
        assert_eq!((b1.symbols["letra"].c_type, b1.symbols["letra"].size_bytes), ("char", 1));
        assert_eq!(
            (b1.symbols["palavra"].c_type, b1.symbols["palavra"].size_bytes),
            ("const char*", 8)
        );
    }
}
