use crate::ir::{
    IrBinOp, IrConst, IrFunction, IrInstruction, IrOperand, IrProgram, IrType, IrUnOp, IrVar,
};
use crate::symbol_table::ScopeId;
use std::collections::HashMap;
use std::fmt;
use std::io::{self, BufRead, BufReader, Write};

/// Valor armazenado em tempo de execução pela Máquina Virtual da RI.
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeValue {
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Char(char),
    Texto(String),
    Bool(bool),
    Vazio,
}

impl RuntimeValue {
    pub fn from_const(c: &IrConst) -> Self {
        match c {
            IrConst::Int(v, true) => Self::Long(*v),
            IrConst::Int(v, false) => Self::Int(*v as i32),
            IrConst::Float(v, true) => Self::Float(*v as f32),
            IrConst::Float(v, false) => Self::Double(*v),
            IrConst::Char(c) => Self::Char(*c),
            IrConst::Texto(s) => Self::Texto(s.clone()),
            IrConst::Bool(b) => Self::Bool(*b),
        }
    }

    pub fn coerce_to(self, target: IrType) -> Self {
        match (self, target) {
            (Self::Int(v), IrType::Long) => Self::Long(v as i64),
            (Self::Int(v), IrType::Float) => Self::Float(v as f32),
            (Self::Int(v), IrType::Double) => Self::Double(v as f64),
            (Self::Long(v), IrType::Int) => Self::Int(v as i32),
            (Self::Long(v), IrType::Float) => Self::Float(v as f32),
            (Self::Long(v), IrType::Double) => Self::Double(v as f64),
            (Self::Float(v), IrType::Double) => Self::Double(v as f64),
            (Self::Double(v), IrType::Float) => Self::Float(v as f32),
            (Self::Char(c), IrType::Texto) => Self::Texto(c.to_string()),
            (other, _) => other,
        }
    }

    pub fn default_for(ty: IrType) -> Self {
        match ty {
            IrType::Int => Self::Int(0),
            IrType::Long => Self::Long(0),
            IrType::Float => Self::Float(0.0),
            IrType::Double => Self::Double(0.0),
            IrType::Char => Self::Char('\0'),
            IrType::Texto => Self::Texto(String::new()),
            IrType::Bool => Self::Bool(false),
            IrType::Vazio => Self::Vazio,
        }
    }

    fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Int(v) => Some(*v as f64),
            Self::Long(v) => Some(*v as f64),
            Self::Float(v) => Some(*v as f64),
            Self::Double(v) => Some(*v),
            _ => None,
        }
    }

    fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(v) => Some(*v as i64),
            Self::Long(v) => Some(*v),
            _ => None,
        }
    }
}

impl fmt::Display for RuntimeValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(v) => write!(f, "{v}"),
            Self::Long(v) => write!(f, "{v}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::Double(v) => write!(f, "{v}"),
            Self::Char(c) => write!(f, "{c}"),
            Self::Texto(s) => f.write_str(s),
            Self::Bool(true) => f.write_str("verdadeiro"),
            Self::Bool(false) => f.write_str("falso"),
            Self::Vazio => f.write_str("vazio"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeError {
    pub message: String,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "erro de execução (RI): {}", self.message)
    }
}

impl std::error::Error for RuntimeError {}

#[derive(Debug, Clone, Default)]
pub struct ExecutionStats {
    pub instructions_executed: usize,
    pub return_value: Option<RuntimeValue>,
}

/// Executa um `IrProgram` usando stdin e stdout padrão.
pub fn interpret_ir(program: &IrProgram) -> Result<ExecutionStats, RuntimeError> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();
    interpret_ir_with_io(program, &mut reader, &mut writer)
}

/// Executa um `IrProgram` usando fluxos de entrada e saída customizados (ideal para testes).
pub fn interpret_ir_with_io<R: BufRead, W: Write>(
    program: &IrProgram,
    reader: &mut R,
    writer: &mut W,
) -> Result<ExecutionStats, RuntimeError> {
    let functions: HashMap<&str, &IrFunction> = program
        .functions
        .iter()
        .map(|f| (f.name.as_str(), f))
        .collect();

    let mut vm = Vm {
        functions,
        globals: HashMap::new(),
        reader,
        writer,
        instructions_executed: 0,
        input_tokens: Vec::new(),
    };

    let mut top_locals = HashMap::new();
    let mut ret = None;
    if !program.globals_init.is_empty() {
        ret = Some(vm.exec_instructions(&program.globals_init, &mut top_locals)?);
    }

    if vm.functions.contains_key("principal") {
        let main_ret = vm.call_function("principal", Vec::new())?;
        ret = Some(main_ret);
    }

    Ok(ExecutionStats {
        instructions_executed: vm.instructions_executed,
        return_value: ret,
    })
}

type Env = HashMap<(Option<ScopeId>, String), RuntimeValue>;

struct Vm<'a, R: BufRead, W: Write> {
    functions: HashMap<&'a str, &'a IrFunction>,
    globals: Env,
    reader: &'a mut R,
    writer: &'a mut W,
    instructions_executed: usize,
    input_tokens: Vec<String>,
}

impl<'a, R: BufRead, W: Write> Vm<'a, R, W> {
    fn call_function(
        &mut self,
        name: &str,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, RuntimeError> {
        let func = *self.functions.get(name).ok_or_else(|| RuntimeError {
            message: format!("função '{name}' não encontrada na RI"),
        })?;

        let mut locals: Env = HashMap::new();
        for local in &func.locals {
            if local.scope != Some(ScopeId(0)) {
                locals.insert(
                    (local.scope, local.name.clone()),
                    RuntimeValue::default_for(local.ty),
                );
            }
        }
        for (param, val) in func.params.iter().zip(args) {
            locals.insert(
                (param.scope, param.name.clone()),
                val.coerce_to(param.ty),
            );
        }

        self.exec_instructions(&func.instructions, &mut locals)
    }

    fn exec_instructions(
        &mut self,
        instrs: &[IrInstruction],
        locals: &mut Env,
    ) -> Result<RuntimeValue, RuntimeError> {
        let mut labels = HashMap::new();
        for (idx, instr) in instrs.iter().enumerate() {
            if let IrInstruction::Label(name) = instr {
                labels.insert(name.as_str(), idx);
            }
        }

        let mut pc = 0usize;
        let mut param_stack: Vec<RuntimeValue> = Vec::new();

        while pc < instrs.len() {
            self.instructions_executed += 1;
            if self.instructions_executed > 10_000_000 {
                return Err(RuntimeError {
                    message: "limite máximo de instruções atingido (possível laço infinito)".into(),
                });
            }

            match &instrs[pc] {
                IrInstruction::Label(_) => {
                    pc += 1;
                }
                IrInstruction::Assign { dst, src } => {
                    let val = self.eval_operand(src, locals)?.coerce_to(dst.ty);
                    self.write_var(dst, val, locals);
                    pc += 1;
                }
                IrInstruction::Unary { dst, op, operand } => {
                    let val = self.eval_operand(operand, locals)?;
                    let res = eval_unary(*op, val)?.coerce_to(dst.ty);
                    self.write_var(dst, res, locals);
                    pc += 1;
                }
                IrInstruction::Binary {
                    dst,
                    op,
                    left,
                    right,
                } => {
                    let l = self.eval_operand(left, locals)?;
                    let r = self.eval_operand(right, locals)?;
                    let res = eval_binary(*op, l, r)?.coerce_to(dst.ty);
                    self.write_var(dst, res, locals);
                    pc += 1;
                }
                IrInstruction::Jump(target) => {
                    pc = *labels.get(target.as_str()).ok_or_else(|| RuntimeError {
                        message: format!("rótulo '{target}' não encontrado"),
                    })?;
                }
                IrInstruction::JumpIfFalse { cond, target } => {
                    let b = match self.eval_operand(cond, locals)? {
                        RuntimeValue::Bool(b) => b,
                        other => {
                            return Err(RuntimeError {
                                message: format!("condição inválida em desvio: {other}"),
                            })
                        }
                    };
                    if !b {
                        pc = *labels.get(target.as_str()).ok_or_else(|| RuntimeError {
                            message: format!("rótulo '{target}' não encontrado"),
                        })?;
                    } else {
                        pc += 1;
                    }
                }
                IrInstruction::JumpIfTrue { cond, target } => {
                    let b = match self.eval_operand(cond, locals)? {
                        RuntimeValue::Bool(b) => b,
                        other => {
                            return Err(RuntimeError {
                                message: format!("condição inválida em desvio: {other}"),
                            })
                        }
                    };
                    if b {
                        pc = *labels.get(target.as_str()).ok_or_else(|| RuntimeError {
                            message: format!("rótulo '{target}' não encontrado"),
                        })?;
                    } else {
                        pc += 1;
                    }
                }
                IrInstruction::Param(op) => {
                    let val = self.eval_operand(op, locals)?;
                    param_stack.push(val);
                    pc += 1;
                }
                IrInstruction::Call {
                    dst,
                    callee,
                    arg_count,
                } => {
                    if param_stack.len() < *arg_count {
                        return Err(RuntimeError {
                            message: format!("argumentos insuficientes na chamada a '{callee}'"),
                        });
                    }
                    let split_at = param_stack.len() - *arg_count;
                    let args = param_stack.split_off(split_at);
                    let ret = self.call_function(callee, args)?;
                    if let Some(d) = dst {
                        self.write_var(d, ret.coerce_to(d.ty), locals);
                    }
                    pc += 1;
                }
                IrInstruction::Return(opt_op) => {
                    return match opt_op {
                        Some(op) => self.eval_operand(op, locals),
                        None => Ok(RuntimeValue::Vazio),
                    };
                }
                IrInstruction::Print(args) => {
                    for (idx, arg) in args.iter().enumerate() {
                        if idx > 0 {
                            write!(self.writer, " ").map_err(|e| RuntimeError {
                                message: format!("falha ao escrever saída: {e}"),
                            })?;
                        }
                        let val = self.eval_operand(arg, locals)?;
                        write!(self.writer, "{val}").map_err(|e| RuntimeError {
                            message: format!("falha ao escrever saída: {e}"),
                        })?;
                    }
                    writeln!(self.writer).map_err(|e| RuntimeError {
                        message: format!("falha ao escrever quebra de linha: {e}"),
                    })?;
                    pc += 1;
                }
                IrInstruction::Read(var) => {
                    let val = self.read_value(var.ty)?;
                    self.write_var(var, val, locals);
                    pc += 1;
                }
            }
        }

        Ok(RuntimeValue::Vazio)
    }

    fn eval_operand(
        &self,
        op: &IrOperand,
        locals: &Env,
    ) -> Result<RuntimeValue, RuntimeError> {
        match op {
            IrOperand::Const(c) => Ok(RuntimeValue::from_const(c)),
            IrOperand::Var(v) => {
                let key = (v.scope, v.name.clone());
                if v.scope == Some(ScopeId(0)) {
                    self.globals.get(&key).cloned()
                } else {
                    locals.get(&key).cloned()
                }
                .ok_or_else(|| RuntimeError {
                    message: format!("variável '{}' não inicializada em tempo de execução", v),
                })
            }
        }
    }

    fn write_var(&mut self, var: &IrVar, val: RuntimeValue, locals: &mut Env) {
        let key = (var.scope, var.name.clone());
        if var.scope == Some(ScopeId(0)) {
            self.globals.insert(key, val);
        } else {
            locals.insert(key, val);
        }
    }

    fn next_input_token(&mut self) -> Result<String, RuntimeError> {
        while self.input_tokens.is_empty() {
            let mut line = String::new();
            let bytes = self.reader.read_line(&mut line).map_err(|e| RuntimeError {
                message: format!("erro ao ler entrada ('leia'): {e}"),
            })?;
            if bytes == 0 {
                return Err(RuntimeError {
                    message: "fim inesperado da entrada padrão ao executar 'leia'".into(),
                });
            }
            let mut tokens: Vec<String> = line.split_whitespace().map(|s| s.to_string()).collect();
            tokens.reverse();
            self.input_tokens = tokens;
        }
        Ok(self.input_tokens.pop().unwrap())
    }

    fn read_value(&mut self, ty: IrType) -> Result<RuntimeValue, RuntimeError> {
        let raw = self.next_input_token()?;
        match ty {
            IrType::Int => raw
                .replace('.', "")
                .parse::<i32>()
                .map(RuntimeValue::Int)
                .map_err(|_| RuntimeError {
                    message: format!("entrada inválida para inteiro ('int'): '{raw}'"),
                }),
            IrType::Long => raw
                .replace('.', "")
                .parse::<i64>()
                .map(RuntimeValue::Long)
                .map_err(|_| RuntimeError {
                    message: format!("entrada inválida para inteiro longo: '{raw}'"),
                }),
            IrType::Float => raw
                .replace(',', ".")
                .parse::<f32>()
                .map(RuntimeValue::Float)
                .map_err(|_| RuntimeError {
                    message: format!("entrada inválida para decimal: '{raw}'"),
                }),
            IrType::Double => raw
                .replace(',', ".")
                .parse::<f64>()
                .map(RuntimeValue::Double)
                .map_err(|_| RuntimeError {
                    message: format!("entrada inválida para decimal ('dec'/'num'): '{raw}'"),
                }),
            IrType::Char => raw
                .chars()
                .next()
                .map(RuntimeValue::Char)
                .ok_or_else(|| RuntimeError {
                    message: "entrada vazia para 'carac'".into(),
                }),
            IrType::Texto => Ok(RuntimeValue::Texto(raw)),
            IrType::Bool => match raw.as_str() {
                "verdadeiro" | "true" | "1" => Ok(RuntimeValue::Bool(true)),
                "falso" | "false" | "0" => Ok(RuntimeValue::Bool(false)),
                _ => Err(RuntimeError {
                    message: format!("entrada inválida para booleano: '{raw}'"),
                }),
            },
            IrType::Vazio => Ok(RuntimeValue::Vazio),
        }
    }
}

fn eval_unary(op: IrUnOp, val: RuntimeValue) -> Result<RuntimeValue, RuntimeError> {
    match (op, val) {
        (IrUnOp::Neg, RuntimeValue::Int(v)) => Ok(RuntimeValue::Int(v.wrapping_neg())),
        (IrUnOp::Neg, RuntimeValue::Long(v)) => Ok(RuntimeValue::Long(v.wrapping_neg())),
        (IrUnOp::Neg, RuntimeValue::Float(v)) => Ok(RuntimeValue::Float(-v)),
        (IrUnOp::Neg, RuntimeValue::Double(v)) => Ok(RuntimeValue::Double(-v)),
        (IrUnOp::Not, RuntimeValue::Bool(b)) => Ok(RuntimeValue::Bool(!b)),
        _ => Err(RuntimeError {
            message: "operação unária inválida na RI".into(),
        }),
    }
}

fn eval_binary(
    op: IrBinOp,
    left: RuntimeValue,
    right: RuntimeValue,
) -> Result<RuntimeValue, RuntimeError> {
    match (left, right) {
        (RuntimeValue::Bool(a), RuntimeValue::Bool(b)) => match op {
            IrBinOp::And => Ok(RuntimeValue::Bool(a && b)),
            IrBinOp::Or => Ok(RuntimeValue::Bool(a || b)),
            IrBinOp::Eq => Ok(RuntimeValue::Bool(a == b)),
            IrBinOp::Ne => Ok(RuntimeValue::Bool(a != b)),
            _ => Err(RuntimeError {
                message: format!("operador '{op}' inválido para booleanos"),
            }),
        },
        (RuntimeValue::Char(a), RuntimeValue::Char(b)) => match op {
            IrBinOp::Eq => Ok(RuntimeValue::Bool(a == b)),
            IrBinOp::Ne => Ok(RuntimeValue::Bool(a != b)),
            _ => Err(RuntimeError {
                message: format!("operador '{op}' inválido para carac"),
            }),
        },
        (RuntimeValue::Texto(a), RuntimeValue::Texto(b)) => match op {
            IrBinOp::Eq => Ok(RuntimeValue::Bool(a == b)),
            IrBinOp::Ne => Ok(RuntimeValue::Bool(a != b)),
            _ => Err(RuntimeError {
                message: format!("operador '{op}' inválido para texto"),
            }),
        },
        (l @ (RuntimeValue::Int(_) | RuntimeValue::Long(_)), r @ (RuntimeValue::Int(_) | RuntimeValue::Long(_))) => {
            let is_long = matches!(l, RuntimeValue::Long(_)) || matches!(r, RuntimeValue::Long(_));
            let a = l.as_i64().unwrap();
            let b = r.as_i64().unwrap();
            let wrap = |v: i64| {
                if is_long {
                    RuntimeValue::Long(v)
                } else {
                    RuntimeValue::Int(v as i32)
                }
            };
            match op {
                IrBinOp::Add => Ok(wrap(a.wrapping_add(b))),
                IrBinOp::Sub => Ok(wrap(a.wrapping_sub(b))),
                IrBinOp::Mul => Ok(wrap(a.wrapping_mul(b))),
                IrBinOp::Div => {
                    if b == 0 {
                        return Err(RuntimeError {
                            message: "divisão por zero em tempo de execução".into(),
                        });
                    }
                    Ok(wrap(a / b))
                }
                IrBinOp::Mod => {
                    if b == 0 {
                        return Err(RuntimeError {
                            message: "módulo por zero em tempo de execução".into(),
                        });
                    }
                    Ok(wrap(a % b))
                }
                IrBinOp::Eq => Ok(RuntimeValue::Bool(a == b)),
                IrBinOp::Ne => Ok(RuntimeValue::Bool(a != b)),
                IrBinOp::Lt => Ok(RuntimeValue::Bool(a < b)),
                IrBinOp::Gt => Ok(RuntimeValue::Bool(a > b)),
                IrBinOp::Le => Ok(RuntimeValue::Bool(a <= b)),
                IrBinOp::Ge => Ok(RuntimeValue::Bool(a >= b)),
                _ => Err(RuntimeError {
                    message: format!("operador '{op}' inválido para inteiros"),
                }),
            }
        }
        (l, r) => {
            let (Some(a), Some(b)) = (l.as_f64(), r.as_f64()) else {
                return Err(RuntimeError {
                    message: format!("tipos incompatíveis na operação '{op}'"),
                });
            };
            let is_f32 = matches!(l, RuntimeValue::Float(_))
                && !matches!(r, RuntimeValue::Double(_))
                || matches!(r, RuntimeValue::Float(_)) && !matches!(l, RuntimeValue::Double(_));
            let wrap_f = |v: f64| {
                if is_f32 {
                    RuntimeValue::Float(v as f32)
                } else {
                    RuntimeValue::Double(v)
                }
            };
            match op {
                IrBinOp::Add => Ok(wrap_f(a + b)),
                IrBinOp::Sub => Ok(wrap_f(a - b)),
                IrBinOp::Mul => Ok(wrap_f(a * b)),
                IrBinOp::Div => {
                    if b == 0.0 {
                        return Err(RuntimeError {
                            message: "divisão por zero em tempo de execução".into(),
                        });
                    }
                    Ok(wrap_f(a / b))
                }
                IrBinOp::Eq => Ok(RuntimeValue::Bool(a == b)),
                IrBinOp::Ne => Ok(RuntimeValue::Bool(a != b)),
                IrBinOp::Lt => Ok(RuntimeValue::Bool(a < b)),
                IrBinOp::Gt => Ok(RuntimeValue::Bool(a > b)),
                IrBinOp::Le => Ok(RuntimeValue::Bool(a <= b)),
                IrBinOp::Ge => Ok(RuntimeValue::Bool(a >= b)),
                _ => Err(RuntimeError {
                    message: format!("operador '{op}' inválido para decimais"),
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::build_ir;
    use crate::optimizer::optimize_ir;
    use crate::{analyze_semantics, parse, tokenize};

    fn run_str(code: &str, input: &str) -> String {
        let tokens = tokenize(code).unwrap();
        let program = parse(&tokens).unwrap();
        let table = analyze_semantics(&program).unwrap();
        let ir = build_ir(&program, &table);
        let (opt, _) = optimize_ir(&ir);
        let mut reader = BufReader::new(input.as_bytes());
        let mut out = Vec::new();
        interpret_ir_with_io(&opt, &mut reader, &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn test_interpreta_escopos_aninhados() {
        let code = include_str!("../examples/escopos.laila");
        let output = run_str(code, "");
        assert_eq!(output.lines().collect::<Vec<_>>(), vec!["3 2", "1 4", "1 2", "1 1"]);
    }

    #[test]
    fn test_interpreta_funcoes_e_laco_para_com_continue() {
        let code = include_str!("../examples/funcoes.laila");
        let output = run_str(code, "");
        assert_eq!(output.trim(), "40");
    }

    #[test]
    fn test_interpreta_leia_e_num_automatico() {
        let code = r#"
vazio principal() {
    num x = 10;
    x = 2,50;
    num y;
    leia(y);
    imprima(x + y);
}
"#;
        let output = run_str(code, "7,50\n");
        assert_eq!(output.trim(), "10");
    }
}
