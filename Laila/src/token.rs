use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub byte: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    TipoNum,
    TipoInt,
    TipoDec,
    TipoCarac,
    TipoVazio,
    Se,
    Senao,
    Enquanto,
    Para,
    Pare,
    Continue,
    Retorna,
    Imprima,
    Leia,
    Id,
    NumInt,
    NumDec,
    Verdadeiro,
    Falso,
    Texto,
    Atrib,
    Soma,
    Subtracao,
    Multiplica,
    Divisao,
    Modulo,
    Igualdade,
    Diferente,
    MaiorQue,
    MenorQue,
    MaiorIgual,
    MenorIgual,
    E,
    Ou,
    Nao,
    PontoVirg,
    AbreChaves,
    FechaChaves,
    AbrePar,
    FechaPar,
    AbreColch,
    FechaColch,
    Virgula,
    Eof,
}

impl TokenKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::TipoNum => "TK_TIPO_NUM",
            Self::TipoInt => "TK_TIPO_INT",
            Self::TipoDec => "TK_TIPO_DEC",
            Self::TipoCarac => "TK_TIPO_CARAC",
            Self::TipoVazio => "TK_TIPO_VAZIO",
            Self::Se => "TK_SE",
            Self::Senao => "TK_SENAO",
            Self::Enquanto => "TK_ENQUANTO",
            Self::Para => "TK_PARA",
            Self::Pare => "TK_PARE",
            Self::Continue => "TK_CONTINUE",
            Self::Retorna => "TK_RETORNA",
            Self::Imprima => "TK_IMPRIMA",
            Self::Leia => "TK_LEIA",
            Self::Id => "TK_ID",
            Self::NumInt => "TK_NUM_INT",
            Self::NumDec => "TK_NUM_DEC",
            Self::Verdadeiro => "TK_VERDADEIRO",
            Self::Falso => "TK_FALSO",
            Self::Texto => "TK_TEXTO",
            Self::Atrib => "TK_ATRIB",
            Self::Soma => "TK_SOMA",
            Self::Subtracao => "TK_SUBTRACAO",
            Self::Multiplica => "TK_MULTIPLICA",
            Self::Divisao => "TK_DIVISAO",
            Self::Modulo => "TK_MODULO",
            Self::Igualdade => "TK_IGUALDADE",
            Self::Diferente => "TK_DIFERENTE",
            Self::MaiorQue => "TK_MAIOR_QUE",
            Self::MenorQue => "TK_MENOR_QUE",
            Self::MaiorIgual => "TK_MAIOR_IGUAL",
            Self::MenorIgual => "TK_MENOR_IGUAL",
            Self::E => "TK_E",
            Self::Ou => "TK_OU",
            Self::Nao => "TK_NAO",
            Self::PontoVirg => "TK_PONTO_VIRG",
            Self::AbreChaves => "TK_ABRE_CHAVES",
            Self::FechaChaves => "TK_FECHA_CHAVES",
            Self::AbrePar => "TK_ABRE_PAR",
            Self::FechaPar => "TK_FECHA_PAR",
            Self::AbreColch => "TK_ABRE_COLCH",
            Self::FechaColch => "TK_FECHA_COLCH",
            Self::Virgula => "TK_VIRGULA",
            Self::Eof => "TK_EOF",
        }
    }

    pub const fn is_numeric_type(self) -> bool {
        matches!(self, Self::TipoNum | Self::TipoInt | Self::TipoDec)
    }

    pub(crate) fn keyword(word: &str) -> Option<Self> {
        Some(match word {
            "num" => Self::TipoNum,
            "int" => Self::TipoInt,
            "dec" => Self::TipoDec,
            "carac" => Self::TipoCarac,
            "vazio" => Self::TipoVazio,
            "se" => Self::Se,
            "senao" => Self::Senao,
            "enquanto" => Self::Enquanto,
            "para" => Self::Para,
            "pare" => Self::Pare,
            "continue" => Self::Continue,
            "retorna" => Self::Retorna,
            "imprima" => Self::Imprima,
            "leia" => Self::Leia,
            "verdadeiro" => Self::Verdadeiro,
            "falso" => Self::Falso,
            _ => return None,
        })
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token<'src> {
    pub kind: TokenKind,
    pub lexeme: &'src str,
    pub span: Span,
}

impl<'src> Token<'src> {
    pub fn text_content(&self) -> Option<&'src str> {
        if self.kind != TokenKind::Texto {
            return None;
        }
        self.lexeme.strip_prefix('"')?.strip_suffix('"')
    }
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            TokenKind::Id | TokenKind::NumInt | TokenKind::NumDec => {
                write!(f, "<{}, {:?}>", self.kind, self.lexeme)
            }
            TokenKind::Texto => write!(
                f,
                "<{}, {:?}>",
                self.kind,
                self.text_content().unwrap_or(self.lexeme)
            ),
            _ => write!(f, "<{}>", self.kind),
        }
    }
}
