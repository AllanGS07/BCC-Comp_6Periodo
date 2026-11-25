use laila::{
    analyze_semantics, build_ir, compile_c_file, generate_c, interpret_ir, optimize_ir, tokenize,
    Parser, Program, SymbolTable, Token,
};
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const HELP: &str = "Uso: laila [opção] [arquivo|-]

Opções:
  --tokens        Imprime os tokens (Análise Léxica).
  --simbolos      Imprime a Tabela de Símbolos e escopos (Análise Semântica).
  --ri            Imprime a RI (TAC) original, otimizações e RI otimizada.
  --c             Imprime o código C11 gerado (ou salva com -o).
  --compilar      Gera C11 e compila executável nativo com GCC (use -o).
  --executar      Compila para C11 com GCC e executa o binário nativo.
  --interpretar   Executa a RI otimizada diretamente na VM interna.
  -o <caminho>    Arquivo de saída para --c ou --compilar.
  -h, --help      Mostra esta ajuda.

Sem opção, valida o código-fonte (léxico, sintático e semântico).
";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Validate,
    Tokens,
    Symbols,
    Ir,
    EmitC,
    CompileExe,
    RunNative,
    InterpretIr,
}

fn run() -> Result<(), String> {
    let mut path: Option<String> = None;
    let mut output_path: Option<String> = None;
    let mut mode = Mode::Validate;
    let mut positional_only = false;
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        if !positional_only {
            match arg.as_str() {
                "-h" | "--help" => {
                    print!("{HELP}");
                    return Ok(());
                }
                "--" => {
                    positional_only = true;
                    continue;
                }
                "-o" => {
                    output_path = Some(
                        args.next()
                            .ok_or_else(|| "esperado caminho após '-o'".to_string())?,
                    );
                    continue;
                }
                "--tokens" => {
                    mode = Mode::Tokens;
                    continue;
                }
                "--simbolos" => {
                    mode = Mode::Symbols;
                    continue;
                }
                "--ri" => {
                    mode = Mode::Ir;
                    continue;
                }
                "--c" => {
                    mode = Mode::EmitC;
                    continue;
                }
                "--compilar" => {
                    mode = Mode::CompileExe;
                    continue;
                }
                "--executar" => {
                    mode = Mode::RunNative;
                    continue;
                }
                "--interpretar" => {
                    mode = Mode::InterpretIr;
                    continue;
                }
                other if other.starts_with('-') && other != "-" => {
                    return Err(format!("opção desconhecida: {other}"));
                }
                _ => {}
            }
        }
        if path.is_some() {
            return Err("informe apenas um arquivo por execução".into());
        }
        path = Some(arg);
    }

    let filename = path.as_deref().unwrap_or("-");
    let source = if filename == "-" {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("falha ao ler a entrada UTF-8: {e}"))?;
        buf
    } else {
        fs::read_to_string(filename)
            .map_err(|e| format!("não foi possível ler {filename:?}: {e}"))?
    };

    let label = if filename == "-" { "<entrada>" } else { filename };
    let tokens = tokenize(&source).map_err(|e| format!("[ERRO LÉXICO] {label}:{e}"))?;

    match mode {
        Mode::Tokens => {
            let mut out = String::new();
            for tok in &tokens {
                out.push_str(&format!(
                    "{}:{}\t{tok}\n",
                    tok.span.start.line, tok.span.start.column
                ));
            }
            print_output(&out)?;
        }
        Mode::Symbols => {
            let (_, table) = run_front_end(&tokens, label)?;
            print_output(&table.format_table())?;
        }
        Mode::Ir => {
            let (program, table) = run_front_end(&tokens, label)?;
            let ir = build_ir(&program, &table);
            let (opt, report) = optimize_ir(&ir);
            let out = format!(
                "=== RI ORIGINAL ===\n{}\n{}\n=== RI OTIMIZADA ===\n{}",
                ir.format_ir(),
                report.format_summary(ir.instruction_count(), opt.instruction_count()),
                opt.format_ir()
            );
            print_output(&out)?;
        }
        Mode::EmitC => {
            let (program, table) = run_front_end(&tokens, label)?;
            let c_code = generate_c(&program, &table);
            if let Some(out_file) = output_path {
                fs::write(&out_file, &c_code)
                    .map_err(|e| format!("falha ao gravar '{out_file}': {e}"))?;
                println!("Código C gerado em '{out_file}'.");
            } else {
                print_output(&c_code)?;
            }
        }
        Mode::CompileExe => {
            let (program, table) = run_front_end(&tokens, label)?;
            let c_code = generate_c(&program, &table);
            let stem = Path::new(filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .filter(|s| *s != "-")
                .unwrap_or("programa");

            let exe_path = match output_path {
                Some(p) => PathBuf::from(p),
                None if cfg!(windows) => PathBuf::from(format!("{stem}.exe")),
                None => PathBuf::from(stem),
            };
            let c_path = exe_path.with_extension("c");
            fs::write(&c_path, &c_code)
                .map_err(|e| format!("falha ao gravar '{}': {e}", c_path.display()))?;
            compile_c_file(&c_path, &exe_path)?;
            println!(
                "Compilado com sucesso: {} -> {}",
                c_path.display(),
                exe_path.display()
            );
        }
        Mode::RunNative => {
            let (program, table) = run_front_end(&tokens, label)?;
            let c_code = generate_c(&program, &table);
            let temp_dir = env::temp_dir();
            let pid = std::process::id();
            let c_path = temp_dir.join(format!("laila_run_{pid}.c"));
            let exe_path = temp_dir.join(if cfg!(windows) {
                format!("laila_run_{pid}.exe")
            } else {
                format!("laila_run_{pid}")
            });

            fs::write(&c_path, &c_code)
                .map_err(|e| format!("falha ao gravar arquivo temporário: {e}"))?;
            let compile_res = compile_c_file(&c_path, &exe_path);
            let _ = fs::remove_file(&c_path);
            compile_res?;

            let status = Command::new(&exe_path)
                .status()
                .map_err(|e| format!("falha ao executar '{}': {e}", exe_path.display()));
            let _ = fs::remove_file(&exe_path);
            let status = status?;
            if !status.success() {
                return Err(format!("programa encerrou com código {:?}", status.code()));
            }
        }
        Mode::InterpretIr => {
            let (program, table) = run_front_end(&tokens, label)?;
            let ir = build_ir(&program, &table);
            let (opt, _) = optimize_ir(&ir);
            interpret_ir(&opt).map_err(|e| format!("[ERRO DE EXECUÇÃO] {label}: {e}"))?;
        }
        Mode::Validate => {
            let (_, table) = run_front_end(&tokens, label)?;
            println!(
                "OK ({label}): {} tokens, {} escopo(s), {} referência(s).",
                tokens.len(),
                table.scopes().len(),
                table.references().len()
            );
        }
    }

    Ok(())
}

fn run_front_end<'src>(
    tokens: &'src [Token<'src>],
    label: &str,
) -> Result<(Program<'src>, SymbolTable), String> {
    let program = Parser::new(tokens)
        .parse_program()
        .map_err(|e| format!("[ERRO SINTÁTICO] {label}:{e}"))?;
    let table = analyze_semantics(&program).map_err(|errs| {
        errs.iter()
            .map(|e| format!("[ERRO SEMÂNTICO] {label}:{e}"))
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    Ok((program, table))
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
