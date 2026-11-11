# RELATÓRIO TÉCNICO — TRABALHO 1: ANALISADOR / FRONT-END COMPLETO

- **Disciplina:** Compiladores
- **Autor:** Allan Gonçalves Santana
- **Linguagem:** Laila
- **Implementação:** Rust (`laila_lexer` / executável `laila`)

---

## 1. Descrição da Linguagem

### 1.1. Objetivo da Linguagem
A linguagem **Laila** é uma linguagem de programação imperativa e estaticamente tipada de escopo educacional, projetada para o ensino de lógica de programação e introdução gradual à sintaxe e semântica da linguagem C.

Seus principais pilares são:
- **Vocabulário em Português do Brasil:** palavras-chave intuitivas (`vazio`, `int`, `dec`, `carac`, `num`, `se`, `senao`, `enquanto`, `para`, `pare`, `continue`, `retorna`, `imprima`, `leia`, `verdadeiro`, `falso`).
- **Notação Numérica Brasileira:** uso da vírgula (`,`) como separador decimal (ex.: `8,50`, `3,14`) e suporte opcional ao ponto (`.`) como separador de milhar (ex.: `1.000.000`, `1.500,75`).
- **Estrutura Rigorosa de Blocos e Escopos do C:** delimitação explícita por chaves `{ ... }` e ponto e vírgula `;`, preparando o estudante para a transição direta à linguagem C.

### 1.2. Exemplos Curtos de Programas

**Exemplo 1 — Estrutura Condicional e Entrada/Saída (`examples/aprovacao.laila`):**
```c
vazio principal() {
    dec nota_final = 8,50;
    se (nota_final >= 7,00) {
        imprima("Aprovado");
    } senao {
        imprima("Reprovado");
    }
}
```

**Exemplo 2 — Funções, Parâmetros e Laço `para` (`examples/funcoes.laila`):**
```c
int soma(int a, int b) {
    retorna a + b;
}

vazio principal() {
    int total = 0;
    para (int i = 0; i < 10; i = i + 1) {
        se (i == 5) {
            continue;
        }
        total = soma(total, i);
    }
    imprima(total);
}
```

---

## 2. Análise Léxica (`src/lexer.rs` e `src/token.rs`)

O analisador léxico varre o código-fonte UTF-8 caractere a caractere, mantendo o rastreamento exato de linha, coluna e deslocamento em bytes (`Position` e `Span`), ignorando espaços em branco e comentários (`// ...` e `/* ... */`).

### 2.1. Tabela de Tokens Reconhecidos

| Categoria | Token (`TokenKind`) | Lexema / Exemplo | Padrão Reconhecido |
| :--- | :--- | :--- | :--- |
| **Palavras Reservadas (Tipos)** | `TK_TIPO_INT`, `TK_TIPO_DEC`, `TK_TIPO_CARAC`, `TK_TIPO_NUM`, `TK_TIPO_VAZIO` | `int`, `dec`, `carac`, `num`, `vazio` | `^(int\|dec\|carac\|num\|vazio)$` |
| **Controle Condicional** | `TK_SE`, `TK_SENAO` | `se`, `senao` | `^(se\|senao)$` |
| **Laços e Fluxo** | `TK_ENQUANTO`, `TK_PARA`, `TK_PARE`, `TK_CONTINUE`, `TK_RETORNA` | `enquanto`, `para`, `pare`, `continue`, `retorna` | Palavras-chave exatas |
| **Entrada e Saída** | `TK_IMPRIMA`, `TK_LEIA` | `imprima`, `leia` | `^(imprima\|leia)$` |
| **Literais Booleanos** | `TK_VERDADEIRO`, `TK_FALSO` | `verdadeiro`, `falso` | `^(verdadeiro\|falso)$` |
| **Identificadores** | `TK_ID` | `principal`, `nota_final`, `x` | `^[a-zA-Z_][a-zA-Z0-9_]*$` |
| **Literais Numéricos** | `TK_NUM_INT`, `TK_NUM_DEC` | `40`, `1.000`, `8,50`, `3,14` | Inteiros (com milhar opcional) e decimais com vírgula |
| **Literal de Texto** | `TK_TEXTO` | `"Aprovado"` | `^"[^"]*"$` |
| **Operador de Atribuição** | `TK_ATRIB` | `=` | `=` |
| **Operadores Aritméticos** | `TK_SOMA`, `TK_SUBTRACAO`, `TK_MULTIPLICA`, `TK_DIVISAO`, `TK_MODULO` | `+`, `-`, `*`, `/`, `%` | Operadores aritméticos padrão |
| **Operadores Relacionais** | `TK_IGUALDADE`, `TK_DIFERENTE`, `TK_MAIOR_QUE`, `TK_MENOR_QUE`, `TK_MAIOR_IGUAL`, `TK_MENOR_IGUAL` | `==`, `!=`, `>`, `<`, `>=`, `<=` | Operadores relacionais de 1 ou 2 caracteres |
| **Operadores Lógicos** | `TK_E`, `TK_OU`, `TK_NAO` | `&&`, `\|\|`, `!` | Conjunção, disjunção e negação |
| **Delimitadores** | `TK_PONTO_VIRG`, `TK_ABRE_CHAVES`, `TK_FECHA_CHAVES`, `TK_ABRE_PAR`, `TK_FECHA_PAR`, `TK_VIRGULA` | `;`, `{`, `}`, `(`, `)`, `,` | Pontuação estrutural |

---

## 3. Análise Sintática (`src/parser.rs`, `src/ast.rs` e `src/cst.rs`)

O analisador sintático utiliza a técnica **Descendente Recursiva preditiva ($LL$)** apoiada nas três operações fundamentais:
1. `lookahead()`: inspeciona o token atual sem consumi-lo.
2. `next()`: consome o token atual e avança o cursor.
3. `match_token(esperado)`: exige um `TokenKind` específico ou interrompe a derivação reportando um erro sintático detalhado.

O compilador constrói duas representações sintáticas complementares:
- **CST (Árvore de Derivação Concreta — `src/cst.rs`):** preserva todos os não-terminais da gramática (`<PROGRAMA>`, `<FUNCAO>`, `<DECL_VARIAVEL>`, `<CONDICIONAL>`), terminais (`=`, `;`, `(`, `)`, `{`, `}`) e produções vazias ($\varepsilon$).
- **AST (Árvore Sintática Abstrata — `src/ast.rs`):** estrutura enxuta e tipada consumida pelo Analisador Semântico.

### 3.1. Gramática Livre de Contexto Reconhecida
- `PROGRAMA` $\to$ `(FUNCAO | COMANDO)+ TK_EOF`
- `FUNCAO` $\to$ `TIPO_RETORNO TK_ID "(" PARAMETROS ")" BLOCO`
- `PARAMETROS` $\to$ `TIPO TK_ID ("," TIPO TK_ID)* | ε`
- `BLOCO` $\to$ `"{" COMANDO* "}"`
- `COMANDO` $\to$ `DECL_VARIAVEL | ATRIBUICAO | CONDICIONAL | REPETICAO_ENQUANTO | REPETICAO_PARA | ESCRITA | LEITURA | RETORNO | "pare" ";" | "continue" ";" | CHAMADA_FUNCAO ";" | BLOCO`
- `DECL_VARIAVEL` $\to$ `TIPO TK_ID ["=" EXPRESSAO_LOGICA] ";"`
- `ATRIBUICAO` $\to$ `TK_ID "=" EXPRESSAO_LOGICA ";"`
- `CONDICIONAL` $\to$ `"se" "(" EXPRESSAO_LOGICA ")" BLOCO ["senao" BLOCO]`
- `REPETICAO_ENQUANTO` $\to$ `"enquanto" "(" EXPRESSAO_LOGICA ")" BLOCO`
- `REPETICAO_PARA` $\to$ `"para" "(" [DECL_VARIAVEL_SIMPLES | ATRIB_SIMPLES] ";" [EXPRESSAO_LOGICA] ";" [ATRIB_SIMPLES] ")" BLOCO`
- **Hierarquia de Precedência de Expressões:**
  1. `EXPR_OU` (`||`) — menor precedência
  2. `EXPR_E` (`&&`)
  3. `EXPR_RELACIONAL` (`!`, `==`, `!=`, `<`, `>`, `<=`, `>=`)
  4. `EXPRESSAO_ARITMETICA` (`+`, `-`)
  5. `TERMO` (`*`, `/`, `%`)
  6. `FATOR` (unários `+`/`-`, literais, identificadores, chamadas `id(...)`, agrupamento `(...)`) — maior precedência

---

## 4. Tabela de Símbolos (`src/semantic.rs`)

### 4.1. Estrutura Utilizada
A Tabela de Símbolos (`SymbolTable`) é organizada como uma **árvore de escopos aninhados** ($B_0, B_1, B_2, \dots$), espelhando exatamente o modelo de blocos da linguagem C:
- Cada escopo (`Scope`) possui:
  - `id`: identificador único `ScopeId` (`B0` para o escopo global, `B1`, `B2`... para blocos filhos).
  - `parent`: ponteiro (`Option<ScopeId>`) para o escopo imediatamente externo.
  - `children`: lista dos sub-blocos aninhados (`Vec<ScopeId>`).
  - `symbols`: mapa `HashMap<String, Symbol>` (preservando a ordem de declaração em `symbol_order`).

### 4.2. Informações Armazenadas por Símbolo (`Symbol`)
Cada entrada na tabela registra:
1. **Nome (`name`):** lexema do identificador.
2. **Tipo Semântico (`ty`):** `int`, `dec`, `carac`, `num`, `bool`, `texto` ou `vazio`.
3. **Representação C e Tamanho (`c_type`, `size_bytes`):** tipo físico inferido para transpilação em C (`int (4B)`, `long long (8B)`, `float (4B)`, `double (8B)`, `char (1B)`, `void`).
4. **Categoria (`kind`):** `variável`, `parâmetro` ou `função` (incluindo a assinatura dos tipos dos parâmetros).
5. **Escopo e Posição de Declaração (`scope_id`, `decl_span`):** bloco $B_k$ e linha:coluna onde foi declarado.
6. **Lista de Referências (`references`):** todas as posições (linha:coluna) onde este símbolo específico foi utilizado.

### 4.3. Inserção, Consulta e Tratamento de Escopos
- **Entrada em Bloco (`enter_scope`):** ao entrar em uma função, condicional (`se`/`senao`), laço (`enquanto`/`para`) ou bloco aninhado `{ ... }`, um novo escopo filho $B_{\text{novo}}$ é criado apontando para o escopo atual como `parent`.
- **Inserção (`insert`):** consulta *apenas* a tabela local do bloco atual (`lookup_local`). Se o nome já existir no mesmo bloco $B_{\text{atual}}$, emite erro semântico de **redeclaração**. Se existir apenas em um bloco ancestral ($B_{\text{pai}}$), a inserção é permitida, realizando o **sombreamento (*shadowing*)** como em C.
- **Consulta e Resolução de Referências (`resolve_and_record`):** ao encontrar o uso de um identificador no bloco atual, a busca inicia em $B_{\text{atual}}$ e sobe recursivamente pela cadeia `parent` ($B_{\text{atual}} \to B_{\text{pai}} \to \dots \to B_0$). A primeira declaração encontrada responde a qual variável aquele uso se refere; a referência é gravada tanto no símbolo quanto na lista global `ResolvedReference`.
- **Saída de Bloco (`exit_scope`):** ao fechar `}`, o escopo ativo retorna para `parent`, desativando a visibilidade das variáveis locais do bloco encerrado.

---

## 5. Semântica da Linguagem (`src/semantic.rs`)

As regras semânticas implementadas em `SemanticAnalyzer` cobrem todas as construções da linguagem:

1. **Declarações e Redeclarações:**
   - Variáveis e parâmetros não podem ter o tipo `vazio` (`VoidVariable`).
   - Não é permitido declarar dois identificadores de mesmo nome no mesmo escopo $B_k$ (`Redeclaration`). Parâmetros formais pertencem ao escopo principal do corpo da função (impedindo que uma variável local de topo na função tenha o mesmo nome de um parâmetro).
2. **Escopo e Uso de Identificadores:**
   - Todo identificador usado em expressões, atribuições (`x = ...`), leitura (`leia(x)`) ou chamadas (`f(...)`) deve ter sido previamente declarado em um escopo visível na cadeia ancestral até `B0` (`UndeclaredIdentifier`).
   - Variáveis declaradas dentro de um bloco interno `{ ... }` não são visíveis após o fechamento do bloco.
3. **Sistema de Tipos e Atribuições:**
   - **Tipagem estrita entre `int` e `dec`:** uma variável `int` só aceita expressões do tipo `int` (ou `num`), e uma variável `dec` só aceita expressões do tipo `dec` (ou `num`). Atribuir `dec` a `int` (ex.: `int x = 8,50;`) ou `int` a `dec` (ex.: `dec y = 10;`) gera erro semântico `TypeMismatch`.
   - O tipo curinga `num` é compatível tanto com `int` quanto com `dec`, tendo sua representação em bytes (`4B` ou `8B`) calculada por `NumCompiler`.
4. **Expressões e Operadores:**
   - **Aritméticos (`+`, `-`, `*`, `/`):** exigem operandos numéricos compatíveis (`int` com `int` resulta em `int`; `dec` com `dec` resulta em `dec`).
   - **Módulo (`%`):** assim como em C, exige exclusivamente operandos inteiros (`int` ou `num`), rejeitando `dec`.
   - **Relacionais (`<`, `>`, `<=`, `>=`):** exigem operandos numéricos compatíveis entre si e produzem resultado lógico `bool`.
   - **Igualdade (`==`, `!=`):** exigem operandos de tipos compatíveis entre si e produzem `bool`.
   - **Lógicos (`&&`, `||`, `!`):** exigem exclusivamente operandos do tipo `bool` e produzem `bool`.
5. **Condições de Controle (`se`, `enquanto`, `para`):**
   - A expressão condicional deve obrigatoriamente resultar no tipo `bool` (`NonBooleanCondition`). Expressões puramente numéricas como `se (idade + 10)` são rejeitadas.
6. **Funções e Retorno (`retorna`):**
   - Na chamada de função, verifica-se se o identificador pertence à categoria `função` (`InvalidSymbolKind`), se a quantidade de argumentos coincide com o número de parâmetros (`ArityMismatch`) e se o tipo de cada argumento é compatível com o parâmetro formal (`TypeMismatch`).
   - Funções `vazio` não podem retornar valores (`retorna expr;`), e funções tipadas (`int`, `dec`, `carac`, `num`) devem retornar uma expressão compatível com seu tipo de retorno (`ReturnMismatch`).
7. **Comandos Específicos (`pare`, `continue`, `imprima`, `leia`):**
   - `pare;` e `continue;` são validados quanto ao contexto de laço (`loop_depth > 0`), gerando `LoopControlOutsideLoop` se usados fora de `enquanto` ou `para`.
   - `leia(id);` exige que `id` seja uma variável declarada (não aceita nome de função).
   - `imprima(...)` rejeita expressões do tipo `vazio`.

---

## 6. Tratamento de Erros

O compilador classifica e reporta erros nas três fases do Front-End com indicação precisa de arquivo, linha, coluna e mensagem explicativa:

1. **Erros Léxicos (`LexError` em `src/lexer.rs`):**
   - `UnexpectedCharacter('.')`: uso de ponto decimal estilo inglês (ex.: `3.14`) fora do padrão de milhar.
   - `UnexpectedCharacter(c)`: símbolos ilegais fora do alfabeto da linguagem (ex.: `@`, `$`).
   - `UnterminatedText`: literal de texto `"..."` sem aspas de fechamento.
   - `MultipleDecimalCommas`: número com mais de uma vírgula decimal (ex.: `1,2,3`).
   - `InvalidNumericSuffix`: número colado a letras/identificador (ex.: `10abc`).
2. **Erros Sintáticos (`ParseError` em `src/parser.rs`):**
   - Ausência de delimitadores obrigatórios (`;`, `(`, `)`, `{`, `}`).
   - Ordem inválida de tokens (ex.: `= taxa 40;`).
   - Palavras-chave ou tipos inexistentes (ex.: `float x = 3,14;`).
   - Expressões malformadas (operador sem operando, parênteses desbalanceados).
3. **Erros Semânticos (`SemanticError` em `src/semantic.rs`):**
   - `UndeclaredIdentifier`: uso de variável ou função não declarada.
   - `Redeclaration`: redeclaração de variável/parâmetro/função no mesmo escopo $B_k$, informando a linha da declaração original.
   - `TypeMismatch`: incompatibilidade de tipos em inicialização, atribuição ou passagem de argumentos.
   - `InvalidOperation`: operadores aplicados a tipos incompatíveis (ex.: `int + dec`, `dec % dec`, `!int`).
   - `NonBooleanCondition`: condição não booleana em `se`, `enquanto` ou `para`.
   - `ArityMismatch`: número incorreto de argumentos em chamada de função.
   - `ReturnMismatch`: retorno incompatível com a assinatura da função.
   - `LoopControlOutsideLoop`: `pare` ou `continue` fora de laço.

---

## 7. Testes e Saídas Reais do Compilador

### 7.1. Programa Válido 1 — Escopos Aninhados e Tabela de Símbolos (`examples/escopos.laila`)
Comando executado:
```powershell
cargo run -- --simbolos examples/escopos.laila
```
Saída real:
```text
========================================================================
                    TABELA DE SÍMBOLOS (POR ESCOPO)                     
========================================================================

B0: escopo global (raiz)
  ┌──────────────────┬──────────┬──────────────────┬──────────────┬────────────┬──────────────────────┐
  │ Identificador    │ Tipo     │ Tipo C (Bytes)   │ Categoria    │ Declaração │ Referências (usos)   │
  ├──────────────────┼──────────┼──────────────────┼──────────────┼────────────┼──────────────────────┤
  │ principal        │ vazio    │ void             │ função       │ 1:1        │ -                    │
  └──────────────────┴──────────┴──────────────────┴──────────────┴────────────┴──────────────────────┘
└── B1: função 'principal' (pai: B0)
      ┌──────────────────┬──────────┬──────────────────┬──────────────┬────────────┬──────────────────────┐
      │ Identificador    │ Tipo     │ Tipo C (Bytes)   │ Categoria    │ Declaração │ Referências (usos)   │
      ├──────────────────┼──────────┼──────────────────┼──────────────┼────────────┼──────────────────────┤
      │ a                │ int      │ int (4B)         │ variável     │ 2:5        │ 12:21, 14:17, 16:13  │
      │ b                │ int      │ int (4B)         │ variável     │ 3:5        │ 16:16                │
      └──────────────────┴──────────┴──────────────────┴──────────────┴────────────┴──────────────────────┘
    └── B2: bloco aninhado (linha 4) (pai: B1)
          ┌──────────────────┬──────────┬──────────────────┬──────────────┬────────────┬──────────────────────┐
          │ Identificador    │ Tipo     │ Tipo C (Bytes)   │ Categoria    │ Declaração │ Referências (usos)   │
          ├──────────────────┼──────────┼──────────────────┼──────────────┼────────────┼──────────────────────┤
          │ b                │ int      │ int (4B)         │ variável     │ 5:9        │ 8:24, 14:20          │
          └──────────────────┴──────────┴──────────────────┴──────────────┴────────────┴──────────────────────┘
        ├── B3: bloco aninhado (linha 6) (pai: B2)
        │     ┌──────────────────┬──────────┬──────────────────┬──────────────┬────────────┬──────────────────────┐
        │     │ Identificador    │ Tipo     │ Tipo C (Bytes)   │ Categoria    │ Declaração │ Referências (usos)   │
        │     ├──────────────────┼──────────┼──────────────────┼──────────────┼────────────┼──────────────────────┤
        │     │ a                │ int      │ int (4B)         │ variável     │ 7:13       │ 8:21                 │
        │     └──────────────────┴──────────┴──────────────────┴──────────────┴────────────┴──────────────────────┘
        └── B4: bloco aninhado (linha 10) (pai: B2)
              ┌──────────────────┬──────────┬──────────────────┬──────────────┬────────────┬──────────────────────┐
              │ Identificador    │ Tipo     │ Tipo C (Bytes)   │ Categoria    │ Declaração │ Referências (usos)   │
              ├──────────────────┼──────────┼──────────────────┼──────────────┼────────────┼──────────────────────┤
              │ b                │ int      │ int (4B)         │ variável     │ 11:13      │ 12:24                │
              └──────────────────┴──────────┴──────────────────┴──────────────┴────────────┴──────────────────────┘

========================================================================
            RESOLUÇÃO DE REFERÊNCIAS (USO -> DECLARAÇÃO)                
========================================================================
  [linha 8:21] 'a' (usado em B3) -> refere-se a variável 'a' : int declarada em B3 (linha 7:13)
  [linha 8:24] 'b' (usado em B3) -> refere-se a variável 'b' : int declarada em B2 (linha 5:9)
  [linha 12:21] 'a' (usado em B4) -> refere-se a variável 'a' : int declarada em B1 (linha 2:5)
  [linha 12:24] 'b' (usado em B4) -> refere-se a variável 'b' : int declarada em B4 (linha 11:13)
  [linha 14:17] 'a' (usado em B2) -> refere-se a variável 'a' : int declarada em B1 (linha 2:5)
  [linha 14:20] 'b' (usado em B2) -> refere-se a variável 'b' : int declarada em B2 (linha 5:9)
  [linha 16:13] 'a' (usado em B1) -> refere-se a variável 'a' : int declarada em B1 (linha 2:5)
  [linha 16:16] 'b' (usado em B1) -> refere-se a variável 'b' : int declarada em B1 (linha 3:5)
```

### 7.2. Programa Válido 2 — Validação Integrada (`examples/aprovacao.laila` e `examples/funcoes.laila`)
Comando executado:
```powershell
cargo run -- examples/aprovacao.laila
cargo run -- examples/funcoes.laila
```
Saída real:
```text
[1/3] Análise Léxica:    OK (33 tokens reconhecidos)
[2/3] Análise Sintática: OK (1 elemento(s) na AST)
[3/3] Análise Semântica: OK (4 escopo(s) na Tabela de Símbolos, 1 referência(s) resolvida(s))
=> Resultado (examples/aprovacao.laila): O código é LÉXICA, SINTÁTICA e SEMANTICAMENTE VÁLIDO.

[1/3] Análise Léxica:    OK (69 tokens reconhecidos)
[2/3] Análise Sintática: OK (2 elemento(s) na AST)
[3/3] Análise Semântica: OK (5 escopo(s) na Tabela de Símbolos, 11 referência(s) resolvida(s))
=> Resultado (examples/funcoes.laila): O código é LÉXICA, SINTÁTICA e SEMANTICAMENTE VÁLIDO.
```

### 7.3. Programa Inválido 1 — Erro Léxico (`examples/erro_lexico.laila`)
```powershell
cargo run -- examples/erro_lexico.laila
```
Saída real:
```text
[ERRO LÉXICO] examples/erro_lexico.laila:3:15: ponto inválido: use vírgula em números decimais (ex.: 3,14)
```

### 7.4. Programa Inválido 2 — Erro Sintático (`examples/erro_sintatico.laila`)
```powershell
cargo run -- examples/erro_sintatico.laila
```
Saída real:
```text
[ERRO SINTÁTICO] examples/erro_sintatico.laila:4:8: erro sintático: esperado '(' (TK_ABRE_PAR), mas foi encontrado 'nota_final' (TK_ID)
```

### 7.5. Programa Inválido 3 — Múltiplos Erros Semânticos (`examples/erro_semantico.laila`)
```powershell
cargo run -- examples/erro_semantico.laila
```
Saída real:
```text
[ERRO SEMÂNTICO] examples/erro_semantico.laila:7:17: erro semântico: tipos incompatíveis na declaração de 'idade': variável é do tipo 'int', mas recebeu expressão do tipo 'dec'
[ERRO SEMÂNTICO] examples/erro_semantico.laila:10:5: erro semântico: redeclaração de 'idade' no mesmo escopo (B2): já declarado anteriormente na linha 7:5
[ERRO SEMÂNTICO] examples/erro_semantico.laila:13:5: erro semântico: variável 'taxa' não foi declarada neste escopo (B2) nem em escopos externos
[ERRO SEMÂNTICO] examples/erro_semantico.laila:16:9: erro semântico: a condição do comando 'se' deve ser lógica ('bool'), mas foi encontrado 'int'
[ERRO SEMÂNTICO] examples/erro_semantico.laila:21:21: erro semântico: a função 'soma' espera 2 argumento(s), mas recebeu 1
```

---

## 8. Limitações Atuais

1. **Vetores/Arrays (`[` e `]`):** embora os tokens `TK_ABRE_COLCH` e `TK_FECHA_COLCH` já sejam reconhecidos pelo analisador léxico para evolução futura, a indexação e declaração de arrays ainda não fazem parte da gramática atual.
2. **Análise de Fluxo de Controle Completa (Reachability):** a validação de `retorna` verifica a compatibilidade de tipos em cada instrução `retorna`, mas ainda não exige prova estática de que todos os caminhos possíveis de `se`/`senao` terminam com `retorna`.
3. **Recuperação de Erros Sintáticos (*Panic-Mode Recovery*):** o parser interrompe no primeiro erro sintático encontrado (enquanto o analisador semântico já acumula e reporta todos os erros semânticos do programa de uma só vez).

---

## 9. Análise Crítica

- **Partes mais desafiadoras:**
  1. **Desambiguação léxica de números no padrão brasileiro:** diferenciar pontos separadores de milhar (`1.000.000`) de pontos decimais inválidos (`3.14`) exigiu lookahead de dígitos no lexer (`thousand_dot`).
  2. **Modelagem fiel de escopos aninhados e *shadowing* estilo C:** estruturar a Tabela de Símbolos como uma árvore persistente de escopos (`B0`, `B1`, `B2`...) de modo que os escopos não fossem descartados ao fechar `}`, permitindo inspecionar e imprimir toda a hierarquia de blocos e todas as referências resolvidas após o término da análise semântica.
- **Decisões de projeto:**
  - Manter duas árvores sintáticas (CST para visualização didática de todas as regras e terminais da gramática, e AST para uma análise semântica limpa e modular).
  - Acumular múltiplos erros semânticos em uma única passagem pela AST para oferecer feedback completo ao programador.
- **Melhorias para a próxima versão:**
  - Implementar a geração completa do código C equivalente (Back-End / Transpilador) e invocação automática do GCC/Clang.
  - Adicionar suporte completo a arrays e estruturas (`registro`).

---

## 10. Pergunta Final

> **“Se o programa já foi considerado válido, como transformamos esse programa em algo executável?”**

Uma vez que o Front-End validou léxica, sintática e semanticamente o programa e construiu a **AST** enriquecida pela **Tabela de Símbolos**, o processo para transformá-lo em um programa executável passa pelas etapas do **Middle-End** e **Back-End** do compilador:

1. **Geração de Representação Intermediária (IR) ou Transpilação para C:**
   No caso da linguagem Laila, cujo projeto prevê a atuação como ponte para o C, percorremos a AST validada consultando a Tabela de Símbolos para emitir código C equivalente (por exemplo, convertendo `vazio principal()` em `int main(void)`, `dec` em `float`/`double`, `8,50` em `8.5f`, e `imprima`/`leia` em chamadas `printf`/`scanf` com os especificadores `%d`, `%f`, `%s` determinados pelo tipo de cada símbolo na tabela). Em um compilador tradicional nativo, a AST seria traduzida para uma Representação Intermediária (como Código de Três Endereços — TAC ou LLVM IR).
2. **Otimização de Código:**
   Sobre a representação intermediária (ou pelo próprio compilador C alvo), aplicam-se otimizações independentes e dependentes de máquina, como propagação de constantes (*constant folding*), eliminação de código morto (*dead code elimination*) e alocação eficiente de registradores.
3. **Geração de Código de Máquina / Assembly e Montagem:**
   As instruções são traduzidas para a linguagem de montagem (Assembly, ex.: x86_64) da arquitetura alvo e convertidas pelo montador (*assembler*) em código objeto binário (`.o` / `.obj`).
4. **Ligação (*Linking*):**
   Por fim, o *linker* (ligador) combina o código objeto gerado com as bibliotecas de tempo de execução (como a `libc` para `printf` e `scanf`), resolvendo os endereços de memória finais e produzindo o arquivo binário executável (`.exe` no Windows ou ELF no Linux) pronto para ser carregado pelo Sistema Operacional.

---

## 11. Roteiro Sugerido para o Vídeo de Apresentação (Até 10 Minutos)

1. **0:00 – 1:30 | Apresentação da Linguagem Laila:**
   - Mostrar objetivo educacional, sintaxe em português, números com vírgula decimal (`8,50`) e estrutura de blocos inspirada em C.
2. **1:30 – 4:00 | Execução de Programas Válidos (Pipeline Completo):**
   - Rodar `cargo run -- --completo examples/aprovacao.laila` mostrando os Tokens, a Árvore de Derivação (CST), a Tabela de Símbolos e o Veredito Final.
   - Rodar `cargo run -- examples/funcoes.laila` mostrando funções, parâmetros e laço `para`.
3. **4:00 – 6:30 | Funcionamento da Tabela de Símbolos e Escopos (`B0..B4`):**
   - Abrir `examples/escopos.laila` lado a lado com o slide e rodar `cargo run -- --simbolos examples/escopos.laila`, destacando como cada uso de `a` e `b` é vinculado ao bloco correto (`B1`, `B2`, `B3` ou `B4`).
4. **6:30 – 9:00 | Demonstração da Detecção de Erros (Léxico, Sintático e Semântico):**
   - Rodar `cargo run -- examples/erro_lexico.laila` (erro de `3.14` com ponto).
   - Rodar `cargo run -- examples/erro_sintatico.laila` (falta de parênteses no `se`).
   - Rodar `cargo run -- examples/erro_semantico.laila` (mostrando os 5 erros semânticos detectados de uma só vez: tipo incompatível, redeclaração no mesmo escopo, variável não declarada, condição não booleana e aridade de função).
   - Rodar `cargo test` mostrando os 38 testes automatizados passando.
5. **9:00 – 10:00 | Decisões de Projeto e Conclusão:**
   - Destacar a estrutura da árvore de escopos em Rust e o papel da Tabela de Símbolos para a geração de código.
