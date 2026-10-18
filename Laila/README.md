# Comandos para testar a árvore

## 1. Executar todos os testes unitários (27 testes passando)

cargo test

## 2. Exibir a árvore de derivação completa (com '=' e ';')

cargo run -- --arvore examples/aprovacao.laila
cargo run -- --arvore examples/numeros.laila

## 3. Inspecionar o tipo C e os bytes alocados de cada número

cargo run -- --lower-num examples/numeros.laila
