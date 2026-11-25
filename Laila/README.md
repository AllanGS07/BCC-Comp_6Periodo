# Compilador Laila

Compilador completo em Rust para a linguagem **Laila** (sintaxe em português, vírgula decimal, tipo numérico automático `num` e tipos estáticos `int`, `dec`, `carac`, `vazio`), com **RI (TAC)**, **Otimizador**, **Interpretador da RI** e **Gerador de Código C11** via **GCC**.

## Testes

```powershell
cargo test
```

## Uso no terminal

```powershell
# Validar programa (léxico + sintático + semântico)
cargo run -- examples/aprovacao.laila

# Inspecionar cada fase
cargo run -- --tokens examples/aprovacao.laila
cargo run -- --simbolos examples/escopos.laila
cargo run -- --ri examples/otimizacao.laila
cargo run -- --c examples/fatorial.laila

# Executar ou compilar
cargo run -- --interpretar examples/fatorial.laila
cargo run -- --executar examples/fatorial.laila
cargo run -- --compilar examples/aprovacao.laila -o aprovacao.exe
```

## Exemplos de erro

```powershell
cargo run -- examples/erro_lexico.laila
cargo run -- examples/erro_sintatico.laila
cargo run -- examples/erro_semantico.laila
```
