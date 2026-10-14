use crate::parser::ParseError;
use crate::{Position, Span, Token, TokenKind};
use std::fmt;

/// Nó da Árvore de Derivação Concreta (Parse Tree / Concrete Syntax Tree - CST).
///
/// Ao contrário da AST (Árvore Sintática Abstrata), a Árvore de Derivação Concreta
/// preserva rigorosamente todos os elementos sintáticos da gramática da linguagem Laila:
/// - Símbolos não-terminais (regras gramaticais como `<PROGRAMA>`, `<FUNCAO>`, `<DECL_VARIAVEL>`)
/// - Símbolos terminais com seus lexemas exatos (incluindo `=`, `;`, `(`, `)`, `{`, `}`, `,`, etc.)
/// - Derivações vazias (ε / epsilon)
#[derive(Debug, Clone, PartialEq)]
pub enum CstNode<'src> {
    /// Símbolo Terminal concreto extraído do código-fonte (ex.: ';', '=', 'nota_final', '8,50').
    Terminal(Token<'src>),
    /// Símbolo Vazio (ε / epsilon), representando derivações vazias na gramática.
    Epsilon,
    /// Símbolo Não-Terminal, representando uma regra de produção gramatical.
    NonTerminal {
        name: &'static str,
        children: Vec<CstNode<'src>>,
        span: Span,
    },
}

impl<'src> CstNode<'src> {
    /// Obtém o span (posição de início e fim) associado a este nó.
    pub fn span(&self) -> Span {
        match self {
            Self::Terminal(tok) => tok.span,
            Self::Epsilon => Span {
                start: Position {
                    byte: 0,
                    line: 0,
                    column: 0,
                },
                end: Position {
                    byte: 0,
                    line: 0,
                    column: 0,
                },
            },
            Self::NonTerminal { span, .. } => *span,
        }
    }

    /// Retorna o nome da regra não-terminal ou descrição do terminal.
    pub fn label(&self) -> String {
        match self {
            Self::Terminal(tok) => match tok.kind {
                TokenKind::Id | TokenKind::NumInt | TokenKind::NumDec => {
                    format!("<{}: {:?}>", tok.kind.name(), tok.lexeme)
                }
                TokenKind::Texto => {
                    format!(
                        "<{}: {:?}>",
                        tok.kind.name(),
                        tok.text_content().unwrap_or(tok.lexeme)
                    )
                }
                _ => format!("<{}: {:?}>", tok.kind.name(), tok.lexeme),
            },
            Self::Epsilon => "ε (vazio)".to_string(),
            Self::NonTerminal { name, .. } => format!("<{name}>"),
        }
    }

    /// Formata a árvore de derivação completa em formato visual hierárquico
    /// utilizando caracteres de desenho de caixa (├──, └──, │).
    pub fn format_tree(&self) -> String {
        let mut out = String::new();
        self.render_tree("", true, true, &mut out);
        out
    }

    fn render_tree(&self, prefix: &str, is_last: bool, is_root: bool, out: &mut String) {
        let branch = if is_root {
            ""
        } else if is_last {
            "└── "
        } else {
            "├── "
        };

        match self {
            Self::Terminal(_) => {
                out.push_str(prefix);
                out.push_str(branch);
                out.push_str(&self.label());
                out.push('\n');
            }
            Self::Epsilon => {
                out.push_str(prefix);
                out.push_str(branch);
                out.push_str("ε (vazio)\n");
            }
            Self::NonTerminal {
                name,
                children,
                span,
            } => {
                out.push_str(prefix);
                out.push_str(branch);
                out.push_str(&format!(
                    "<{name}> (linhas {}:{})\n",
                    span.start.line, span.end.line
                ));

                let child_prefix = if is_root {
                    ""
                } else if is_last {
                    "    "
                } else {
                    "│   "
                };
                let new_prefix = format!("{prefix}{child_prefix}");

                for (i, child) in children.iter().enumerate() {
                    let child_is_last = i + 1 == children.len();
                    child.render_tree(&new_prefix, child_is_last, false, out);
                }
            }
        }
    }
}

impl fmt::Display for CstNode<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format_tree())
    }
}

/// Analisador Sintático Concreto que gera a Árvore de Derivação Completa (CST).
///
/// Preserva todos os tokens terminais (incluindo pontuações como `=`, `;`, `(`, `)`, `{`, `}`)
/// e regras gramaticais como nós da árvore.
pub struct CstParser<'src> {
    tokens: &'src [Token<'src>],
    cursor: usize,
    default_eof: Token<'src>,
}

impl<'src> CstParser<'src> {
    pub fn new(tokens: &'src [Token<'src>]) -> Self {
        let default_eof = tokens.last().copied().unwrap_or(Token {
            kind: TokenKind::Eof,
            lexeme: "",
            span: Span {
                start: Position {
                    byte: 0,
                    line: 1,
                    column: 1,
                },
                end: Position {
                    byte: 0,
                    line: 1,
                    column: 1,
                },
            },
        });

        Self {
            tokens,
            cursor: 0,
            default_eof,
        }
    }

    #[inline]
    pub fn lookahead(&self) -> &Token<'src> {
        if self.cursor < self.tokens.len() {
            &self.tokens[self.cursor]
        } else {
            &self.default_eof
        }
    }

    pub fn next(&mut self) -> Token<'src> {
        let current = *self.lookahead();
        if self.cursor < self.tokens.len() && current.kind != TokenKind::Eof {
            self.cursor += 1;
        }
        current
    }

    pub fn match_token(&mut self, expected: TokenKind) -> Result<Token<'src>, ParseError> {
        let current = *self.lookahead();
        if current.kind == expected {
            Ok(self.next())
        } else {
            Err(ParseError {
                message: format!(
                    "esperado '{}' ({}), mas foi encontrado '{}' ({})",
                    expected_description(expected),
                    expected.name(),
                    current.lexeme,
                    current.kind.name()
                ),
                span: current.span,
                expected: Some(expected),
                found: current.kind,
            })
        }
    }

    #[inline]
    pub fn check(&self, kind: TokenKind) -> bool {
        self.lookahead().kind == kind
    }

    #[inline]
    pub fn peek_ahead(&self, offset: usize) -> &Token<'src> {
        let idx = self.cursor + offset;
        if idx < self.tokens.len() {
            &self.tokens[idx]
        } else {
            &self.default_eof
        }
    }

    // =========================================================================
    // Derivação Gramatical Completa
    // =========================================================================

    /// PROGRAMA → (FUNCAO | COMANDO)+ TK_EOF
    pub fn parse_program(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        while !self.check(TokenKind::Eof) {
            if self.is_function_header() {
                children.push(self.parse_function()?);
            } else if self.is_command_start() {
                children.push(self.parse_statement()?);
            } else {
                let tok = *self.lookahead();
                return Err(ParseError {
                    message: format!(
                        "início inválido de função ou comando: token inesperado '{}' ({})",
                        tok.lexeme,
                        tok.kind.name()
                    ),
                    span: tok.span,
                    expected: None,
                    found: tok.kind,
                });
            }
        }

        let eof_tok = self.match_token(TokenKind::Eof)?;
        let end = eof_tok.span.end;
        children.push(CstNode::Terminal(eof_tok));

        Ok(CstNode::NonTerminal {
            name: "PROGRAMA",
            children,
            span: Span { start, end },
        })
    }

    fn is_function_header(&self) -> bool {
        let k0 = self.lookahead().kind;
        let is_ret_type = matches!(
            k0,
            TokenKind::TipoVazio
                | TokenKind::TipoInt
                | TokenKind::TipoDec
                | TokenKind::TipoCarac
                | TokenKind::TipoNum
        );
        is_ret_type
            && self.peek_ahead(1).kind == TokenKind::Id
            && self.peek_ahead(2).kind == TokenKind::AbrePar
    }

    /// FUNCAO → TIPO_RETORNO IDENTIFICADOR "(" PARAMETROS ")" BLOCO
    pub fn parse_function(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        children.push(self.parse_tipo_retorno()?);
        children.push(CstNode::Terminal(self.match_token(TokenKind::Id)?));
        children.push(CstNode::Terminal(self.match_token(TokenKind::AbrePar)?));
        children.push(self.parse_parametros()?);
        children.push(CstNode::Terminal(self.match_token(TokenKind::FechaPar)?));
        let bloco = self.parse_bloco()?;
        let end = bloco.span().end;
        children.push(bloco);

        Ok(CstNode::NonTerminal {
            name: "FUNCAO",
            children,
            span: Span { start, end },
        })
    }

    /// TIPO_RETORNO → "vazio" | TIPO
    pub fn parse_tipo_retorno(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        if self.check(TokenKind::TipoVazio) {
            let tok = self.match_token(TokenKind::TipoVazio)?;
            let end = tok.span.end;
            children.push(CstNode::Terminal(tok));
            Ok(CstNode::NonTerminal {
                name: "TIPO_RETORNO",
                children,
                span: Span { start, end },
            })
        } else {
            let tipo = self.parse_tipo()?;
            let end = tipo.span().end;
            children.push(tipo);
            Ok(CstNode::NonTerminal {
                name: "TIPO_RETORNO",
                children,
                span: Span { start, end },
            })
        }
    }

    /// TIPO → "int" | "dec" | "carac" | "num"
    pub fn parse_tipo(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let tok = *self.lookahead();
        if matches!(
            tok.kind,
            TokenKind::TipoInt | TokenKind::TipoDec | TokenKind::TipoCarac | TokenKind::TipoNum
        ) {
            let consumed = self.next();
            let end = consumed.span.end;
            Ok(CstNode::NonTerminal {
                name: "TIPO",
                children: vec![CstNode::Terminal(consumed)],
                span: Span { start, end },
            })
        } else {
            Err(ParseError {
                message: format!(
                    "esperado tipo de dado ('int', 'dec', 'carac', 'num'), encontrado '{}' ({})",
                    tok.lexeme,
                    tok.kind.name()
                ),
                span: tok.span,
                expected: Some(TokenKind::TipoInt),
                found: tok.kind,
            })
        }
    }

    /// PARAMETROS → LISTA_PARAMETROS | ε
    pub fn parse_parametros(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        if self.is_type() {
            children.push(self.parse_lista_parametros()?);
            let end = children.last().unwrap().span().end;
            Ok(CstNode::NonTerminal {
                name: "PARAMETROS",
                children,
                span: Span { start, end },
            })
        } else {
            Ok(CstNode::NonTerminal {
                name: "PARAMETROS",
                children: vec![CstNode::Epsilon],
                span: Span { start, end: start },
            })
        }
    }

    /// LISTA_PARAMETROS → PARAMETRO ("," PARAMETRO)*
    fn parse_lista_parametros(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        children.push(self.parse_parametro()?);
        while self.check(TokenKind::Virgula) {
            children.push(CstNode::Terminal(self.match_token(TokenKind::Virgula)?));
            children.push(self.parse_parametro()?);
        }

        let end = children.last().unwrap().span().end;
        Ok(CstNode::NonTerminal {
            name: "LISTA_PARAMETROS",
            children,
            span: Span { start, end },
        })
    }

    /// PARAMETRO → TIPO IDENTIFICADOR
    fn parse_parametro(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        children.push(self.parse_tipo()?);
        let id_tok = self.match_token(TokenKind::Id)?;
        let end = id_tok.span.end;
        children.push(CstNode::Terminal(id_tok));

        Ok(CstNode::NonTerminal {
            name: "PARAMETRO",
            children,
            span: Span { start, end },
        })
    }

    /// BLOCO → "{" COMANDOS "}"
    pub fn parse_bloco(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        children.push(CstNode::Terminal(
            self.match_token(TokenKind::AbreChaves)?,
        ));
        children.push(self.parse_comandos()?);
        let fecha = self.match_token(TokenKind::FechaChaves)?;
        let end = fecha.span.end;
        children.push(CstNode::Terminal(fecha));

        Ok(CstNode::NonTerminal {
            name: "BLOCO",
            children,
            span: Span { start, end },
        })
    }

    /// COMANDOS → COMANDO COMANDOS | ε
    pub fn parse_comandos(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        while !self.check(TokenKind::FechaChaves) && !self.check(TokenKind::Eof) {
            children.push(self.parse_statement()?);
        }

        if children.is_empty() {
            children.push(CstNode::Epsilon);
        }

        let end = self.lookahead().span.start;
        Ok(CstNode::NonTerminal {
            name: "COMANDOS",
            children,
            span: Span { start, end },
        })
    }

    fn is_type(&self) -> bool {
        matches!(
            self.lookahead().kind,
            TokenKind::TipoInt | TokenKind::TipoDec | TokenKind::TipoCarac | TokenKind::TipoNum
        )
    }

    fn is_command_start(&self) -> bool {
        self.is_type()
            || matches!(
                self.lookahead().kind,
                TokenKind::Id
                    | TokenKind::Se
                    | TokenKind::Enquanto
                    | TokenKind::Para
                    | TokenKind::Pare
                    | TokenKind::Continue
                    | TokenKind::Retorna
                    | TokenKind::Imprima
                    | TokenKind::Leia
                    | TokenKind::AbreChaves
            )
    }

    /// COMANDO → DECL_VARIAVEL | ATRIBUICAO | CONDICIONAL | REPETICAO_ENQUANTO |
    ///           REPETICAO_PARA | ESCRITA | LEITURA | RETORNO | "pare" ";" | "continue" ";" |
    ///           CHAMADA_COMANDO | BLOCO
    pub fn parse_statement(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        if self.is_type() {
            let decl = self.parse_decl_variavel()?;
            let end = decl.span().end;
            children.push(decl);
            return Ok(CstNode::NonTerminal {
                name: "COMANDO",
                children,
                span: Span { start, end },
            });
        }

        let current = *self.lookahead();
        let stmt_node = match current.kind {
            TokenKind::Id => {
                if self.peek_ahead(1).kind == TokenKind::AbrePar {
                    let mut call_children = Vec::new();
                    call_children.push(self.parse_chamada_funcao()?);
                    call_children.push(CstNode::Terminal(
                        self.match_token(TokenKind::PontoVirg)?,
                    ));
                    let end = call_children.last().unwrap().span().end;
                    CstNode::NonTerminal {
                        name: "CHAMADA_COMANDO",
                        children: call_children,
                        span: Span { start, end },
                    }
                } else {
                    self.parse_atribuicao()?
                }
            }
            TokenKind::Se => self.parse_condicional()?,
            TokenKind::Enquanto => self.parse_enquanto()?,
            TokenKind::Para => self.parse_para()?,
            TokenKind::Pare => {
                let pare_tok = self.match_token(TokenKind::Pare)?;
                let pv_tok = self.match_token(TokenKind::PontoVirg)?;
                let end = pv_tok.span.end;
                CstNode::NonTerminal {
                    name: "COMANDO_PARE",
                    children: vec![CstNode::Terminal(pare_tok), CstNode::Terminal(pv_tok)],
                    span: Span { start, end },
                }
            }
            TokenKind::Continue => {
                let cont_tok = self.match_token(TokenKind::Continue)?;
                let pv_tok = self.match_token(TokenKind::PontoVirg)?;
                let end = pv_tok.span.end;
                CstNode::NonTerminal {
                    name: "COMANDO_CONTINUE",
                    children: vec![CstNode::Terminal(cont_tok), CstNode::Terminal(pv_tok)],
                    span: Span { start, end },
                }
            }
            TokenKind::Retorna => self.parse_retorno()?,
            TokenKind::Imprima => self.parse_escrita()?,
            TokenKind::Leia => self.parse_leitura()?,
            TokenKind::AbreChaves => self.parse_bloco()?,
            _ => {
                return Err(ParseError {
                    message: format!(
                        "comando inválido ou token inesperado '{}' ({})",
                        current.lexeme,
                        current.kind.name()
                    ),
                    span: current.span,
                    expected: None,
                    found: current.kind,
                })
            }
        };

        let end = stmt_node.span().end;
        children.push(stmt_node);

        Ok(CstNode::NonTerminal {
            name: "COMANDO",
            children,
            span: Span { start, end },
        })
    }

    /// DECL_VARIAVEL → TIPO IDENTIFICADOR ";" | TIPO IDENTIFICADOR "=" EXPRESSAO ";"
    pub fn parse_decl_variavel(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        // 1. TIPO
        children.push(self.parse_tipo()?);
        // 2. IDENTIFICADOR
        children.push(CstNode::Terminal(self.match_token(TokenKind::Id)?));

        // 3. Opcional "=" EXPRESSAO
        if self.check(TokenKind::Atrib) {
            children.push(CstNode::Terminal(self.match_token(TokenKind::Atrib)?));
            children.push(self.parse_expressao_ou_logica()?);
        }

        // 4. ";"
        let pv = self.match_token(TokenKind::PontoVirg)?;
        let end = pv.span.end;
        children.push(CstNode::Terminal(pv));

        Ok(CstNode::NonTerminal {
            name: "DECL_VARIAVEL",
            children,
            span: Span { start, end },
        })
    }

    /// ATRIBUICAO → IDENTIFICADOR "=" EXPRESSAO ";"
    pub fn parse_atribuicao(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        children.push(CstNode::Terminal(self.match_token(TokenKind::Id)?));
        children.push(CstNode::Terminal(self.match_token(TokenKind::Atrib)?));
        children.push(self.parse_expressao_ou_logica()?);
        let pv = self.match_token(TokenKind::PontoVirg)?;
        let end = pv.span.end;
        children.push(CstNode::Terminal(pv));

        Ok(CstNode::NonTerminal {
            name: "ATRIBUICAO",
            children,
            span: Span { start, end },
        })
    }

    /// ATRIB_SIMPLES → IDENTIFICADOR "=" EXPRESSAO
    pub fn parse_atrib_simples(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        children.push(CstNode::Terminal(self.match_token(TokenKind::Id)?));
        children.push(CstNode::Terminal(self.match_token(TokenKind::Atrib)?));
        let expr = self.parse_expressao_ou_logica()?;
        let end = expr.span().end;
        children.push(expr);

        Ok(CstNode::NonTerminal {
            name: "ATRIB_SIMPLES",
            children,
            span: Span { start, end },
        })
    }

    /// CONDICIONAL → "se" "(" EXPRESSAO_LOGICA ")" BLOCO ["senao" BLOCO]
    pub fn parse_condicional(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        children.push(CstNode::Terminal(self.match_token(TokenKind::Se)?));
        children.push(CstNode::Terminal(self.match_token(TokenKind::AbrePar)?));
        children.push(self.parse_expressao_logica()?);
        children.push(CstNode::Terminal(self.match_token(TokenKind::FechaPar)?));
        let then_branch = self.parse_bloco()?;
        let mut end = then_branch.span().end;
        children.push(then_branch);

        if self.check(TokenKind::Senao) {
            children.push(CstNode::Terminal(self.match_token(TokenKind::Senao)?));
            let else_branch = self.parse_bloco()?;
            end = else_branch.span().end;
            children.push(else_branch);
        }

        Ok(CstNode::NonTerminal {
            name: "CONDICIONAL",
            children,
            span: Span { start, end },
        })
    }

    /// REPETICAO_ENQUANTO → "enquanto" "(" EXPRESSAO_LOGICA ")" BLOCO
    pub fn parse_enquanto(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        children.push(CstNode::Terminal(self.match_token(TokenKind::Enquanto)?));
        children.push(CstNode::Terminal(self.match_token(TokenKind::AbrePar)?));
        children.push(self.parse_expressao_logica()?);
        children.push(CstNode::Terminal(self.match_token(TokenKind::FechaPar)?));
        let body = self.parse_bloco()?;
        let end = body.span().end;
        children.push(body);

        Ok(CstNode::NonTerminal {
            name: "REPETICAO_ENQUANTO",
            children,
            span: Span { start, end },
        })
    }

    /// REPETICAO_PARA → "para" "(" [FOR_INIT] ";" [EXPRESSAO_LOGICA] ";" [ATRIB_SIMPLES] ")" BLOCO
    pub fn parse_para(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        children.push(CstNode::Terminal(self.match_token(TokenKind::Para)?));
        children.push(CstNode::Terminal(self.match_token(TokenKind::AbrePar)?));

        // FOR_INIT
        if self.is_type() {
            let decl_init = self.parse_decl_variavel_sem_pv()?;
            children.push(decl_init);
        } else if self.check(TokenKind::Id) {
            children.push(self.parse_atrib_simples()?);
        } else {
            children.push(CstNode::Epsilon);
        }
        children.push(CstNode::Terminal(
            self.match_token(TokenKind::PontoVirg)?,
        ));

        // Condição
        if !self.check(TokenKind::PontoVirg) {
            children.push(self.parse_expressao_logica()?);
        } else {
            children.push(CstNode::Epsilon);
        }
        children.push(CstNode::Terminal(
            self.match_token(TokenKind::PontoVirg)?,
        ));

        // Incremento
        if self.check(TokenKind::Id) {
            children.push(self.parse_atrib_simples()?);
        } else {
            children.push(CstNode::Epsilon);
        }
        children.push(CstNode::Terminal(self.match_token(TokenKind::FechaPar)?));

        // Corpo
        let body = self.parse_bloco()?;
        let end = body.span().end;
        children.push(body);

        Ok(CstNode::NonTerminal {
            name: "REPETICAO_PARA",
            children,
            span: Span { start, end },
        })
    }

    fn parse_decl_variavel_sem_pv(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        children.push(self.parse_tipo()?);
        children.push(CstNode::Terminal(self.match_token(TokenKind::Id)?));

        if self.check(TokenKind::Atrib) {
            children.push(CstNode::Terminal(self.match_token(TokenKind::Atrib)?));
            children.push(self.parse_expressao_ou_logica()?);
        }

        let end = children.last().unwrap().span().end;
        Ok(CstNode::NonTerminal {
            name: "DECL_VARIAVEL",
            children,
            span: Span { start, end },
        })
    }

    /// ESCRITA → "imprima" "(" ARGUMENTOS ")" ";"
    pub fn parse_escrita(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        children.push(CstNode::Terminal(self.match_token(TokenKind::Imprima)?));
        children.push(CstNode::Terminal(self.match_token(TokenKind::AbrePar)?));

        if !self.check(TokenKind::FechaPar) {
            let mut args = Vec::new();
            args.push(self.parse_expressao_ou_logica()?);
            while self.check(TokenKind::Virgula) {
                args.push(CstNode::Terminal(self.match_token(TokenKind::Virgula)?));
                args.push(self.parse_expressao_ou_logica()?);
            }
            let args_start = args.first().unwrap().span().start;
            let args_end = args.last().unwrap().span().end;
            children.push(CstNode::NonTerminal {
                name: "ARGUMENTOS",
                children: args,
                span: Span {
                    start: args_start,
                    end: args_end,
                },
            });
        } else {
            children.push(CstNode::Epsilon);
        }

        children.push(CstNode::Terminal(self.match_token(TokenKind::FechaPar)?));
        let pv = self.match_token(TokenKind::PontoVirg)?;
        let end = pv.span.end;
        children.push(CstNode::Terminal(pv));

        Ok(CstNode::NonTerminal {
            name: "ESCRITA",
            children,
            span: Span { start, end },
        })
    }

    /// LEITURA → "leia" "(" IDENTIFICADOR ")" ";"
    pub fn parse_leitura(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        children.push(CstNode::Terminal(self.match_token(TokenKind::Leia)?));
        children.push(CstNode::Terminal(self.match_token(TokenKind::AbrePar)?));
        children.push(CstNode::Terminal(self.match_token(TokenKind::Id)?));
        children.push(CstNode::Terminal(self.match_token(TokenKind::FechaPar)?));
        let pv = self.match_token(TokenKind::PontoVirg)?;
        let end = pv.span.end;
        children.push(CstNode::Terminal(pv));

        Ok(CstNode::NonTerminal {
            name: "LEITURA",
            children,
            span: Span { start, end },
        })
    }

    /// RETORNO → "retorna" [EXPRESSAO] ";"
    pub fn parse_retorno(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        children.push(CstNode::Terminal(self.match_token(TokenKind::Retorna)?));

        if !self.check(TokenKind::PontoVirg) {
            children.push(self.parse_expressao_ou_logica()?);
        }

        let pv = self.match_token(TokenKind::PontoVirg)?;
        let end = pv.span.end;
        children.push(CstNode::Terminal(pv));

        Ok(CstNode::NonTerminal {
            name: "RETORNO",
            children,
            span: Span { start, end },
        })
    }

    // =========================================================================
    // Expressões
    // =========================================================================

    #[inline]
    pub fn parse_expressao_ou_logica(&mut self) -> Result<CstNode<'src>, ParseError> {
        self.parse_expressao_logica()
    }

    /// EXPRESSAO_LOGICA → EXPR_OU
    pub fn parse_expressao_logica(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        let ou = self.parse_expr_ou()?;
        let end = ou.span().end;
        children.push(ou);

        Ok(CstNode::NonTerminal {
            name: "EXPRESSAO_LOGICA",
            children,
            span: Span { start, end },
        })
    }

    /// EXPR_OU → EXPR_E ("||" EXPR_E)*
    fn parse_expr_ou(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        children.push(self.parse_expr_e()?);

        while self.check(TokenKind::Ou) {
            children.push(CstNode::Terminal(self.next()));
            children.push(self.parse_expr_e()?);
        }

        let end = children.last().unwrap().span().end;
        if children.len() == 1 {
            Ok(children.pop().unwrap())
        } else {
            Ok(CstNode::NonTerminal {
                name: "EXPR_OU",
                children,
                span: Span { start, end },
            })
        }
    }

    /// EXPR_E → EXPR_RELACIONAL ("&&" EXPR_RELACIONAL)*
    fn parse_expr_e(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        children.push(self.parse_expr_relacional()?);

        while self.check(TokenKind::E) {
            children.push(CstNode::Terminal(self.next()));
            children.push(self.parse_expr_relacional()?);
        }

        let end = children.last().unwrap().span().end;
        if children.len() == 1 {
            Ok(children.pop().unwrap())
        } else {
            Ok(CstNode::NonTerminal {
                name: "EXPR_E",
                children,
                span: Span { start, end },
            })
        }
    }

    /// EXPR_RELACIONAL → "!" EXPR_RELACIONAL | EXPRESSAO_ARITMETICA [OP_RELACIONAL EXPRESSAO_ARITMETICA]
    fn parse_expr_relacional(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        if self.check(TokenKind::Nao) {
            children.push(CstNode::Terminal(self.next()));
            let operand = self.parse_expr_relacional()?;
            let end = operand.span().end;
            children.push(operand);
            return Ok(CstNode::NonTerminal {
                name: "EXPR_RELACIONAL",
                children,
                span: Span { start, end },
            });
        }

        let left = self.parse_expressao_aritmetica()?;
        children.push(left);

        if matches!(
            self.lookahead().kind,
            TokenKind::Igualdade
                | TokenKind::Diferente
                | TokenKind::MenorQue
                | TokenKind::MaiorQue
                | TokenKind::MenorIgual
                | TokenKind::MaiorIgual
        ) {
            let op_tok = self.next();
            let op_span = op_tok.span;
            children.push(CstNode::NonTerminal {
                name: "OP_RELACIONAL",
                children: vec![CstNode::Terminal(op_tok)],
                span: op_span,
            });
            children.push(self.parse_expressao_aritmetica()?);
            let end = children.last().unwrap().span().end;
            Ok(CstNode::NonTerminal {
                name: "EXPR_RELACIONAL",
                children,
                span: Span { start, end },
            })
        } else {
            Ok(children.pop().unwrap())
        }
    }

    /// EXPRESSAO_ARITMETICA → TERMO (( "+" | "-" ) TERMO)*
    pub fn parse_expressao_aritmetica(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        children.push(self.parse_termo()?);

        while self.check(TokenKind::Soma) || self.check(TokenKind::Subtracao) {
            children.push(CstNode::Terminal(self.next()));
            children.push(self.parse_termo()?);
        }

        let end = children.last().unwrap().span().end;
        if children.len() == 1 {
            Ok(children.pop().unwrap())
        } else {
            Ok(CstNode::NonTerminal {
                name: "EXPRESSAO_ARITMETICA",
                children,
                span: Span { start, end },
            })
        }
    }

    /// TERMO → FATOR (( "*" | "/" | "%" ) FATOR)*
    pub fn parse_termo(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        children.push(self.parse_fator()?);

        while self.check(TokenKind::Multiplica)
            || self.check(TokenKind::Divisao)
            || self.check(TokenKind::Modulo)
        {
            children.push(CstNode::Terminal(self.next()));
            children.push(self.parse_fator()?);
        }

        let end = children.last().unwrap().span().end;
        if children.len() == 1 {
            Ok(children.pop().unwrap())
        } else {
            Ok(CstNode::NonTerminal {
                name: "TERMO",
                children,
                span: Span { start, end },
            })
        }
    }

    /// FATOR → IDENTIFICADOR | CHAMADA_FUNCAO |
    ///         LITERAL_INT | LITERAL_DEC | LITERAL_TEXTO | LITERAL_BOOL |
    ///         "(" EXPRESSAO_LOGICA ")" | "-" FATOR | "+" FATOR
    pub fn parse_fator(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let current = *self.lookahead();
        let mut children = Vec::new();

        if current.kind == TokenKind::Subtracao || current.kind == TokenKind::Soma {
            children.push(CstNode::Terminal(self.next()));
            let sub_fator = self.parse_fator()?;
            let end = sub_fator.span().end;
            children.push(sub_fator);
            return Ok(CstNode::NonTerminal {
                name: "FATOR",
                children,
                span: Span { start, end },
            });
        }

        match current.kind {
            TokenKind::Id => {
                if self.peek_ahead(1).kind == TokenKind::AbrePar {
                    let call = self.parse_chamada_funcao()?;
                    let end = call.span().end;
                    children.push(call);
                    Ok(CstNode::NonTerminal {
                        name: "FATOR",
                        children,
                        span: Span { start, end },
                    })
                } else {
                    let id_tok = self.next();
                    let end = id_tok.span.end;
                    children.push(CstNode::Terminal(id_tok));
                    Ok(CstNode::NonTerminal {
                        name: "FATOR",
                        children,
                        span: Span { start, end },
                    })
                }
            }
            TokenKind::NumInt
            | TokenKind::NumDec
            | TokenKind::Texto
            | TokenKind::Verdadeiro
            | TokenKind::Falso => {
                let lit = self.next();
                let end = lit.span.end;
                children.push(CstNode::Terminal(lit));
                Ok(CstNode::NonTerminal {
                    name: "FATOR",
                    children,
                    span: Span { start, end },
                })
            }
            TokenKind::AbrePar => {
                children.push(CstNode::Terminal(self.next()));
                children.push(self.parse_expressao_ou_logica()?);
                let fecha = self.match_token(TokenKind::FechaPar)?;
                let end = fecha.span.end;
                children.push(CstNode::Terminal(fecha));
                Ok(CstNode::NonTerminal {
                    name: "FATOR",
                    children,
                    span: Span { start, end },
                })
            }
            _ => Err(ParseError {
                message: format!(
                    "esperado fator em expressão (identificador, literal numérico, texto, booleano ou parênteses), encontrado '{}' ({})",
                    current.lexeme,
                    current.kind.name()
                ),
                span: current.span,
                expected: None,
                found: current.kind,
            }),
        }
    }

    /// CHAMADA_FUNCAO → IDENTIFICADOR "(" [ARGUMENTOS] ")"
    pub fn parse_chamada_funcao(&mut self) -> Result<CstNode<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let mut children = Vec::new();

        children.push(CstNode::Terminal(self.match_token(TokenKind::Id)?));
        children.push(CstNode::Terminal(self.match_token(TokenKind::AbrePar)?));

        if !self.check(TokenKind::FechaPar) {
            let mut args = Vec::new();
            args.push(self.parse_expressao_ou_logica()?);
            while self.check(TokenKind::Virgula) {
                args.push(CstNode::Terminal(self.match_token(TokenKind::Virgula)?));
                args.push(self.parse_expressao_ou_logica()?);
            }
            let args_start = args.first().unwrap().span().start;
            let args_end = args.last().unwrap().span().end;
            children.push(CstNode::NonTerminal {
                name: "ARGUMENTOS",
                children: args,
                span: Span {
                    start: args_start,
                    end: args_end,
                },
            });
        } else {
            children.push(CstNode::Epsilon);
        }

        let fecha = self.match_token(TokenKind::FechaPar)?;
        let end = fecha.span.end;
        children.push(CstNode::Terminal(fecha));

        Ok(CstNode::NonTerminal {
            name: "CHAMADA_FUNCAO",
            children,
            span: Span { start, end },
        })
    }
}

fn expected_description(kind: TokenKind) -> &'static str {
    match kind {
        TokenKind::TipoInt => "int",
        TokenKind::TipoDec => "dec",
        TokenKind::TipoCarac => "carac",
        TokenKind::TipoNum => "num",
        TokenKind::TipoVazio => "vazio",
        TokenKind::Se => "se",
        TokenKind::Senao => "senao",
        TokenKind::Enquanto => "enquanto",
        TokenKind::Para => "para",
        TokenKind::Pare => "pare",
        TokenKind::Continue => "continue",
        TokenKind::Retorna => "retorna",
        TokenKind::Imprima => "imprima",
        TokenKind::Leia => "leia",
        TokenKind::Id => "identificador",
        TokenKind::NumInt => "número inteiro",
        TokenKind::NumDec => "número decimal",
        TokenKind::Atrib => "=",
        TokenKind::PontoVirg => ";",
        TokenKind::Virgula => ",",
        TokenKind::AbrePar => "(",
        TokenKind::FechaPar => ")",
        TokenKind::AbreChaves => "{",
        TokenKind::FechaChaves => "}",
        _ => kind.name(),
    }
}

/// Analisa uma fatia de tokens e gera a Árvore de Derivação Concreta (CST).
pub fn parse_cst<'src>(tokens: &'src [Token<'src>]) -> Result<CstNode<'src>, ParseError> {
    let mut parser = CstParser::new(tokens);
    parser.parse_program()
}

/// Analisa o código-fonte em string e retorna a Árvore de Derivação Concreta (CST).
pub fn parse_cst_source(source: &str) -> Result<CstNode<'_>, String> {
    let tokens = crate::tokenize(source).map_err(|e| format!("{e}"))?;
    let leaked: &[Token<'_>] = Box::leak(tokens.into_boxed_slice());
    parse_cst(leaked).map_err(|e| format!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenize;

    #[test]
    fn test_cst_contem_atrib_e_ponto_virgula() {
        let code = "num nota = 8,50;";
        let tokens = tokenize(code).unwrap();
        let cst = parse_cst(&tokens).unwrap();
        let tree_str = cst.format_tree();

        // Garante que a árvore contém a regra de produção e os símbolos terminais
        assert!(tree_str.contains("<DECL_VARIAVEL>"));
        assert!(tree_str.contains("<TK_TIPO_NUM: \"num\">"));
        assert!(tree_str.contains("<TK_ID: \"nota\">"));
        assert!(tree_str.contains("<TK_ATRIB: \"=\">"));
        assert!(tree_str.contains("<TK_NUM_DEC: \"8,50\">"));
        assert!(tree_str.contains("<TK_PONTO_VIRG: \";\">"));
    }

    #[test]
    fn test_cst_funcao_completa_com_chaves_e_parenteses() {
        let code = "vazio principal() { dec nota_final = 8,50; }";
        let tokens = tokenize(code).unwrap();
        let cst = parse_cst(&tokens).unwrap();
        let tree_str = cst.format_tree();

        assert!(tree_str.contains("<PROGRAMA>"));
        assert!(tree_str.contains("<FUNCAO>"));
        assert!(tree_str.contains("<TIPO_RETORNO>"));
        assert!(tree_str.contains("<TK_TIPO_VAZIO: \"vazio\">"));
        assert!(tree_str.contains("<TK_ID: \"principal\">"));
        assert!(tree_str.contains("<TK_ABRE_PAR: \"(\">"));
        assert!(tree_str.contains("<PARAMETROS>"));
        assert!(tree_str.contains("ε (vazio)"));
        assert!(tree_str.contains("<TK_FECHA_PAR: \")\">"));
        assert!(tree_str.contains("<BLOCO>"));
        assert!(tree_str.contains("<TK_ABRE_CHAVES: \"{\">"));
        assert!(tree_str.contains("<TK_FECHA_CHAVES: \"}\">"));
    }

    #[test]
    fn test_cst_condicional_se_senao() {
        let code = "se (x >= 10) { imprima(\"OK\"); } senao { imprima(\"NOK\"); }";
        let tokens = tokenize(code).unwrap();
        let cst = parse_cst(&tokens).unwrap();
        let tree_str = cst.format_tree();

        assert!(tree_str.contains("<CONDICIONAL>"));
        assert!(tree_str.contains("<TK_SE: \"se\">"));
        assert!(tree_str.contains("<OP_RELACIONAL>"));
        assert!(tree_str.contains("<TK_MAIOR_IGUAL: \">=\">"));
        assert!(tree_str.contains("<TK_SENAO: \"senao\">"));
        assert!(tree_str.contains("<ESCRITA>"));
        assert!(tree_str.contains("<TK_IMPRIMA: \"imprima\">"));
    }
}
