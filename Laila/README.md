# Analisador Sintático da Linguagem Laila

Implementação do Analisador Sintático Descendente Recursivo para a linguagem **Laila**, desenvolvida para a disciplina de **Compiladores**.

---

## 1. Operações Básicas do Analisador Sintático

O analisador segue estritamente a metodologia clássica de compiladores apresentada em aula:

1. **`lookahead()`**:
   - Consulta o token atual no fluxo de tokens sem consumi-lo.
2. **`next()`**:
   - Consome o token atual e avança o cursor para o próximo.
3. **`match_token(expected)` / `match(expected)`**:
   - Exige que o token atual seja do tipo esperado.
   - **Se for o esperado:** consome o token e retorna seu valor (equivalente a `next()`).
   - **Se não for:** sinaliza um erro sintático preciso com linha, coluna, token esperado e token encontrado.

---

## 2. Erros e Omissões da Gramática Original Corrigidos

Ao analisar o PDF da Gramática Representativa em conjunto com o vocabulário oficial da linguagem, foram identificados e corrigidos os seguintes problemas estruturais:

1. **Regra `FATOR` ausente na especificação:**
   - No documento original, `FATOR` era utilizado na regra `TERMO → FATOR (( "*" | "/" | "%" ) FATOR)*` e no passo a passo de derivação, mas **nunca havia sido definido na seção de regras sintáticas**.
   - Além disso, a linguagem precisa aceitar variáveis em expressões aritméticas (ex.: `i = i + 1;` presente nos próprios exemplos do PDF).
   - **Correção implementada:** A regra `FATOR` foi devidamente introduzida suportando:
     - Identificadores (`IDENTIFICADOR`), incluindo chamadas de função;
     - Literais numéricos inteiros e decimais (`LITERAL_INT`, `LITERAL_DEC`);
     - Literais de texto e booleanos (`LITERAL_TEXTO`, `LITERAL_BOOL`);
     - Expressões agrupadas entre parênteses (`"(" EXPRESSAO ")"`);
     - Operadores unários (`+` e `-`).

2. **`LISTA_PARAMETROS` limitada a apenas um parâmetro:**
   - A gramática especificava `LISTA_PARAMETROS → TIPO IDENTIFICADOR`, impossibilitando a declaração de funções com 2 ou mais parâmetros (ex.: `soma(int a, int b)`), apesar do token `TK_VIRGULA` (`","`) já existir no vocabulário do compilador.
   - **Correção implementada:** `LISTA_PARAMETROS → TIPO IDENTIFICADOR ("," TIPO IDENTIFICADOR)*`.

3. **Omissão dos comandos de controle de fluxo de repetição (`pare` e `continue`):**
   - As palavras-chave `pare` (equivalente ao `break`) e `continue` foram definidas no vocabulário oficial, porém foram omitidas da produção `COMANDO`.
   - **Correção implementada:** Inclusão de `"pare" ";"` e `"continue" ";"` na lista de comandos válidos.

4. **Operadores lógicos ausentes em `EXPRESSAO_LOGICA`:**
   - O vocabulário definia os tokens `TK_E` (`&&`), `TK_OU` (`||`) e `TK_NAO` (`!`), mas a regra da gramática só previa `EXPRESSAO OP_RELACIONAL EXPRESSAO | LITERAL_BOOL`. Isso impedia expressões lógicas compostas como `se (x > 0 && taxa == 10)`.
   - **Correção implementada:** Criação da precedência lógica clássica:
     - `EXPR_OU → EXPR_E ("||" EXPR_E)*`
     - `EXPR_E → EXPR_RELACIONAL ("&&" EXPR_RELACIONAL)*`
     - `EXPR_RELACIONAL → "!" EXPR_RELACIONAL | EXPRESSAO (OP_RELACIONAL EXPRESSAO)?`

5. **Suporte a múltiplas funções no programa:**
   - A regra `PROGRAMA → FUNCAO` permitia apenas uma única função no arquivo.
   - **Correção implementada:** `PROGRAMA → (FUNCAO | COMANDO)+ TK_EOF`, permitindo programas modulares com funções auxiliares e a função `principal()`.

6. **Chamada de função como comando:**
   - Permitida a instrução `IDENTIFICADOR "(" [ARGUMENTOS] ")" ";"`, possibilitando invocar procedimentos e funções que realizam efeitos colaterais.

---

## 3. Árvore de Derivação Completa (CST / Parse Tree) vs. AST

A linguagem oferece duas visualizações da estrutura sintática:
1. **Árvore de Derivação Completa (CST / Parse Tree - `--arvore` ou `--cst`):**
   - Preserva **rigorosamente todos os elementos da gramática**, incluindo pontuações (`=`, `;`, `,`), delimitadores (`(`, `)`, `{`, `}`) e palavras-chave.
   - Ideal para apresentação na disciplina de Compiladores e verificação formal do processo de derivação gramatical.
2. **Árvore Sintática Abstrata (AST - `--ast`):**
   - Representação enxuta voltada para geração de código e análise semântica.

---

## 4. O Tipo Numérico Unificado `num` e Notação Numérica Brasileira

Para simplificar a tipagem e otimizar memória RAM, a linguagem Laila implementa o tipo unificado `num`:
- **Economia Dinâmica de Memória RAM:** O compilador infere o tipo C de menor consumo de memória necessário:
  - **Inteiros sem vírgula (`int` - 4 bytes):** Ex.: `idade = 21;`, `populacao = 1.000.000;`.
  - **Inteiros longos sem vírgula (`long long` - 8 bytes):** Ex.: inteiros maiores que $2^{31}-1$.
  - **Decimais simples com vírgula (`float` - 4 bytes):** Ex.: `nota = 8,50;`, economizando 4 bytes de RAM comparado ao `double`.
  - **Decimais de dupla precisão (`double` - 8 bytes):** Ex.: `pi = 3,141592653589793;`.
- **Notação em Português do Brasil (PT-BR):**
  - **Vírgula (`,`) para decimais:** Ex.: `8,50`, `3,14`.
  - **Ponto (`.`) para separador de milhares/unidades:** Ex.: `1.000`, `1.000.000`, `1.500,75`.
  - O uso inadvertido de ponto como separador decimal (ex.: `3.14`) é rejeitado com mensagem de erro educativa orientando o uso da vírgula.

---

## 5. Como Executar

### Compilação e Testes
```bash
cargo build
cargo test
```

### Execução de Arquivos
```bash
# Validar sintaxe
cargo run -- examples/aprovacao.laila

# Imprimir árvore de derivação completa (CST - inclui '=', ';', chaves, parênteses)
cargo run -- --arvore examples/aprovacao.laila
# ou
cargo run -- --cst examples/aprovacao.laila

# Imprimir árvore sintática abstrata (AST)
cargo run -- --ast examples/aprovacao.laila

# Inspecionar tipos C, bytes de RAM e literais de num
cargo run -- --lower-num examples/numeros.laila

# Apenas análise léxica (tokens)
cargo run -- --tokens examples/aprovacao.laila
```
