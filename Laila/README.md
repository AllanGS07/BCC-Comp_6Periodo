# Compilador Laila — Front-End Completo (Léxico, Sintático, Semântico e Tabela de Símbolos)

## 1. Executar todos os testes automatizados (38 testes)

```powershell
cargo test
```

## 2. Executar o Front-End Completo (Léxico + Sintático + Semântico + Tabela de Símbolos)

```powershell
# Validação direta
cargo run -- examples/aprovacao.laila
cargo run -- examples/funcoes.laila
cargo run -- examples/escopos.laila
cargo run -- examples/numeros.laila

# Exibição completa de todas as etapas (Tokens + Árvore CST + Tabela de Símbolos + Veredito)
cargo run -- --completo examples/aprovacao.laila
```

## 3. Exibir a Tabela de Símbolos (Escopos B0, B1, B2... e Resolução de Referências)

```powershell
cargo run -- --simbolos examples/escopos.laila
cargo run -- --simbolos examples/aprovacao.laila
cargo run -- --simbolos examples/funcoes.laila
```

## 4. Testar Detecção de Erros (Léxicos, Sintáticos e Semânticos)

```powershell
cargo run -- examples/erro_lexico.laila
cargo run -- examples/erro_sintatico.laila
cargo run -- examples/erro_semantico.laila
```

## 5. Exibir apenas Tokens ou Árvore de Derivação (CST)

```powershell
cargo run -- --tokens examples/aprovacao.laila
cargo run -- --arvore examples/aprovacao.laila
cargo run -- --lower-num examples/numeros.laila
```


