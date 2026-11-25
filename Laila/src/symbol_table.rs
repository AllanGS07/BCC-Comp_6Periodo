use crate::ast::Type;
use crate::Span;
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

    /// Verifica se um valor deste tipo pode ser atribuído a um alvo do tipo `target`.
    pub const fn is_assignable_to(self, target: Self) -> bool {
        matches!(
            (target, self),
            (Self::Int, Self::Int | Self::Num)
                | (Self::Dec, Self::Dec | Self::Num)
                | (Self::Num, Self::Int | Self::Dec | Self::Num)
                | (Self::Carac, Self::Carac | Self::Texto)
                | (Self::Bool, Self::Bool)
                | (Self::Texto, Self::Texto | Self::Carac)
        )
    }

    /// Unifica dois tipos numéricos em operações binárias (tipagem estrita entre `int` e `dec`,
    /// com `num` compatível com ambos).
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

/// Retorna o tipo C padrão e o tamanho em bytes para um `SemanticType`.
pub const fn default_c_layout(ty: SemanticType) -> (&'static str, usize) {
    match ty {
        SemanticType::Int => ("int", 4),
        SemanticType::Dec | SemanticType::Num => ("double", 8),
        SemanticType::Carac => ("char", 1),
        SemanticType::Bool => ("bool", 1),
        SemanticType::Texto => ("const char*", 8),
        SemanticType::Vazio => ("void", 0),
    }
}

/// Categoria do símbolo armazenado na Tabela de Símbolos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Variable,
    Parameter,
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
    pub c_type: &'static str,
    pub size_bytes: usize,
    pub kind: SymbolKind,
    pub scope_id: ScopeId,
    pub decl_span: Span,
    pub references: Vec<Span>,
}

/// Registro de resolução de referência de um identificador (Uso -> Declaração).
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

/// Tabela de Símbolos Hierárquica (árvore de escopos B0, B1, B2...) seguindo as regras de C.
#[derive(Debug, Clone)]
pub struct SymbolTable {
    scopes: Vec<Scope>,
    current_scope: ScopeId,
    references: Vec<ResolvedReference>,
    decl_scopes: HashMap<Span, ScopeId>,
    block_scopes: HashMap<Span, ScopeId>,
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolTable {
    pub fn new() -> Self {
        let global = Scope::new(ScopeId(0), None, "escopo global");
        Self {
            scopes: vec![global],
            current_scope: ScopeId(0),
            references: Vec::new(),
            decl_scopes: HashMap::new(),
            block_scopes: HashMap::new(),
        }
    }

    #[inline]
    pub fn current_scope_id(&self) -> ScopeId {
        self.current_scope
    }

    pub fn scopes(&self) -> &[Scope] {
        &self.scopes
    }

    pub fn scope(&self, id: ScopeId) -> &Scope {
        &self.scopes[id.0]
    }

    pub fn references(&self) -> &[ResolvedReference] {
        &self.references
    }

    pub fn enter_scope_for(&mut self, description: impl Into<String>, span: Span) -> ScopeId {
        let new_id = ScopeId(self.scopes.len());
        let parent_id = self.current_scope;
        self.scopes[parent_id.0].children.push(new_id);
        self.scopes
            .push(Scope::new(new_id, Some(parent_id), description));
        self.current_scope = new_id;
        self.block_scopes.insert(span, new_id);
        new_id
    }

    pub fn exit_scope(&mut self) {
        if let Some(parent) = self.scopes[self.current_scope.0].parent {
            self.current_scope = parent;
        }
    }

    pub fn scope_for_block(&self, span: Span) -> Option<ScopeId> {
        self.block_scopes.get(&span).copied()
    }

    pub fn scope_for_decl(&self, span: Span) -> Option<ScopeId> {
        self.decl_scopes.get(&span).copied()
    }

    pub fn symbol(&self, scope_id: ScopeId, name: &str) -> Option<&Symbol> {
        self.scopes.get(scope_id.0)?.symbols.get(name)
    }

    pub fn update_symbol_layout(
        &mut self,
        scope_id: ScopeId,
        name: &str,
        c_type: &'static str,
        size_bytes: usize,
    ) {
        if let Some(sym) = self
            .scopes
            .get_mut(scope_id.0)
            .and_then(|s| s.symbols.get_mut(name))
        {
            sym.c_type = c_type;
            sym.size_bytes = size_bytes;
        }
        for r in &mut self.references {
            if r.resolved_scope == scope_id && r.name == name {
                r.c_type = c_type;
            }
        }
    }

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

    pub fn find_reference_at(&self, use_span: Span) -> Option<&ResolvedReference> {
        self.references.iter().find(|r| r.use_span == use_span)
    }

    pub fn insert(
        &mut self,
        name: impl Into<String>,
        ty: SemanticType,
        kind: SymbolKind,
        decl_span: Span,
    ) -> Result<ScopeId, Box<Symbol>> {
        let (c_type, size_bytes) = default_c_layout(ty);
        let name = name.into();
        let scope_id = self.current_scope;
        let scope = &mut self.scopes[scope_id.0];

        if let Some(existing) = scope.symbols.get(&name) {
            return Err(Box::new(existing.clone()));
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
        self.decl_scopes.insert(decl_span, scope_id);
        Ok(scope_id)
    }

    /// Formata a Tabela de Símbolos em formato visual hierárquico (`B0, B1, B2...`).
    pub fn format_table(&self) -> String {
        let mut out = String::new();
        out.push_str("========================================================================\n");
        out.push_str("                    TABELA DE SÍMBOLOS (POR ESCOPO)                     \n");
        out.push_str(
            "========================================================================\n\n",
        );

        self.render_scope_tree(ScopeId(0), "", true, true, &mut out);

        out.push_str(
            "\n========================================================================\n",
        );
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
                "Identificador",
                "Tipo",
                "Tipo C (Bytes)",
                "Categoria",
                "Declaração",
                "Referências (usos)"
            ));
            out.push_str(&format!(
                "{table_prefix}  ├──────────────────┼──────────┼──────────────────┼──────────────┼────────────┼──────────────────────┤\n"
            ));

            for name in &scope.symbol_order {
                let sym = &scope.symbols[name];
                let decl_pos = format!(
                    "{}:{}",
                    sym.decl_span.start.line, sym.decl_span.start.column
                );
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
