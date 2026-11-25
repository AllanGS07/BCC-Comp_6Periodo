# Documentação do Back-End e Roteiro da Apresentação Final (15–20 min)

Este documento reúne a arquitetura completa do **Back-End** da linguagem **Laila** e um roteiro pronto para a apresentação final de **15 a 20 minutos**.

---

## 1. Arquitetura Geral do Compilador (`src/`)

| Fase | Módulo | Responsabilidade |
| :--- | :--- | :--- |
| **Front-End — Léxico** | [src/lexer.rs](src/lexer.rs), [src/token.rs](src/token.rs) | Tokenização UTF-8, comentários (`//`, `/* */`), números no padrão BR (`3,14`, `1.000`) e rastreamento de linha/coluna. |
| **Front-End — Sintático** | [src/parser.rs](src/parser.rs), [src/ast.rs](src/ast.rs) | Analisador Descendente Recursivo $LL$ com geração da **AST**. |
| **Front-End — Semântico** | [src/symbol_table.rs](src/symbol_table.rs), [src/semantic.rs](src/semantic.rs), [src/number.rs](src/number.rs) | Tabela de Símbolos em árvore de escopos (`B0, B1, ...`), checagem de tipos e inferência automática de `num` (`int`, `long long`, `float`, `double`). |
| **Back-End — RI (TAC/C3E)** | [src/ir.rs](src/ir.rs) | Conversão da AST + Tabela de Símbolos para **Código de Três Endereços** linear com temporários (`t0, t1...`), rótulos (`L0_...`) e resolução de escopos (`var@Bk`). |
| **Back-End — Otimização** | [src/optimizer.rs](src/optimizer.rs) | Passes iterativos até ponto fixo: *Constant Folding*, *Simplificação Algébrica*, *Propagação Local de Constantes/Cópias*, *Branch Folding* e *Dead Code Elimination*. |
| **Back-End — Interpretação** | [src/interpreter.rs](src/interpreter.rs) | Máquina Virtual baseada em pilha de chamadas que executa diretamente a RI otimizada (`imprima`, `leia`, recursão, controle de fluxo). |
| **Back-End — Geração C** | [src/codegen_c.rs](src/codegen_c.rs) | Transpilação para **C11** limpo e compilação nativa automática via **GCC** (`-std=c11 -O2 -Wall -Wextra`). |

---

## 2. Tipagem Automática (`num`) vs Tipagem Estática (`int`, `dec`)

- **Tipagem Estática (`int`, `dec`, `carac`):**
  - `int` só aceita valores inteiros (ex.: `int idade = 21;`). Atribuir `8,50` gera erro semântico.
  - `dec` só aceita valores decimais (ex.: `dec nota = 8,50;`). Atribuir `10` gera erro semântico.
- **Tipagem Automática (`num`):**
  - `num` escolhe automaticamente a menor representação C segura (`int` 4B, `long long` 8B, `float` 4B ou `double` 8B) analisando todas as inicializações e atribuições daquela variável em ponto fixo:
    - `num idade = 21;` $\to$ `int` (4 bytes)
    - `num preco = 19,90;` $\to$ `float` (4 bytes)
    - `num pi_preciso = 3,141592653589793;` $\to$ `double` (8 bytes)
    - `num taxa = 10; taxa = 3,50;` $\to$ promove automaticamente para `float` (4 bytes), evitando truncamento na geração C.
    - Se um `num` promovido a decimal for usado com `%`, o analisador semântico detecta e reporta erro antes da geração de código.

---

## 3. Roteiro para Apresentação (15–20 minutos)

1. **[0:00 – 2:30] Apresentação e Objetivos do Compilador**
   - Explicar o objetivo educacional da linguagem Laila: sintaxe em português, vírgula decimal (`8,50`), ponte passo a passo até C e escolha entre tipagem estática (`int`/`dec`) ou automática (`num`).
   - Mostrar um exemplo curto: [examples/aprovacao.laila](examples/aprovacao.laila) e [examples/numeros.laila](examples/numeros.laila).

2. **[2:30 – 7:00] Front-End (Vocabulário, Gramática/AST, Semântico)**
   - Rodar `cargo run -- --tokens examples/aprovacao.laila` e `cargo run -- --simbolos examples/escopos.laila` para demonstrar tokens, escopos aninhados (`B0..B4`) e *shadowing* estilo C.
   - Demonstrar detecção de erros:
     - `cargo run -- examples/erro_lexico.laila`
     - `cargo run -- examples/erro_sintatico.laila`
     - `cargo run -- examples/erro_semantico.laila`

3. **[7:00 – 14:00] Back-End (RI, Otimização, Geração C / Interpretação)**
   - **RI e Otimização:** rodar `cargo run -- --ri examples/otimizacao.laila`
     - Mostrar o Código de Três Endereços antes (16 instruções) e depois (2 instruções: `imprima 14, 3.5f` e `retorna`).
     - Explicar os passes no relatório: dobramento de constantes (`3 * 4 -> 12`, `2 + 12 -> 14`), simplificação algébrica (`base + 0`), propagação, eliminação do ramo morto (`senao imprima(999)`) e remoção da variável não utilizada (`inutil`).
   - **Geração C + Compilação Nativa vs Interpretação da RI:**
     - Rodar `cargo run -- --c examples/fatorial.laila` para mostrar o código C11 gerado (`int main(void)`, protótipos, tipos `int`/`float`/`double`, `printf`).
     - Comparar a saída da **VM da RI** (`cargo run -- --interpretar examples/fatorial.laila`) com a saída do **binário compilado pelo GCC** (`cargo run -- --executar examples/fatorial.laila`).
     - Rodar `cargo run -- --executar examples/numeros.laila` e `cargo run -- --compilar examples/aprovacao.laila -o aprovacao.exe`.

4. **[14:00 – 18:00] Estrutura do Código e Testes Automatizados**
   - Fazer um tour rápido pela organização modular em `src/`: `lexer.rs`, `parser.rs`, `ast.rs`, `symbol_table.rs`, `semantic.rs`, `number.rs`, `ir.rs`, `optimizer.rs`, `interpreter.rs`, `codegen_c.rs`.
   - Rodar `cargo test` ao vivo mostrando todos os testes automatizados (incluindo o teste de paridade entre VM da RI e GCC) passando.
