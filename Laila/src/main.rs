use laila_lexer::{
    analyze_semantics,
    number::{LiteralPrecision, NumCompiler},
    tokenize, CstParser, Parser, TokenKind,
};
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::process::ExitCode;

const HELP: &str = "Uso: laila [opções] [arquivo|-]

Opções:
  --tokens              Executa apenas o analisador léxico e imprime os tokens.
  --lower-num           Mostra tipo, bytes e literal C de cada constante numérica.
  --ast                 Executa o analisador sintático e imprime a árvore sintática abstrata (AST).
  --arvore, --cst       Executa e imprime a árvore de derivação completa (CST), com todos os terminais (=, ;, etc.).
  --simbolos, --tabela  Executa a análise semântica e imprime a Tabela de Símbolos (escopos B0, B1...) e referências.
  --completo, --front   Exibe todas as etapas do Front-End (Tokens + Árvore CST + Tabela de Símbolos + Veredito).
  -h, --help            Mostra esta mensagem de ajuda.

Sem opções específicas de saída, executa o Front-End completo (análise léxica,
sintática e semântica) e informa se o programa é válido.
Sem arquivo, lê o código Laila da entrada padrão (stdin).

Exemplos:
  laila examples/aprovacao.laila
  laila --completo examples/aprovacao.laila
  laila --tokens examples/aprovacao.laila
  laila --arvore examples/aprovacao.laila
  laila --simbolos examples/escopos.laila
  laila --lower-num examples/numeros.laila
";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    ValidateAll,
    FullFrontEnd,
    PrintAst,
    PrintCst,
    PrintSymbols,
    TokensOnly,
    LowerNum,
}

fn run() -> Result<(), String> {
    let mut path = None;
    let mut mode = Mode::ValidateAll;
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
            if arg == "--arvore" || arg == "--cst" {
                mode = Mode::PrintCst;
                continue;
            }
            if arg == "--simbolos" || arg == "--tabela" || arg == "--semantico" {
                mode = Mode::PrintSymbols;
                continue;
            }
            if arg == "--completo" || arg == "--front" {
                mode = Mode::FullFrontEnd;
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
    let tokens = tokenize(&source).map_err(|e| format!("[ERRO LÉXICO] {label}:{e}"))?;

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
            let program = parser
                .parse_program()
                .map_err(|e| format!("[ERRO SINTÁTICO] {label}:{e}"))?;
            println!("{program:#?}");
        }
        Mode::PrintCst => {
            let mut parser = CstParser::new(&tokens);
            let cst = parser
                .parse_program()
                .map_err(|e| format!("[ERRO SINTÁTICO] {label}:{e}"))?;
            print_output(&cst.format_tree())?;
        }
        Mode::PrintSymbols => {
            let mut parser = Parser::new(&tokens);
            let program = parser
                .parse_program()
                .map_err(|e| format!("[ERRO SINTÁTICO] {label}:{e}"))?;
            let table = analyze_semantics(&program).map_err(|errs| {
                errs.iter()
                    .map(|e| format!("[ERRO SEMÂNTICO] {label}:{e}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
            print_output(&table.format_table())?;
        }
        Mode::FullFrontEnd => {
            let mut output = String::new();
            output.push_str("========================================================================\n");
            output.push_str("                    1. ANÁLISE LÉXICA (TOKENS)                          \n");
            output.push_str("========================================================================\n");
            for token in &tokens {
                output.push_str(&format!(
                    "  {}:{}\t{token}\n",
                    token.span.start.line, token.span.start.column
                ));
            }

            let mut cst_parser = CstParser::new(&tokens);
            let cst = cst_parser
                .parse_program()
                .map_err(|e| format!("[ERRO SINTÁTICO] {label}:{e}"))?;

            output.push_str("\n========================================================================\n");
            output.push_str("            2. ANÁLISE SINTÁTICA (ÁRVORE DE DERIVAÇÃO - CST)            \n");
            output.push_str("========================================================================\n");
            output.push_str(&cst.format_tree());

            let mut ast_parser = Parser::new(&tokens);
            let program = ast_parser
                .parse_program()
                .map_err(|e| format!("[ERRO SINTÁTICO] {label}:{e}"))?;

            let table = analyze_semantics(&program).map_err(|errs| {
                errs.iter()
                    .map(|e| format!("[ERRO SEMÂNTICO] {label}:{e}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;

            output.push('\n');
            output.push_str(&table.format_table());
            output.push_str("\n========================================================================\n");
            output.push_str(&format!(
                "VEREDITO FINAL ({label}):\n  [OK] Análise Léxica     ({} tokens)\n  [OK] Análise Sintática  ({} construção(ões) de topo)\n  [OK] Análise Semântica  ({} escopo(s), {} referência(s) resolvida(s))\n  => O programa é LÉXICA, SINTÁTICA e SEMANTICAMENTE VÁLIDO!\n",
                tokens.len(),
                program.items.len(),
                table.scopes().len(),
                table.references().len()
            ));
            output.push_str("========================================================================\n");
            print_output(&output)?;
        }
        Mode::ValidateAll => {
            let mut parser = Parser::new(&tokens);
            let program = parser
                .parse_program()
                .map_err(|e| format!("[ERRO SINTÁTICO] {label}:{e}"))?;
            let table = analyze_semantics(&program).map_err(|errs| {
                errs.iter()
                    .map(|e| format!("[ERRO SEMÂNTICO] {label}:{e}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
            println!("[1/3] Análise Léxica:    OK ({} tokens reconhecidos)", tokens.len());
            println!(
                "[2/3] Análise Sintática: OK ({} elemento(s) na AST)",
                program.items.len()
            );
            println!(
                "[3/3] Análise Semântica: OK ({} escopo(s) na Tabela de Símbolos, {} referência(s) resolvida(s))",
                table.scopes().len(),
                table.references().len()
            );
            println!("=> Resultado ({label}): O código é LÉXICA, SINTÁTICA e SEMANTICAMENTE VÁLIDO.");
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
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
