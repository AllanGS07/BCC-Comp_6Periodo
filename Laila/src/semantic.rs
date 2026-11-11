use crate::ast::*;
use crate::number::{NumCompiler, NumRepresentation};
use crate::{Span, Token, TokenKind};
use std::collections::HashMap;
use std::fmt;

/// Identificador único de um escopo (bloco) na Tabela de Símbolos (ex.: B0, B1, B2...).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopeId(pub usize);

impl fmt::Display for ScopeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "B{}", self.0)
    }
}

/// Tipos semânticos avaliados pelo analisador semântico da linguagem Laila.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticType {
    Int,
    Dec,
    Carac,
    Num,
    Bool,
    Texto,
    Vazio,
}

impl SemanticType {
    pub const fn from_ast_type(ty: Type) -> Self {
        match ty {
            Type::Int => Self::Int,
            Type::Dec => Self::Dec,
            Type::Carac => Self::Carac,
            Type::Num => Self::Num,
            Type::Vazio => Self::Vazio,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Dec => "dec",
            Self::Carac => "carac",
            Self::Num => "num",
            Self::Bool => "bool",
            Self::Texto => "texto",
            Self::Vazio => "vazio",
        }
    }

    pub const fn is_numeric(self) -> bool {
        matches!(self, Self::Int | Self::Dec | Self::Num)
    }

    /// Verifica se um valor do tipo `value_ty` pode ser atribuído a um alvo do tipo `target_ty`.
    ///
    /// Segue a regra de tipagem estrita entre `int` e `dec`, com `num` atuando como tipo
    /// numérico flexível compatível com `int` e `dec`, e `carac` compatível com literal de texto.
    pub const fn is_assignable_to(self, target: Self) -> bool {
        match (target, self) {
            (Self::Int, Self::Int | Self::Num) => true,
            (Self::Dec, Self::Dec | Self::Num) => true,
            (Self::Num, Self::Int | Self::Dec | Self::Num) => true,
            (Self::Carac, Self::Carac | Self::Texto) => true,
            (Self::Bool, Self::Bool) => true,
            (Self::Texto, Self::Texto | Self::Carac) => true,
            _ => false,
        }
    }

    /// Verifica se dois tipos numéricos são compatíveis em uma expressão binária e retorna o tipo resultante.
    pub const fn unify_numeric(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Self::Int, Self::Int) => Some(Self::Int),
            (Self::Dec, Self::Dec) => Some(Self::Dec),
            (Self::Num, Self::Int) | (Self::Int, Self::Num) => Some(Self::Int),
            (Self::Num, Self::Dec) | (Self::Dec, Self::Num) => Some(Self::Dec),
            (Self::Num, Self::Num) => Some(Self::Num),
            _ => None,
        }
    }
}

impl fmt::Display for SemanticType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Categoria do símbolo armazenado na Tabela de Símbolos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    /// Variável local ou global.
    Variable,
    /// Parâmetro formal de função.
    Parameter,
    /// Declaração de função com os tipos de seus parâmetros.
    Function { params: Vec<SemanticType> },
}

impl SymbolKind {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Variable => "variável",
            Self::Parameter => "parâmetro",
            Self::Function { .. } => "função",
        }
    }
}

/// Entrada individual na Tabela de Símbolos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub ty: SemanticType,
    /// Tipo equivalente na linguagem C (ex.: `int`, `long long`, `float`, `double`, `char`, `void`).
    pub c_type: &'static str,
    /// Tamanho em bytes na representação alvo em C.
    pub size_bytes: usize,
    pub kind: SymbolKind,
    pub scope_id: ScopeId,
    pub decl_span: Span,
    /// Posições onde este símbolo específico foi referenciado no código.
    pub references: Vec<Span>,
}

/// Registro de resolução de referência de um identificador (Uso -> Declaração).
/// Responde diretamente à pergunta:
/// "Se um nome aparece em diferentes blocos, a qual declaração ele se refere?"
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedReference {
    pub name: String,
    pub use_span: Span,
    pub use_scope: ScopeId,
    pub resolved_scope: ScopeId,
    pub decl_span: Span,
    pub ty: SemanticType,
    pub c_type: &'static str,
    pub kind: SymbolKind,
}

/// Um nó de escopo (bloco B0, B1, B2...) na árvore da Tabela de Símbolos.
#[derive(Debug, Clone)]
pub struct Scope {
    pub id: ScopeId,
    pub parent: Option<ScopeId>,
    pub children: Vec<ScopeId>,
    pub description: String,
    /// Ordem de inserção preservada para exibição fiel da tabela do bloco.
    pub symbol_order: Vec<String>,
    pub symbols: HashMap<String, Symbol>,
}

impl Scope {
    pub fn new(id: ScopeId, parent: Option<ScopeId>, description: impl Into<String>) -> Self {
        Self {
            id,
            parent,
            children: Vec::new(),
            description: description.into(),
            symbol_order: Vec::new(),
            symbols: HashMap::new(),
        }
    }
}

/// Tabela de Símbolos Hierárquica (árvore de escopos B0, B1, B2...).
///
/// Implementa o encadeamento de blocos aninhados (como em C):
/// - Declarações são inseridas no escopo atual (`current_scope`).
/// - Buscas (`lookup`) iniciam no escopo atual e sobem pela cadeia de pais (`parent`)
///   até o escopo global `B0`, garantindo que declarações em blocos internos ocultem
///   (*shadowing*) variáveis homônimas de blocos externos.
#[derive(Debug, Clone)]
pub struct SymbolTable {
    scopes: Vec<Scope>,
    current_scope: ScopeId,
    references: Vec<ResolvedReference>,
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolTable {
    /// Cria uma nova Tabela de Símbolos contendo o escopo global inicial `B0`.
    pub fn new() -> Self {
        let global = Scope::new(ScopeId(0), None, "escopo global");
        Self {
            scopes: vec![global],
            current_scope: ScopeId(0),
            references: Vec::new(),
        }
    }

    /// Retorna o identificador do escopo ativo no momento.
    #[inline]
    pub fn current_scope_id(&self) -> ScopeId {
        self.current_scope
    }

    /// Retorna todos os escopos criados ($B_0, B_1, \dots$).
    pub fn scopes(&self) -> &[Scope] {
        &self.scopes
    }

    /// Retorna um escopo específico pelo seu `ScopeId`.
    pub fn scope(&self, id: ScopeId) -> &Scope {
        &self.scopes[id.0]
    }

    /// Retorna a lista de todas as referências resolvidas (usos de identificadores).
    pub fn references(&self) -> &[ResolvedReference] {
        &self.references
    }

    /// Entra em um novo escopo filho (ex.: ao abrir um novo bloco `{ ... }`),
    /// criando um novo nó $B_k$ ligado ao escopo atual.
    pub fn enter_scope(&mut self, description: impl Into<String>) -> ScopeId {
        let new_id = ScopeId(self.scopes.len());
        let parent_id = self.current_scope;
        self.scopes[parent_id.0].children.push(new_id);
        self.scopes
            .push(Scope::new(new_id, Some(parent_id), description));
        self.current_scope = new_id;
        new_id
    }

    /// Sai do escopo atual e retorna ao escopo pai (ex.: ao fechar `}`).
    pub fn exit_scope(&mut self) {
        if let Some(parent) = self.scopes[self.current_scope.0].parent {
            self.current_scope = parent;
        }
    }

    /// Consulta um nome **apenas** no escopo atual (para detectar redeclaração no mesmo bloco).
    pub fn lookup_local(&self, name: &str) -> Option<&Symbol> {
        self.scopes[self.current_scope.0].symbols.get(name)
    }

    /// Consulta um nome a partir do escopo atual subindo pela cadeia de blocos pais ($B_k \to \dots \to B_0$).
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        let mut cursor = Some(self.current_scope);
        while let Some(scope_id) = cursor {
            let scope = &self.scopes[scope_id.0];
            if let Some(sym) = scope.symbols.get(name) {
                return Some(sym);
            }
            cursor = scope.parent;
        }
        None
    }

    /// Consulta um nome a partir do escopo atual e registra a referência de uso (`use_span`)
    /// no símbolo encontrado.
    pub fn resolve_and_record(&mut self, name: &str, use_span: Span) -> Option<Symbol> {
        let use_scope = self.current_scope;
        let mut cursor = Some(use_scope);
        while let Some(scope_id) = cursor {
            if let Some(sym) = self.scopes[scope_id.0].symbols.get_mut(name) {
                sym.references.push(use_span);
                let cloned = sym.clone();
                self.references.push(ResolvedReference {
                    name: name.to_string(),
                    use_span,
                    use_scope,
                    resolved_scope: cloned.scope_id,
                    decl_span: cloned.decl_span,
                    ty: cloned.ty,
                    c_type: cloned.c_type,
                    kind: cloned.kind.clone(),
                });
                return Some(cloned);
            }
            cursor = self.scopes[scope_id.0].parent;
        }
        None
    }

    /// Busca a resolução registrada para um uso específico de identificador pela sua posição (`Span`).
    pub fn find_reference_at(&self, use_span: Span) -> Option<&ResolvedReference> {
        self.references.iter().find(|r| r.use_span == use_span)
    }

    /// Insere um novo símbolo no escopo atual com tipo C padrão derivado de `ty`.
    pub fn insert(
        &mut self,
        name: impl Into<String>,
        ty: SemanticType,
        kind: SymbolKind,
        decl_span: Span,
    ) -> Result<ScopeId, Symbol> {
        let (c_type, size_bytes) = default_c_layout(ty);
        self.insert_with_c_type(name, ty, c_type, size_bytes, kind, decl_span)
    }

    /// Insere um novo símbolo no escopo atual especificando também sua representação em C.
    /// Retorna `Err(existing_symbol)` caso o identificador já tenha sido declarado neste mesmo escopo.
    pub fn insert_with_c_type(
        &mut self,
        name: impl Into<String>,
        ty: SemanticType,
        c_type: &'static str,
        size_bytes: usize,
        kind: SymbolKind,
        decl_span: Span,
    ) -> Result<ScopeId, Symbol> {
        let name = name.into();
        let scope_id = self.current_scope;
        let scope = &mut self.scopes[scope_id.0];

        if let Some(existing) = scope.symbols.get(&name) {
            return Err(existing.clone());
        }

        let symbol = Symbol {
            name: name.clone(),
            ty,
            c_type,
            size_bytes,
            kind,
            scope_id,
            decl_span,
            references: Vec::new(),
        };

        scope.symbol_order.push(name.clone());
        scope.symbols.insert(name, symbol);
        Ok(scope_id)
    }

    /// Formata a Tabela de Símbolos completa em formato visual hierárquico,
    /// exibindo os blocos $B_0, B_1, B_2, \dots$ e a resolução de referências.
    pub fn format_table(&self) -> String {
        let mut out = String::new();
        out.push_str("========================================================================\n");
        out.push_str("                    TABELA DE SÍMBOLOS (POR ESCOPO)                     \n");
        out.push_str("========================================================================\n\n");

        self.render_scope_tree(ScopeId(0), "", true, true, &mut out);

        out.push_str("\n========================================================================\n");
        out.push_str("            RESOLUÇÃO DE REFERÊNCIAS (USO -> DECLARAÇÃO)                \n");
        out.push_str("========================================================================\n");

        if self.references.is_empty() {
            out.push_str("  (nenhum uso de identificador registrado)\n");
        } else {
            for r in &self.references {
                out.push_str(&format!(
                    "  [linha {}:{}] '{}' (usado em {}) -> refere-se a {} '{}' : {} declarada em {} (linha {}:{})\n",
                    r.use_span.start.line,
                    r.use_span.start.column,
                    r.name,
                    r.use_scope,
                    r.kind.label(),
                    r.name,
                    r.ty,
                    r.resolved_scope,
                    r.decl_span.start.line,
                    r.decl_span.start.column,
                ));
            }
        }

        out
    }

    fn render_scope_tree(
        &self,
        scope_id: ScopeId,
        prefix: &str,
        is_last: bool,
        is_root: bool,
        out: &mut String,
    ) {
        let scope = &self.scopes[scope_id.0];
        let branch = if is_root {
            ""
        } else if is_last {
            "└── "
        } else {
            "├── "
        };

        let parent_info = match scope.parent {
            Some(p) => format!("pai: {p}"),
            None => "raiz".to_string(),
        };

        out.push_str(&format!(
            "{prefix}{branch}{}: {} ({parent_info})\n",
            scope.id, scope.description
        ));

        let child_prefix = if is_root {
            ""
        } else if is_last {
            "    "
        } else {
            "│   "
        };
        let table_prefix = format!("{prefix}{child_prefix}");

        if scope.symbol_order.is_empty() {
            out.push_str(&format!("{table_prefix}  [vazio]\n"));
        } else {
            out.push_str(&format!(
                "{table_prefix}  ┌──────────────────┬──────────┬──────────────────┬──────────────┬────────────┬──────────────────────┐\n"
            ));
            out.push_str(&format!(
                "{table_prefix}  │ {:<16} │ {:<8} │ {:<16} │ {:<12} │ {:<10} │ {:<20} │\n",
                "Identificador", "Tipo", "Tipo C (Bytes)", "Categoria", "Declaração", "Referências (usos)"
            ));
            out.push_str(&format!(
                "{table_prefix}  ├──────────────────┼──────────┼──────────────────┼──────────────┼────────────┼──────────────────────┤\n"
            ));

            for name in &scope.symbol_order {
                let sym = &scope.symbols[name];
                let decl_pos = format!("{}:{}", sym.decl_span.start.line, sym.decl_span.start.column);
                let c_info = if sym.size_bytes > 0 {
                    format!("{} ({}B)", sym.c_type, sym.size_bytes)
                } else {
                    sym.c_type.to_string()
                };
                let refs_str = if sym.references.is_empty() {
                    "-".to_string()
                } else {
                    sym.references
                        .iter()
                        .map(|s| format!("{}:{}", s.start.line, s.start.column))
                        .collect::<Vec<_>>()
                        .join(", ")
                };

                out.push_str(&format!(
                    "{table_prefix}  │ {:<16} │ {:<8} │ {:<16} │ {:<12} │ {:<10} │ {:<20} │\n",
                    sym.name,
                    sym.ty.as_str(),
                    c_info,
                    sym.kind.label(),
                    decl_pos,
                    refs_str
                ));
            }

            out.push_str(&format!(
                "{table_prefix}  └──────────────────┴──────────┴──────────────────┴──────────────┴────────────┴──────────────────────┘\n"
            ));
        }

        for (i, &child_id) in scope.children.iter().enumerate() {
            let child_is_last = i + 1 == scope.children.len();
            self.render_scope_tree(child_id, &table_prefix, child_is_last, false, out);
        }
    }
}

/// Categorias de erros semânticos detectados na linguagem Laila.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticErrorKind {
    /// Identificador (variável ou função) utilizado sem ter sido declarado em nenhum escopo visível.
    UndeclaredIdentifier { name: String },
    /// Identificador redeclarado dentro do mesmo escopo/bloco.
    Redeclaration {
        name: String,
        scope: ScopeId,
        previous_span: Span,
    },
    /// Atribuição, inicialização ou operação entre tipos incompatíveis.
    TypeMismatch {
        expected: SemanticType,
        found: SemanticType,
        context: String,
    },
    /// Operação binária ou unária inválida para os tipos fornecidos.
    InvalidOperation {
        op: String,
        left: SemanticType,
        right: Option<SemanticType>,
    },
    /// Condição de controle (`se`, `enquanto`, `para`) não resulta em tipo lógico (`bool`).
    NonBooleanCondition {
        found: SemanticType,
        construct: &'static str,
    },
    /// Tentativa de chamar uma variável comum como se fosse função, ou usar função como variável.
    InvalidSymbolKind {
        name: String,
        expected: &'static str,
        found: &'static str,
    },
    /// Quantidade incorreta de argumentos em uma chamada de função.
    ArityMismatch {
        callee: String,
        expected: usize,
        found: usize,
    },
    /// Comando `retorna` incompatível com o tipo de retorno da função.
    ReturnMismatch {
        function_name: String,
        expected: SemanticType,
        found: SemanticType,
    },
    /// Uso de `pare` ou `continue` fora de um laço (`enquanto` / `para`).
    LoopControlOutsideLoop { keyword: &'static str },
    /// Declaração de variável com tipo `vazio`.
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

/// Contexto interno da função em análise (para validar instruções `retorna`).
#[derive(Debug, Clone)]
struct FunctionContext {
    name: String,
    return_type: SemanticType,
}

/// Analisador Semântico da linguagem Laila.
///
/// Percorre a AST realizando:
/// 1. Construção da Tabela de Símbolos com escopos aninhados ($B_0, B_1, B_2, \dots$) seguindo as regras de C.
/// 2. Verificação de declaração prévia de variáveis e funções (`UndeclaredIdentifier`, `Redeclaration`).
/// 3. Resolução de escopo e vinculação de cada uso de identificador à sua declaração mais interna visível.
/// 4. Checagem estrita de compatibilidade de tipos em declarações, atribuições, expressões, condições e retornos.
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

    /// Retorna os erros semânticos acumulados durante a análise.
    pub fn errors(&self) -> &[SemanticError] {
        &self.errors
    }

    /// Analisa um programa completo e retorna a Tabela de Símbolos construída
    /// ou a lista de erros semânticos encontrados.
    pub fn analyze(mut self, program: &Program<'_>) -> Result<SymbolTable, Vec<SemanticError>> {
        self.check_program(program);
        if self.errors.is_empty() {
            Ok(self.table)
        } else {
            Err(self.errors)
        }
    }

    /// Executa a análise semântica sobre o programa (acumulando todos os erros).
    pub fn check_program(&mut self, program: &Program<'_>) {
        for item in &program.items {
            match item {
                ProgramItem::Function(func) => self.check_function(func),
                ProgramItem::Statement(stmt) => self.check_statement(stmt),
            }
        }
    }

    fn check_function(&mut self, func: &Function<'_>) {
        let ret_ty = SemanticType::from_ast_type(func.return_type);
        let param_types: Vec<SemanticType> = func
            .params
            .iter()
            .map(|p| SemanticType::from_ast_type(p.param_type))
            .collect();

        // 1. Registra a função no escopo atual (global B0)
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

        // 2. Cria o escopo do corpo da função (ex.: B1).
        // Em C, os parâmetros formais pertencem ao escopo principal do corpo da função.
        self.table
            .enter_scope(format!("função '{}'", func.name));

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

        // Analisa os comandos diretamente no escopo da função (como em C, onde os parâmetros
        // e as variáveis locais do bloco principal da função dividem o mesmo escopo B_k).
        for stmt in &func.body.statements {
            self.check_statement(stmt);
        }

        self.current_function = prev_func;
        self.table.exit_scope();
    }

    fn check_block(&mut self, block: &Block<'_>, description: impl Into<String>) {
        self.table.enter_scope(description);
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
            Statement::Break(span) => {
                if self.loop_depth == 0 {
                    self.errors.push(SemanticError {
                        kind: SemanticErrorKind::LoopControlOutsideLoop { keyword: "pare" },
                        message: "o comando 'pare' só pode ser utilizado dentro de um laço ('enquanto' ou 'para')".to_string(),
                        span: *span,
                    });
                }
            }
            Statement::Continue(span) => {
                if self.loop_depth == 0 {
                    self.errors.push(SemanticError {
                        kind: SemanticErrorKind::LoopControlOutsideLoop {
                            keyword: "continue",
                        },
                        message: "o comando 'continue' só pode ser utilizado dentro de um laço ('enquanto' ou 'para')".to_string(),
                        span: *span,
                    });
                }
            }
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

        // Avalia a expressão inicializadora e infere a representação em C (ex.: float 4B vs double 8B vs int 4B)
        let init_ty = decl.init.as_ref().and_then(|expr| self.check_expr(expr));
        let (c_type, size_bytes) = infer_c_layout(var_ty, decl.init.as_ref());

        // 1. Verifica se já foi declarada no MESMO escopo (B_atual)
        if let Err(existing) = self.table.insert_with_c_type(
            decl.name,
            var_ty,
            c_type,
            size_bytes,
            SymbolKind::Variable,
            decl.span,
        ) {
            self.push_redeclaration_error(decl.name, decl.span, &existing);
        }

        // 2. Verifica se o valor atribuído na inicialização é compatível com o tipo da variável
        if let (Some(expr), Some(found_ty)) = (&decl.init, init_ty) {
            if !found_ty.is_assignable_to(var_ty) {
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
    }

    fn check_assignment(&mut self, assign: &Assignment<'_>) {
        let value_ty = self.check_expr(&assign.value);

        // 1. Verifica se a variável já foi declarada (busca em B_atual -> pais -> B0)
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

                // 2. Verifica se o valor atribuído é compatível com o tipo da variável
                if let Some(found_ty) = value_ty {
                    if !found_ty.is_assignable_to(sym.ty) {
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
            }
            None => {
                self.errors.push(SemanticError {
                    kind: SemanticErrorKind::UndeclaredIdentifier {
                        name: assign.name.to_string(),
                    },
                    message: format!(
                        "variável '{}' não foi declarada neste escopo ({}) nem em escopos externos",
                        assign.name,
                        self.table.current_scope_id()
                    ),
                    span: assign.span,
                });
            }
        }
    }

    fn check_if(&mut self, if_stmt: &IfStmt<'_>) {
        if let Some(cond_ty) = self.check_expr(&if_stmt.condition) {
            if cond_ty != SemanticType::Bool {
                self.errors.push(SemanticError {
                    kind: SemanticErrorKind::NonBooleanCondition {
                        found: cond_ty,
                        construct: "se",
                    },
                    message: format!(
                        "a condição do comando 'se' deve ser lógica ('bool'), mas foi encontrado '{}'",
                        cond_ty
                    ),
                    span: if_stmt.condition.span(),
                });
            }
        }

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
        if let Some(cond_ty) = self.check_expr(&while_stmt.condition) {
            if cond_ty != SemanticType::Bool {
                self.errors.push(SemanticError {
                    kind: SemanticErrorKind::NonBooleanCondition {
                        found: cond_ty,
                        construct: "enquanto",
                    },
                    message: format!(
                        "a condição do laço 'enquanto' deve ser lógica ('bool'), mas foi encontrado '{}'",
                        cond_ty
                    ),
                    span: while_stmt.condition.span(),
                });
            }
        }

        self.loop_depth += 1;
        self.check_block(
            &while_stmt.body,
            format!("bloco 'enquanto' (linha {})", while_stmt.body.span.start.line),
        );
        self.loop_depth -= 1;
    }

    fn check_for(&mut self, for_stmt: &ForStmt<'_>) {
        // Em C (C99+), o laço `for` possui seu próprio escopo para variáveis declaradas no cabeçalho
        self.table.enter_scope(format!(
            "laço 'para' (linha {})",
            for_stmt.span.start.line
        ));

        if let Some(init) = &for_stmt.init {
            match init {
                ForInit::Assignment(assign) => self.check_assignment(assign),
                ForInit::VarDecl(decl) => self.check_var_decl(decl),
            }
        }

        if let Some(cond) = &for_stmt.condition {
            if let Some(cond_ty) = self.check_expr(cond) {
                if cond_ty != SemanticType::Bool {
                    self.errors.push(SemanticError {
                        kind: SemanticErrorKind::NonBooleanCondition {
                            found: cond_ty,
                            construct: "para",
                        },
                        message: format!(
                            "a condição do laço 'para' deve ser lógica ('bool'), mas foi encontrado '{}'",
                            cond_ty
                        ),
                        span: cond.span(),
                    });
                }
            }
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
            if let Some(ty) = self.check_expr(arg) {
                if ty == SemanticType::Vazio {
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
    }

    fn check_read(&mut self, read_stmt: &ReadStmt<'_>) {
        match self.table.resolve_and_record(read_stmt.var_name, read_stmt.span) {
            Some(sym) => {
                if matches!(sym.kind, SymbolKind::Function { .. }) {
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
            }
            None => {
                self.errors.push(SemanticError {
                    kind: SemanticErrorKind::UndeclaredIdentifier {
                        name: read_stmt.var_name.to_string(),
                    },
                    message: format!(
                        "variável '{}' usada em 'leia' não foi declarada neste escopo ({}) nem em escopos externos",
                        read_stmt.var_name,
                        self.table.current_scope_id()
                    ),
                    span: read_stmt.span,
                });
            }
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
                            "a função '{}' tem retorno 'vazio' e não pode retornar um valor do tipo '{}'",
                            func_name, found
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
                            "a função '{}' deve retornar um valor do tipo '{}', mas 'retorna;' foi usado sem expressão",
                            func_name, expected_ty
                        ),
                        span: ret_stmt.span,
                    });
                }
                (expected_ty, Some(expr), Some(found)) if expected_ty != SemanticType::Vazio => {
                    if !found.is_assignable_to(expected_ty) {
                        self.errors.push(SemanticError {
                            kind: SemanticErrorKind::ReturnMismatch {
                                function_name: func_name.clone(),
                                expected: expected_ty,
                                found,
                            },
                            message: format!(
                                "tipo de retorno incompatível na função '{}': esperado '{}', mas foi retornado '{}'",
                                func_name, expected_ty, found
                            ),
                            span: expr.span(),
                        });
                    }
                }
                _ => {}
            }
        }
    }

    /// Avalia o tipo semântico de uma expressão, retornando `None` caso um erro impeça a determinação.
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
            Expr::Identifier { name, span } => {
                match self.table.resolve_and_record(name, *span) {
                    Some(sym) => {
                        if matches!(sym.kind, SymbolKind::Function { .. }) {
                            self.errors.push(SemanticError {
                                kind: SemanticErrorKind::InvalidSymbolKind {
                                    name: (*name).to_string(),
                                    expected: "variável",
                                    found: "função",
                                },
                                message: format!(
                                    "'{}' é uma função e deve ser chamada com parênteses",
                                    name
                                ),
                                span: *span,
                            });
                            None
                        } else {
                            Some(sym.ty)
                        }
                    }
                    None => {
                        self.errors.push(SemanticError {
                            kind: SemanticErrorKind::UndeclaredIdentifier {
                                name: (*name).to_string(),
                            },
                            message: format!(
                                "variável '{}' não foi declarada neste escopo ({}) nem em escopos externos",
                                name,
                                self.table.current_scope_id()
                            ),
                            span: *span,
                        });
                        None
                    }
                }
            }
            Expr::Call(call) => self.check_function_call(call),
            Expr::Unary { op, operand, span } => {
                let operand_ty = self.check_expr(operand)?;
                match op {
                    UnaryOp::Neg => {
                        if operand_ty.is_numeric() {
                            Some(operand_ty)
                        } else {
                            self.errors.push(SemanticError {
                                kind: SemanticErrorKind::InvalidOperation {
                                    op: op.to_string(),
                                    left: operand_ty,
                                    right: None,
                                },
                                message: format!(
                                    "o operador unário '-' exige operando numérico ('int', 'dec' ou 'num'), mas foi encontrado '{}'",
                                    operand_ty
                                ),
                                span: *span,
                            });
                            None
                        }
                    }
                    UnaryOp::Not => {
                        if operand_ty == SemanticType::Bool {
                            Some(SemanticType::Bool)
                        } else {
                            self.errors.push(SemanticError {
                                kind: SemanticErrorKind::InvalidOperation {
                                    op: op.to_string(),
                                    left: operand_ty,
                                    right: None,
                                },
                                message: format!(
                                    "o operador lógico '!' exige operando booleano ('bool'), mas foi encontrado '{}'",
                                    operand_ty
                                ),
                                span: *span,
                            });
                            None
                        }
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

                match op {
                    // Operadores Aritméticos: +, -, *, /
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                        if let Some(res_ty) = l_ty.unify_numeric(r_ty) {
                            Some(res_ty)
                        } else {
                            self.errors.push(SemanticError {
                                kind: SemanticErrorKind::InvalidOperation {
                                    op: op.to_string(),
                                    left: l_ty,
                                    right: Some(r_ty),
                                },
                                message: format!(
                                    "operação aritmética '{}' inválida entre os tipos '{}' e '{}' (tipos numéricos incompatíveis)",
                                    op, l_ty, r_ty
                                ),
                                span: *span,
                            });
                            None
                        }
                    }
                    // Operador Módulo: % (exclusivo para inteiros em C)
                    BinaryOp::Mod => {
                        let valid_left = matches!(l_ty, SemanticType::Int | SemanticType::Num);
                        let valid_right = matches!(r_ty, SemanticType::Int | SemanticType::Num);
                        if valid_left && valid_right {
                            Some(SemanticType::Int)
                        } else {
                            self.errors.push(SemanticError {
                                kind: SemanticErrorKind::InvalidOperation {
                                    op: op.to_string(),
                                    left: l_ty,
                                    right: Some(r_ty),
                                },
                                message: format!(
                                    "o operador de resto '%' exige operandos inteiros ('int'), mas foi usado com '{}' e '{}'",
                                    l_ty, r_ty
                                ),
                                span: *span,
                            });
                            None
                        }
                    }
                    // Operadores Relacionais de Ordem: <, >, <=, >=
                    BinaryOp::LessThan
                    | BinaryOp::GreaterThan
                    | BinaryOp::LessEqual
                    | BinaryOp::GreaterEqual => {
                        if l_ty.unify_numeric(r_ty).is_some() {
                            Some(SemanticType::Bool)
                        } else {
                            self.errors.push(SemanticError {
                                kind: SemanticErrorKind::InvalidOperation {
                                    op: op.to_string(),
                                    left: l_ty,
                                    right: Some(r_ty),
                                },
                                message: format!(
                                    "comparação relacional '{}' inválida entre '{}' e '{}'",
                                    op, l_ty, r_ty
                                ),
                                span: *span,
                            });
                            None
                        }
                    }
                    // Operadores de Igualdade: ==, !=
                    BinaryOp::Equal | BinaryOp::NotEqual => {
                        let compatible = l_ty.unify_numeric(r_ty).is_some()
                            || (l_ty == r_ty && l_ty != SemanticType::Vazio)
                            || l_ty.is_assignable_to(r_ty)
                            || r_ty.is_assignable_to(l_ty);
                        if compatible {
                            Some(SemanticType::Bool)
                        } else {
                            self.errors.push(SemanticError {
                                kind: SemanticErrorKind::InvalidOperation {
                                    op: op.to_string(),
                                    left: l_ty,
                                    right: Some(r_ty),
                                },
                                message: format!(
                                    "comparação de igualdade '{}' inválida entre tipos incompatíveis '{}' e '{}'",
                                    op, l_ty, r_ty
                                ),
                                span: *span,
                            });
                            None
                        }
                    }
                    // Operadores Lógicos: &&, ||
                    BinaryOp::And | BinaryOp::Or => {
                        if l_ty == SemanticType::Bool && r_ty == SemanticType::Bool {
                            Some(SemanticType::Bool)
                        } else {
                            self.errors.push(SemanticError {
                                kind: SemanticErrorKind::InvalidOperation {
                                    op: op.to_string(),
                                    left: l_ty,
                                    right: Some(r_ty),
                                },
                                message: format!(
                                    "o operador lógico '{}' exige operandos booleanos ('bool'), mas foi usado com '{}' e '{}'",
                                    op, l_ty, r_ty
                                ),
                                span: *span,
                            });
                            None
                        }
                    }
                }
            }
        }
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
                            if let Some(found_ty) = *arg_ty_opt {
                                if !found_ty.is_assignable_to(expected_ty) {
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
                                            "tipo incompatível no {}º argumento de '{}': esperado '{}', mas foi encontrado '{}'",
                                            idx + 1,
                                            call.callee,
                                            expected_ty,
                                            found_ty
                                        ),
                                        span: arg_expr.span(),
                                    });
                                }
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

    fn push_redeclaration_error(&mut self, name: &str, span: Span, existing: &Symbol) {
        self.errors.push(SemanticError {
            kind: SemanticErrorKind::Redeclaration {
                name: name.to_string(),
                scope: existing.scope_id,
                previous_span: existing.decl_span,
            },
            message: format!(
                "redeclaração de '{}' no mesmo escopo ({}): já declarado anteriormente na linha {}:{}",
                name,
                existing.scope_id,
                existing.decl_span.start.line,
                existing.decl_span.start.column
            ),
            span,
        });
    }
}

/// Retorna o tipo C padrão e o tamanho em bytes para um `SemanticType`.
pub const fn default_c_layout(ty: SemanticType) -> (&'static str, usize) {
    match ty {
        SemanticType::Int => ("int", 4),
        SemanticType::Dec => ("double", 8),
        SemanticType::Carac => ("char", 1),
        SemanticType::Num => ("double", 8),
        SemanticType::Bool => ("bool", 1),
        SemanticType::Texto => ("const char*", 8),
        SemanticType::Vazio => ("void", 0),
    }
}

/// Infere o tipo C mais adequado e o tamanho em bytes a partir do tipo declarado e do literal inicializador.
fn infer_c_layout(var_ty: SemanticType, init: Option<&Expr<'_>>) -> (&'static str, usize) {
    let compiler = NumCompiler::new();
    if let Some(expr) = init {
        match expr {
            Expr::IntLiteral { raw, span } if matches!(var_ty, SemanticType::Int | SemanticType::Num) => {
                let tok = Token {
                    kind: TokenKind::NumInt,
                    lexeme: raw,
                    span: *span,
                };
                if let Ok(compiled) = compiler.lower_literal(&tok) {
                    return (compiled.representation.c_type(), compiled.representation.bytes());
                }
            }
            Expr::DecLiteral { raw, span } if matches!(var_ty, SemanticType::Dec | SemanticType::Num) => {
                let tok = Token {
                    kind: TokenKind::NumDec,
                    lexeme: raw,
                    span: *span,
                };
                if let Ok(compiled) = compiler.lower_literal(&tok) {
                    let rep = if var_ty == SemanticType::Dec && matches!(compiled.representation, NumRepresentation::Int | NumRepresentation::Long) {
                        NumRepresentation::Double
                    } else {
                        compiled.representation
                    };
                    return (rep.c_type(), rep.bytes());
                }
            }
            _ => {}
        }
    }
    default_c_layout(var_ty)
}

/// Ponto de entrada conveniente para analisar semanticamente uma AST já construída.
pub fn analyze_semantics(program: &Program<'_>) -> Result<SymbolTable, Vec<SemanticError>> {
    SemanticAnalyzer::new().analyze(program)
}

/// Ponto de entrada de alto nível: recebe o código-fonte Laila, executa análise léxica,
/// sintática e semântica, retornando a `SymbolTable` completa.
pub fn analyze_source(source: &str) -> Result<SymbolTable, String> {
    let tokens = crate::tokenize(source).map_err(|e| format!("{e}"))?;
    let program = crate::parse(&tokens).map_err(|e| format!("{e}"))?;
    analyze_semantics(&program).map_err(|errs| {
        errs.iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })
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
        // Reproduz exatamente o exemplo do slide "PROBLEMA SEMÂNTICO" em Laila:
        // vazio principal() {               // B1
        //     int a = 1;
        //     int b = 1;
        //     {                             // B2
        //         int b = 2;
        //         {                         // B3
        //             int a = 3;
        //             imprima(a, b);        // a -> B3, b -> B2
        //         }
        //         {                         // B4
        //             int b = 4;
        //             imprima(a, b);        // a -> B1, b -> B4
        //         }
        //         imprima(a, b);            // a -> B1, b -> B2
        //     }
        //     imprima(a, b);                // a -> B1, b -> B1
        // }
        let code = r#"
vazio principal() {
    int a = 1;
    int b = 1;
    {
        int b = 2;
        {
            int a = 3;
            imprima(a, b);
        }
        {
            int b = 4;
            imprima(a, b);
        }
        imprima(a, b);
    }
    imprima(a, b);
}
"#;
        let table = analyze_str(code).expect("deveria passar na análise semântica");

        // Escopos criados: B0 (global), B1 (principal), B2 (bloco), B3 (bloco), B4 (bloco)
        assert_eq!(table.scopes().len(), 5);

        let refs = table.references();
        assert_eq!(refs.len(), 8);

        // 1º imprima(a, b) dentro de B3 -> a de B3, b de B2
        assert_eq!(refs[0].name, "a");
        assert_eq!(refs[0].use_scope, ScopeId(3));
        assert_eq!(refs[0].resolved_scope, ScopeId(3));

        assert_eq!(refs[1].name, "b");
        assert_eq!(refs[1].use_scope, ScopeId(3));
        assert_eq!(refs[1].resolved_scope, ScopeId(2));

        // 2º imprima(a, b) dentro de B4 -> a de B1, b de B4
        assert_eq!(refs[2].name, "a");
        assert_eq!(refs[2].use_scope, ScopeId(4));
        assert_eq!(refs[2].resolved_scope, ScopeId(1));

        assert_eq!(refs[3].name, "b");
        assert_eq!(refs[3].use_scope, ScopeId(4));
        assert_eq!(refs[3].resolved_scope, ScopeId(4));

        // 3º imprima(a, b) dentro de B2 -> a de B1, b de B2
        assert_eq!(refs[4].name, "a");
        assert_eq!(refs[4].use_scope, ScopeId(2));
        assert_eq!(refs[4].resolved_scope, ScopeId(1));

        assert_eq!(refs[5].name, "b");
        assert_eq!(refs[5].use_scope, ScopeId(2));
        assert_eq!(refs[5].resolved_scope, ScopeId(2));

        // 4º imprima(a, b) dentro de B1 -> a de B1, b de B1
        assert_eq!(refs[6].name, "a");
        assert_eq!(refs[6].use_scope, ScopeId(1));
        assert_eq!(refs[6].resolved_scope, ScopeId(1));

        assert_eq!(refs[7].name, "b");
        assert_eq!(refs[7].use_scope, ScopeId(1));
        assert_eq!(refs[7].resolved_scope, ScopeId(1));
    }

    #[test]
    fn test_shadowing_muda_tipo_em_bloco_interno() {
        // Como no slide "TABELA DE SÍMBOLOS" onde `y` é `int` em B1 e `dec`/`carac` em B2
        let code = r#"
vazio principal() {
    int y = 10;
    {
        dec y = 3,14;
        y = 7,00;
    }
    y = 20;
}
"#;
        assert!(analyze_str(code).is_ok());
    }

    #[test]
    fn test_erro_variavel_nao_declarada() {
        let code = r#"
vazio principal() {
    taxa = 40;
}
"#;
        let errs = analyze_str(code).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(matches!(
            errs[0].kind,
            SemanticErrorKind::UndeclaredIdentifier { ref name } if name == "taxa"
        ));
    }

    #[test]
    fn test_erro_variavel_fora_do_escopo_interno() {
        let code = r#"
vazio principal() {
    {
        int interna = 5;
    }
    imprima(interna);
}
"#;
        let errs = analyze_str(code).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(matches!(
            errs[0].kind,
            SemanticErrorKind::UndeclaredIdentifier { ref name } if name == "interna"
        ));
    }

    #[test]
    fn test_erro_redeclaracao_mesmo_bloco() {
        let code = r#"
vazio principal() {
    int a = 1;
    int a = 2;
}
"#;
        let errs = analyze_str(code).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(matches!(
            errs[0].kind,
            SemanticErrorKind::Redeclaration { ref name, scope: ScopeId(1), .. } if name == "a"
        ));
    }

    #[test]
    fn test_erro_redeclaracao_parametro_no_corpo_da_funcao() {
        let code = r#"
int dobro(int x) {
    int x = 10;
    retorna x;
}
"#;
        let errs = analyze_str(code).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(matches!(
            errs[0].kind,
            SemanticErrorKind::Redeclaration { ref name, .. } if name == "x"
        ));
    }

    #[test]
    fn test_erro_incompatibilidade_de_tipos_int_e_dec() {
        let code = r#"
vazio principal() {
    int x = 3,14;
    dec y = 10;
}
"#;
        let errs = analyze_str(code).unwrap_err();
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
        let code = r#"
vazio principal() {
    num a = 10;
    num b = 8,50;
    int i = a;
    dec d = b;
}
"#;
        assert!(analyze_str(code).is_ok());
    }

    #[test]
    fn test_erro_condicao_nao_booleana() {
        let code = r#"
vazio principal() {
    int x = 10;
    se (x + 5) {
        imprima(x);
    }
}
"#;
        let errs = analyze_str(code).unwrap_err();
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
        let code = r#"
int soma(int a, int b) {
    retorna a + b;
}

vazio principal() {
    int resultado = soma(10, 20);
    imprima(resultado);
}
"#;
        assert!(analyze_str(code).is_ok());
    }

    #[test]
    fn test_erro_retorno_incompativel() {
        let code = r#"
int calcula() {
    retorna 3,14;
}
"#;
        let errs = analyze_str(code).unwrap_err();
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
}
