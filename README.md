# BCC-Comp_6Periodo

## Roteiro de Comandos — Apresentação ao Vivo

> Execute a partir da pasta `Laila/` (`cd Laila`).

### 1. Front-End (Validação, Léxico, Semântico e Erros)

```powershell
# Validação completa do Front-End (léxico + sintático + semântico)
cargo run -- examples/aprovacao.laila

# Tokens reconhecidos (Análise Léxica + vírgula decimal BR)
cargo run -- --tokens examples/aprovacao.laila

# Tabela de Símbolos, árvore de escopos (B0..B4) e shadowing
cargo run -- --simbolos examples/escopos.laila

# Detecção de erros (léxico, sintático e 5 erros semânticos juntos)
cargo run -- examples/erro_lexico.laila
cargo run -- examples/erro_sintatico.laila
cargo run -- examples/erro_semantico.laila
```

### 2. Back-End (RI/TAC, Otimizador, Geração C11 e VM)
``` powershell
# RI original (16 instruções) vs RI otimizada (2 instruções)
cargo run -- --ri examples/otimizacao.laila

# Código C11 gerado (protótipos, tipos inferidos e printf)
cargo run -- --c examples/fatorial.laila

# Comparação de saída: VM da RI vs binário nativo compilado pelo GCC
cargo run -- --interpretar examples/fatorial.laila
cargo run -- --executar examples/fatorial.laila

# Inferência automática do tipo `num` (int, float, double)
cargo run -- --executar examples/numeros.laila

# Compilação para executável nativo (.exe)
cargo run -- --compilar examples/aprovacao.laila -o aprovacao.exe
.\aprovacao.exe
```

### 3. Testes Automatizados (46 testes, incluindo paridade VM vs GCC)
``` powershell
cargo test
```
