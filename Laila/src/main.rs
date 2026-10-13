use laila_lexer::{
    number::{LiteralPrecision, NumCompiler},
    parse, tokenize, Parser, TokenKind,
};
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::process::ExitCode;

const HELP: &str = "Uso: laila [opções] [arquivo|-]

Opções:
  --tokens        Executa apenas o analisador léxico e imprime os tokens.
  --lower-num     Mostra tipo, bytes e literal C de cada constante numérica.
  --ast           Executa o analisador sintático e imprime a árvore sintática detalhada.
  -h, --help      Mostra esta mensagem de ajuda.

Sem opções específicas de saída, executa o analisador sintático e confirma
a validade sintática do código-fonte.
Sem arquivo, lê o código Laila da entrada padrão (stdin).

Exemplos:
  laila examples/aprovacao.laila
  laila --tokens examples/aprovacao.laila
  laila --ast examples/aprovacao.laila
  laila --lower-num examples/numeros.laila
";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    ParseAndValidate,
    PrintAst,
    TokensOnly,
    LowerNum,
}

fn run() -> Result<(), String> {
    let mut path = None;
    let mut mode = Mode::ParseAndValidate;
    let mut positional_only = false;

    for arg in env::args().skip(1) {
        if !positional_only {
            if arg == "--help" || arg == "-h" {
                print!("{HELP}");
                return Ok(());
            }
            if arg == "--" {
                positional_only = true;
                continue;
            }
            if arg == "--tokens" {
                mode = Mode::TokensOnly;
                continue;
            }
            if arg == "--ast" {
                mode = Mode::PrintAst;
                continue;
            }
            if arg == "--lower-num" {
                mode = Mode::LowerNum;
                continue;
            }
            if arg.starts_with('-') && arg != "-" {
                return Err(format!("opção desconhecida: {arg}"));
            }
        }
        if path.is_some() {
            return Err("informe apenas um arquivo por execução".into());
        }
        path = Some(arg);
    }

    let filename = path.as_deref().unwrap_or("-");
    let source = if filename == "-" {
        let mut source = String::new();
        io::stdin()
            .read_to_string(&mut source)
            .map_err(|e| format!("falha ao ler a entrada UTF-8: {e}"))?;
        source
    } else {
        fs::read_to_string(filename)
            .map_err(|e| format!("não foi possível ler {filename:?}: {e}"))?
    };

    let label = if filename == "-" {
        "<entrada>"
    } else {
        filename
    };

    // 1. Análise Léxica
    let tokens = tokenize(&source).map_err(|e| format!("{label}:{e}"))?;

    // 2. Modos de execução
    match mode {
        Mode::TokensOnly => {
            let mut output = String::new();
            for token in &tokens {
                output.push_str(&format!(
                    "{}:{}\t{token}\n",
                    token.span.start.line, token.span.start.column
                ));
            }
            print_output(&output)?;
        }
        Mode::LowerNum => {
            let compiler = NumCompiler::new();
            let mut output = String::new();
            for token in &tokens {
                output.push_str(&format!(
                    "{}:{}\t{token}",
                    token.span.start.line, token.span.start.column
                ));
                if matches!(token.kind, TokenKind::NumInt | TokenKind::NumDec) {
                    let number = compiler
                        .lower_literal(token)
                        .map_err(|e| format!("{label}:{e}"))?;
                    output.push_str(&format!(
                        "\tC: {} ({} bytes) {}",
                        number.representation.c_type(),
                        number.representation.bytes(),
                        number.c_literal
                    ));
                    if number.precision == LiteralPrecision::RoundedToDouble {
                        output.push_str(" [arredondado]");
                    }
                }
                output.push('\n');
            }
            print_output(&output)?;
        }
        Mode::PrintAst => {
            let mut parser = Parser::new(&tokens);
            let program = parser.parse_program().map_err(|e| format!("{label}:{e}"))?;
            println!("{program:#?}");
        }
        Mode::ParseAndValidate => {
            let mut parser = Parser::new(&tokens);
            let program = parser.parse_program().map_err(|e| format!("{label}:{e}"))?;
            println!(
                "Análise sintática concluída com sucesso! ({label}: {} elemento(s) no programa)",
                program.items.len()
            );
        }
    }

    Ok(())
}

fn print_output(output: &str) -> Result<(), String> {
    match io::stdout().lock().write_all(output.as_bytes()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(format!("falha ao escrever a saída: {e}")),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("erro: {message}");
            ExitCode::FAILURE
        }
    }
}
