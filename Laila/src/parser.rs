use crate::ast::*;
use crate::{Position, Span, Token, TokenKind};
use std::fmt;

/// Erro gerado durante a análise sintática.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
    pub expected: Option<TokenKind>,
    pub found: TokenKind,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: erro sintático: {}",
            self.span.start.line, self.span.start.column, self.message
        )
    }
}

impl std::error::Error for ParseError {}

/// Analisador Sintático Descendente Recursivo para a linguagem Laila.
///
/// Implementa fielmente as três operações básicas da disciplina de Compiladores:
/// 1. `lookahead()`: consulta o token atual sem consumi-lo.
/// 2. `next()`: consome o token atual e avança para o próximo.
/// 3. `match_token(X)` (ou `match(X)`): exige que o token atual seja X, consumindo-o,
///    ou sinaliza um erro sintático.
pub struct Parser<'src> {
    tokens: &'src [Token<'src>],
    cursor: usize,
    default_eof: Token<'src>,
}

impl<'src> Parser<'src> {
    /// Inicializa o analisador sintático com a fatia de tokens gerada pelo lexer.
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

    /// 1. OLHAR (lookahead): Retorna o token atual, sem consumi-lo.
    #[inline]
    pub fn lookahead(&self) -> &Token<'src> {
        if self.cursor < self.tokens.len() {
            &self.tokens[self.cursor]
        } else {
            &self.default_eof
        }
    }

    /// 2. AVANÇAR (advance): Consome o token atual e avança para o próximo.
    pub fn advance(&mut self) -> Token<'src> {
        let current = *self.lookahead();
        if self.cursor < self.tokens.len() && current.kind != TokenKind::Eof {
            self.cursor += 1;
        }
        current
    }

    /// 3. EXIGIR (match_token):
    ///    Se o token atual for o esperado, consome e retorna o token.
    ///    Se não for, sinaliza um erro sintático descritivo.
    pub fn match_token(&mut self, expected: TokenKind) -> Result<Token<'src>, ParseError> {
        let current = *self.lookahead();
        if current.kind == expected {
            Ok(self.advance())
        } else {
            Err(ParseError {
                message: format!(
                    "esperado '{}' ({}), mas foi encontrado '{}' ({})",
                    expected.description(),
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

    /// Verifica se o token atual é do tipo esperado (sem consumi-lo).
    #[inline]
    pub fn check(&self, kind: TokenKind) -> bool {
        self.lookahead().kind == kind
    }

    /// Retorna o lookahead do próximo token (lookahead(2)).
    #[inline]
    pub fn peek_ahead(&self, offset: usize) -> &Token<'src> {
        let idx = self.cursor + offset;
        if idx < self.tokens.len() {
            &self.tokens[idx]
        } else {
            &self.default_eof
        }
    }

    /// PROGRAMA → (FUNCAO | COMANDO)+ TK_EOF
    /// Suporta múltiplos programas completos, coleções de funções e scripts com comandos.
    pub fn parse_program(&mut self) -> Result<Program<'src>, ParseError> {
        let start_pos = self.lookahead().span.start;
        let mut items = Vec::new();

        while !self.check(TokenKind::Eof) {
            if self.is_function_header() {
                let func = self.parse_function()?;
                items.push(ProgramItem::Function(func));
            } else if self.is_command_start() {
                let stmt = self.parse_statement()?;
                items.push(ProgramItem::Statement(Box::new(stmt)));
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

        let end_pos = self.lookahead().span.end;
        self.match_token(TokenKind::Eof)?;

        Ok(Program {
            items,
            span: Span {
                start: start_pos,
                end: end_pos,
            },
        })
    }

    /// Verifica se a sequência adiante é o início de uma função: `TIPO_RETORNO IDENTIFICADOR "("`
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
    pub fn parse_function(&mut self) -> Result<Function<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let return_type = self.parse_tipo_retorno()?;
        let id_tok = self.match_token(TokenKind::Id)?;
        self.match_token(TokenKind::AbrePar)?;
        let params = self.parse_parametros()?;
        self.match_token(TokenKind::FechaPar)?;
        let body = self.parse_bloco()?;
        let end = body.span.end;

        Ok(Function {
            return_type,
            name: id_tok.lexeme,
            params,
            body,
            span: Span { start, end },
        })
    }

    /// TIPO_RETORNO → "vazio" | TIPO
    pub fn parse_tipo_retorno(&mut self) -> Result<Type, ParseError> {
        if self.check(TokenKind::TipoVazio) {
            self.advance();
            Ok(Type::Vazio)
        } else {
            self.parse_tipo()
        }
    }

    /// TIPO → "int" | "dec" | "carac" | "num"
    pub fn parse_tipo(&mut self) -> Result<Type, ParseError> {
        let tok = *self.lookahead();
        if let Some(t) = Type::from_token_kind(tok.kind)
            && t != Type::Vazio {
                self.advance();
                return Ok(t);
            }
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

    /// PARAMETROS → LISTA_PARAMETROS | ε
    /// LISTA_PARAMETROS → TIPO IDENTIFICADOR ("," TIPO IDENTIFICADOR)*
    pub fn parse_parametros(&mut self) -> Result<Vec<Param<'src>>, ParseError> {
        let mut params = Vec::new();

        if self.is_type() {
            params.push(self.parse_parametro()?);
            while self.check(TokenKind::Virgula) {
                self.match_token(TokenKind::Virgula)?;
                params.push(self.parse_parametro()?);
            }
        }

        Ok(params)
    }

    fn parse_parametro(&mut self) -> Result<Param<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let param_type = self.parse_tipo()?;
        let id_tok = self.match_token(TokenKind::Id)?;
        let end = id_tok.span.end;

        Ok(Param {
            param_type,
            name: id_tok.lexeme,
            span: Span { start, end },
        })
    }

    /// BLOCO → "{" COMANDOS "}"
    pub fn parse_bloco(&mut self) -> Result<Block<'src>, ParseError> {
        let start = self.match_token(TokenKind::AbreChaves)?.span.start;
        let statements = self.parse_comandos()?;
        let end = self.match_token(TokenKind::FechaChaves)?.span.end;

        Ok(Block {
            statements,
            span: Span { start, end },
        })
    }

    /// COMANDOS → COMANDO COMANDOS | ε
    pub fn parse_comandos(&mut self) -> Result<Vec<Statement<'src>>, ParseError> {
        let mut statements = Vec::new();
        while !self.check(TokenKind::FechaChaves) && !self.check(TokenKind::Eof) {
            statements.push(self.parse_statement()?);
        }
        Ok(statements)
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
    ///           CHAMADA_FUNCAO ";" | BLOCO
    pub fn parse_statement(&mut self) -> Result<Statement<'src>, ParseError> {
        let current = *self.lookahead();
        if self.is_type() {
            return self.parse_decl_variavel().map(Statement::VarDecl);
        }

        match current.kind {
            TokenKind::Id => {
                // Se o próximo token for '(', trata-se de uma chamada de função como instrução.
                if self.peek_ahead(1).kind == TokenKind::AbrePar {
                    let call = self.parse_chamada_funcao()?;
                    self.match_token(TokenKind::PontoVirg)?;
                    Ok(Statement::FunctionCall(call))
                } else {
                    self.parse_atribuicao().map(Statement::Assignment)
                }
            }
            TokenKind::Se => self.parse_condicional().map(Statement::If),
            TokenKind::Enquanto => self.parse_enquanto().map(Statement::While),
            TokenKind::Para => self.parse_para().map(|f| Statement::For(Box::new(f))),
            TokenKind::Pare => {
                let start = self.match_token(TokenKind::Pare)?.span.start;
                let end = self.match_token(TokenKind::PontoVirg)?.span.end;
                Ok(Statement::Break(Span { start, end }))
            }
            TokenKind::Continue => {
                let start = self.match_token(TokenKind::Continue)?.span.start;
                let end = self.match_token(TokenKind::PontoVirg)?.span.end;
                Ok(Statement::Continue(Span { start, end }))
            }
            TokenKind::Retorna => self.parse_retorno().map(Statement::Return),
            TokenKind::Imprima => self.parse_escrita().map(Statement::Print),
            TokenKind::Leia => self.parse_leitura().map(Statement::Read),
            TokenKind::AbreChaves => self.parse_bloco().map(Statement::Block),
            _ => Err(ParseError {
                message: format!(
                    "comando inválido ou token inesperado '{}' ({})",
                    current.lexeme,
                    current.kind.name()
                ),
                span: current.span,
                expected: None,
                found: current.kind,
            }),
        }
    }

    /// DECL_VARIAVEL → TIPO IDENTIFICADOR ";" | TIPO IDENTIFICADOR "=" EXPRESSAO ";"
    pub fn parse_decl_variavel(&mut self) -> Result<VarDecl<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let var_type = self.parse_tipo()?;
        let id_tok = self.match_token(TokenKind::Id)?;

        let init = if self.check(TokenKind::Atrib) {
            self.match_token(TokenKind::Atrib)?;
            Some(self.parse_expressao_ou_logica()?)
        } else {
            None
        };

        let end = self.match_token(TokenKind::PontoVirg)?.span.end;

        Ok(VarDecl {
            var_type,
            name: id_tok.lexeme,
            init,
            span: Span { start, end },
        })
    }

    /// ATRIBUICAO → IDENTIFICADOR "=" EXPRESSAO ";"
    pub fn parse_atribuicao(&mut self) -> Result<Assignment<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let id_tok = self.match_token(TokenKind::Id)?;
        self.match_token(TokenKind::Atrib)?;
        let value = self.parse_expressao_ou_logica()?;
        let end = self.match_token(TokenKind::PontoVirg)?.span.end;

        Ok(Assignment {
            name: id_tok.lexeme,
            value,
            span: Span { start, end },
        })
    }

    /// ATRIB_SIMPLES → IDENTIFICADOR "=" EXPRESSAO
    pub fn parse_atrib_simples(&mut self) -> Result<Assignment<'src>, ParseError> {
        let start = self.lookahead().span.start;
        let id_tok = self.match_token(TokenKind::Id)?;
        self.match_token(TokenKind::Atrib)?;
        let value = self.parse_expressao_ou_logica()?;
        let end = value.span().end;

        Ok(Assignment {
            name: id_tok.lexeme,
            value,
            span: Span { start, end },
        })
    }

    /// CONDICIONAL → "se" "(" EXPRESSAO_LOGICA ")" BLOCO | "se" "(" EXPRESSAO_LOGICA ")" BLOCO "senao" BLOCO
    pub fn parse_condicional(&mut self) -> Result<IfStmt<'src>, ParseError> {
        let start = self.match_token(TokenKind::Se)?.span.start;
        self.match_token(TokenKind::AbrePar)?;
        let condition = self.parse_expressao_logica()?;
        self.match_token(TokenKind::FechaPar)?;
        let then_branch = self.parse_bloco()?;

        let mut end = then_branch.span.end;
        let else_branch = if self.check(TokenKind::Senao) {
            self.match_token(TokenKind::Senao)?;
            let b = self.parse_bloco()?;
            end = b.span.end;
            Some(b)
        } else {
            None
        };

        Ok(IfStmt {
            condition,
            then_branch,
            else_branch,
            span: Span { start, end },
        })
    }

    /// REPETICAO_ENQUANTO → "enquanto" "(" EXPRESSAO_LOGICA ")" BLOCO
    pub fn parse_enquanto(&mut self) -> Result<WhileStmt<'src>, ParseError> {
        let start = self.match_token(TokenKind::Enquanto)?.span.start;
        self.match_token(TokenKind::AbrePar)?;
        let condition = self.parse_expressao_logica()?;
        self.match_token(TokenKind::FechaPar)?;
        let body = self.parse_bloco()?;
        let end = body.span.end;

        Ok(WhileStmt {
            condition,
            body,
            span: Span { start, end },
        })
    }

    /// REPETICAO_PARA → "para" "(" (ATRIB_SIMPLES | DECL_VARIAVEL_SIMPLES | ε) ";"
    ///                            EXPRESSAO_LOGICA ";"
    ///                            (ATRIB_SIMPLES | ε) ")" BLOCO
    pub fn parse_para(&mut self) -> Result<ForStmt<'src>, ParseError> {
        let start = self.match_token(TokenKind::Para)?.span.start;
        self.match_token(TokenKind::AbrePar)?;

        let init = if self.check(TokenKind::Id) {
            Some(ForInit::Assignment(self.parse_atrib_simples()?))
        } else if self.is_type() {
            let var_start = self.lookahead().span.start;
            let var_type = self.parse_tipo()?;
            let id_tok = self.match_token(TokenKind::Id)?;
            let init_expr = if self.check(TokenKind::Atrib) {
                self.match_token(TokenKind::Atrib)?;
                Some(self.parse_expressao_ou_logica()?)
            } else {
                None
            };
            let var_end = init_expr
                .as_ref()
                .map(|e| e.span().end)
                .unwrap_or(id_tok.span.end);
            Some(ForInit::VarDecl(VarDecl {
                var_type,
                name: id_tok.lexeme,
                init: init_expr,
                span: Span {
                    start: var_start,
                    end: var_end,
                },
            }))
        } else {
            None
        };
        self.match_token(TokenKind::PontoVirg)?;

        let condition = if !self.check(TokenKind::PontoVirg) {
            Some(self.parse_expressao_logica()?)
        } else {
            None
        };
        self.match_token(TokenKind::PontoVirg)?;

        let increment = if self.check(TokenKind::Id) {
            Some(self.parse_atrib_simples()?)
        } else {
            None
        };
        self.match_token(TokenKind::FechaPar)?;

        let body = self.parse_bloco()?;
        let end = body.span.end;

        Ok(ForStmt {
            init,
            condition,
            increment,
            body,
            span: Span { start, end },
        })
    }

    /// ESCRITA → "imprima" "(" ARG_IMPRIMA ("," ARG_IMPRIMA)* ")" ";"
    pub fn parse_escrita(&mut self) -> Result<PrintStmt<'src>, ParseError> {
        let start = self.match_token(TokenKind::Imprima)?.span.start;
        self.match_token(TokenKind::AbrePar)?;

        let mut args = Vec::new();
        if !self.check(TokenKind::FechaPar) {
            args.push(self.parse_expressao_ou_logica()?);
            while self.check(TokenKind::Virgula) {
                self.match_token(TokenKind::Virgula)?;
                args.push(self.parse_expressao_ou_logica()?);
            }
        }
        self.match_token(TokenKind::FechaPar)?;
        let end = self.match_token(TokenKind::PontoVirg)?.span.end;

        Ok(PrintStmt {
            args,
            span: Span { start, end },
        })
    }

    /// LEITURA → "leia" "(" IDENTIFICADOR ")" ";"
    pub fn parse_leitura(&mut self) -> Result<ReadStmt<'src>, ParseError> {
        let start = self.match_token(TokenKind::Leia)?.span.start;
        self.match_token(TokenKind::AbrePar)?;
        let id_tok = self.match_token(TokenKind::Id)?;
        self.match_token(TokenKind::FechaPar)?;
        let end = self.match_token(TokenKind::PontoVirg)?.span.end;

        Ok(ReadStmt {
            var_name: id_tok.lexeme,
            span: Span { start, end },
        })
    }

    /// RETORNO → "retorna" EXPRESSAO ";" | "retorna" ";"
    pub fn parse_retorno(&mut self) -> Result<ReturnStmt<'src>, ParseError> {
        let start = self.match_token(TokenKind::Retorna)?.span.start;
        let value = if !self.check(TokenKind::PontoVirg) {
            Some(self.parse_expressao_ou_logica()?)
        } else {
            None
        };
        let end = self.match_token(TokenKind::PontoVirg)?.span.end;

        Ok(ReturnStmt {
            value,
            span: Span { start, end },
        })
    }

    // =========================================================================
    // Expressões (Lógicas, Relacionais e Aritméticas)
    // =========================================================================

    /// Suporte unificado para expressões onde lógica, relacional ou aritmética são válidas.
    #[inline]
    pub fn parse_expressao_ou_logica(&mut self) -> Result<Expr<'src>, ParseError> {
        self.parse_expressao_logica()
    }

    /// EXPRESSAO_LOGICA → EXPR_OU
    pub fn parse_expressao_logica(&mut self) -> Result<Expr<'src>, ParseError> {
        self.parse_expr_ou()
    }

    /// EXPR_OU → EXPR_E ("||" EXPR_E)*
    fn parse_expr_ou(&mut self) -> Result<Expr<'src>, ParseError> {
        let mut left = self.parse_expr_e()?;

        while self.check(TokenKind::Ou) {
            let _ = self.advance();
            let right = self.parse_expr_e()?;
            let span = Span {
                start: left.span().start,
                end: right.span().end,
            };
            left = Expr::Binary {
                left: Box::new(left),
                op: BinaryOp::Or,
                right: Box::new(right),
                span,
            };
        }

        Ok(left)
    }

    /// EXPR_E → EXPR_RELACIONAL ("&&" EXPR_RELACIONAL)*
    fn parse_expr_e(&mut self) -> Result<Expr<'src>, ParseError> {
        let mut left = self.parse_expr_relacional()?;

        while self.check(TokenKind::E) {
            let _ = self.advance();
            let right = self.parse_expr_relacional()?;
            let span = Span {
                start: left.span().start,
                end: right.span().end,
            };
            left = Expr::Binary {
                left: Box::new(left),
                op: BinaryOp::And,
                right: Box::new(right),
                span,
            };
        }

        Ok(left)
    }

    /// EXPR_RELACIONAL → "!" EXPR_RELACIONAL | EXPRESSAO_ARITMETICA (OP_RELACIONAL EXPRESSAO_ARITMETICA)?
    fn parse_expr_relacional(&mut self) -> Result<Expr<'src>, ParseError> {
        if self.check(TokenKind::Nao) {
            let op_tok = self.advance();
            let operand = self.parse_expr_relacional()?;
            let span = Span {
                start: op_tok.span.start,
                end: operand.span().end,
            };
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                operand: Box::new(operand),
                span,
            });
        }

        let left = self.parse_expressao_aritmetica()?;

        let rel_op = match self.lookahead().kind {
            TokenKind::Igualdade => Some(BinaryOp::Equal),
            TokenKind::Diferente => Some(BinaryOp::NotEqual),
            TokenKind::MenorQue => Some(BinaryOp::LessThan),
            TokenKind::MaiorQue => Some(BinaryOp::GreaterThan),
            TokenKind::MenorIgual => Some(BinaryOp::LessEqual),
            TokenKind::MaiorIgual => Some(BinaryOp::GreaterEqual),
            _ => None,
        };

        if let Some(op) = rel_op {
            self.advance(); // Consome o operador relacional
            let right = self.parse_expressao_aritmetica()?;
            let span = Span {
                start: left.span().start,
                end: right.span().end,
            };
            Ok(Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span,
            })
        } else {
            Ok(left)
        }
    }

    /// EXPRESSAO → TERMO (( "+" | "-" ) TERMO)*
    pub fn parse_expressao_aritmetica(&mut self) -> Result<Expr<'src>, ParseError> {
        let mut left = self.parse_termo()?;

        while self.check(TokenKind::Soma) || self.check(TokenKind::Subtracao) {
            let op_tok = self.advance();
            let op = if op_tok.kind == TokenKind::Soma {
                BinaryOp::Add
            } else {
                BinaryOp::Sub
            };
            let right = self.parse_termo()?;
            let span = Span {
                start: left.span().start,
                end: right.span().end,
            };
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span,
            };
        }

        Ok(left)
    }

    /// TERMO → FATOR (( "*" | "/" | "%" ) FATOR)*
    pub fn parse_termo(&mut self) -> Result<Expr<'src>, ParseError> {
        let mut left = self.parse_fator()?;

        while self.check(TokenKind::Multiplica)
            || self.check(TokenKind::Divisao)
            || self.check(TokenKind::Modulo)
        {
            let op_tok = self.advance();
            let op = match op_tok.kind {
                TokenKind::Multiplica => BinaryOp::Mul,
                TokenKind::Divisao => BinaryOp::Div,
                TokenKind::Modulo => BinaryOp::Mod,
                _ => unreachable!(),
            };
            let right = self.parse_fator()?;
            let span = Span {
                start: left.span().start,
                end: right.span().end,
            };
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span,
            };
        }

        Ok(left)
    }

    /// FATOR (Regra corrigida e completada):
    /// FATOR → IDENTIFICADOR | IDENTIFICADOR "(" ARGS ")" |
    ///         LITERAL_INT | LITERAL_DEC | LITERAL_TEXTO | LITERAL_BOOL |
    ///         "(" EXPRESSAO_LOGICA ")" | "-" FATOR | "+" FATOR
    pub fn parse_fator(&mut self) -> Result<Expr<'src>, ParseError> {
        let current = *self.lookahead();

        if current.kind == TokenKind::Subtracao {
            self.advance();
            let operand = self.parse_fator()?;
            let span = Span {
                start: current.span.start,
                end: operand.span().end,
            };
            return Ok(Expr::Unary {
                op: UnaryOp::Neg,
                operand: Box::new(operand),
                span,
            });
        }

        if current.kind == TokenKind::Soma {
            self.advance();
            return self.parse_fator();
        }

        match current.kind {
            TokenKind::Id => {
                let id_tok = self.advance();
                if self.check(TokenKind::AbrePar) {
                    let call = self.parse_chamada_restante(id_tok)?;
                    Ok(Expr::Call(call))
                } else {
                    Ok(Expr::Identifier {
                        name: id_tok.lexeme,
                        span: id_tok.span,
                    })
                }
            }
            TokenKind::NumInt => {
                let tok = self.advance();
                Ok(Expr::IntLiteral {
                    raw: tok.lexeme,
                    span: tok.span,
                })
            }
            TokenKind::NumDec => {
                let tok = self.advance();
                Ok(Expr::DecLiteral {
                    raw: tok.lexeme,
                    span: tok.span,
                })
            }
            TokenKind::Texto => {
                let tok = self.advance();
                let content = tok.text_content().unwrap_or(tok.lexeme);
                Ok(Expr::StringLiteral {
                    content,
                    span: tok.span,
                })
            }
            TokenKind::Verdadeiro => {
                let tok = self.advance();
                Ok(Expr::BoolLiteral {
                    value: true,
                    span: tok.span,
                })
            }
            TokenKind::Falso => {
                let tok = self.advance();
                Ok(Expr::BoolLiteral {
                    value: false,
                    span: tok.span,
                })
            }
            TokenKind::AbrePar => {
                let start = self.advance().span.start;
                let inner = self.parse_expressao_ou_logica()?;
                let end = self.match_token(TokenKind::FechaPar)?.span.end;
                Ok(Expr::Grouping {
                    expr: Box::new(inner),
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

    /// Auxiliar para chamada de função a partir de um identificador.
    fn parse_chamada_funcao(&mut self) -> Result<FunctionCall<'src>, ParseError> {
        let id_tok = self.match_token(TokenKind::Id)?;
        self.parse_chamada_restante(id_tok)
    }

    fn parse_chamada_restante(
        &mut self,
        id_tok: Token<'src>,
    ) -> Result<FunctionCall<'src>, ParseError> {
        let start = id_tok.span.start;
        self.match_token(TokenKind::AbrePar)?;

        let mut args = Vec::new();
        if !self.check(TokenKind::FechaPar) {
            args.push(self.parse_expressao_ou_logica()?);
            while self.check(TokenKind::Virgula) {
                self.match_token(TokenKind::Virgula)?;
                args.push(self.parse_expressao_ou_logica()?);
            }
        }
        let end = self.match_token(TokenKind::FechaPar)?.span.end;

        Ok(FunctionCall {
            callee: id_tok.lexeme,
            args,
            span: Span { start, end },
        })
    }
}

/// Ponto de entrada conveniente para executar a análise sintática a partir de uma fatia de tokens.
pub fn parse<'src>(tokens: &'src [Token<'src>]) -> Result<Program<'src>, ParseError> {
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenize;

    fn parse_ok(src: &str) -> bool {
        let tokens = tokenize(src).expect("falha ao tokenizar entrada no teste");
        parse(&tokens).is_ok()
    }

    #[test]
    fn test_valid_declaracao_simples() {
        assert!(parse_ok("int i;"));
    }

    #[test]
    fn test_valid_declaracao_dec_init() {
        assert!(parse_ok("dec nota_final = 8,50;"));
    }

    #[test]
    fn test_valid_atribuicao_simples() {
        assert!(parse_ok("taxa = 40;"));
    }

    #[test]
    fn test_valid_condicional_simples() {
        assert!(parse_ok("se (nota_final >= 7,00) { imprima(\"Aprovado\"); }"));
    }

    #[test]
    fn test_valid_condicional_composta() {
        assert!(parse_ok("se (x > 0) { taxa = 10; } senao { taxa = 0; }"));
    }

    #[test]
    fn test_valid_laco_enquanto() {
        assert!(parse_ok("enquanto (i < 10) { i = i + 1; }"));
    }

    #[test]
    fn test_valid_laco_para() {
        assert!(parse_ok("para (i = 0; i < 10; i = i + 1) { imprima(i); }"));
    }

    #[test]
    fn test_valid_comando_saida() {
        assert!(parse_ok("imprima(\"Aprovado\");"));
    }

    #[test]
    fn test_valid_comando_entrada() {
        assert!(parse_ok("leia(posicao);"));
    }

    #[test]
    fn test_valid_funcao_principal() {
        assert!(parse_ok("vazio principal() { dec nota_final = 8,50; }"));
    }

    #[test]
    fn test_valid_multiplos_parametros() {
        assert!(parse_ok("int soma(int a, dec b, carac c) { retorna a + b; }"));
    }

    #[test]
    fn test_valid_pare_e_continue() {
        assert!(parse_ok("vazio loop() { enquanto (verdadeiro) { pare; continue; } }"));
    }

    #[test]
    fn test_valid_expressoes_logicas_complexas() {
        assert!(parse_ok("se (x > 0 && taxa == 10 || !ativo) { retorna; }"));
    }

    #[test]
    fn test_invalid_falta_ponto_virgula() {
        assert!(!parse_ok("int x"));
    }

    #[test]
    fn test_invalid_condicional_sem_parenteses() {
        assert!(!parse_ok("se nota_final >= 7,00 { imprima(\"Aprovado\"); }"));
    }

    #[test]
    fn test_invalid_ordem_atribuicao() {
        assert!(!parse_ok("= taxa 40;"));
    }

    #[test]
    fn test_invalid_bloco_nao_fechado() {
        assert!(!parse_ok("se (x > 0) { imprima(x);"));
    }
}
