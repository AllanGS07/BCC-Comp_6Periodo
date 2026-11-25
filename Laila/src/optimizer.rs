use crate::ir::{IrBinOp, IrConst, IrInstruction, IrOperand, IrProgram, IrType, IrUnOp, IrVar};
use crate::symbol_table::ScopeId;
use std::collections::{HashMap, HashSet};

/// Relatório detalhado das otimizações aplicadas sobre a Representação Intermediária.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptimizationReport {
    pub constants_folded: usize,
    pub algebraic_simplifications: usize,
    pub propagations: usize,
    pub dead_instructions_removed: usize,
    pub branches_simplified: usize,
    pub passes_run: usize,
    pub log: Vec<String>,
}

impl OptimizationReport {
    pub fn total_optimizations(&self) -> usize {
        self.constants_folded
            + self.algebraic_simplifications
            + self.propagations
            + self.dead_instructions_removed
            + self.branches_simplified
    }

    pub fn format_summary(&self, before_instrs: usize, after_instrs: usize) -> String {
        let mut out = String::new();
        out.push_str("========================================================================\n");
        out.push_str("                   RELATÓRIO DE OTIMIZAÇÃO DA RI                        \n");
        out.push_str("========================================================================\n");
        out.push_str(&format!(
            "  Instruções antes  : {before_instrs}\n  Instruções depois : {after_instrs}\n  Passes executados : {}\n",
            self.passes_run
        ));
        out.push_str(&format!(
            "  - Dobramento de constantes (Constant Folding) : {}\n",
            self.constants_folded
        ));
        out.push_str(&format!(
            "  - Simplificações algébricas                   : {}\n",
            self.algebraic_simplifications
        ));
        out.push_str(&format!(
            "  - Propagação de constantes e cópias           : {}\n",
            self.propagations
        ));
        out.push_str(&format!(
            "  - Simplificações de desvio (Branch Folding)   : {}\n",
            self.branches_simplified
        ));
        out.push_str(&format!(
            "  - Código morto removido (Dead Code Elim.)     : {}\n",
            self.dead_instructions_removed
        ));
        if self.log.is_empty() {
            out.push_str("\n  (nenhuma otimização aplicável; código já mínimo)\n");
        } else {
            out.push_str("\n  Transformações registradas:\n");
            for entry in self.log.iter().take(25) {
                out.push_str(&format!("    • {entry}\n"));
            }
            if self.log.len() > 25 {
                out.push_str(&format!(
                    "    ... e mais {} transformação(ões)\n",
                    self.log.len() - 25
                ));
            }
        }
        out
    }
}

/// Executa os passes de otimização iterativamente até atingir ponto fixo.
pub fn optimize_ir(program: &IrProgram) -> (IrProgram, OptimizationReport) {
    let mut optimized = program.clone();
    let mut report = OptimizationReport::default();

    let mut global_changed = true;
    while global_changed && report.passes_run < 16 {
        global_changed = false;
        report.passes_run += 1;

        if optimize_block(&mut optimized.globals_init, &mut report) {
            global_changed = true;
        }
        for func in &mut optimized.functions {
            if optimize_block(&mut func.instructions, &mut report) {
                global_changed = true;
            }
        }
    }

    (optimized, report)
}

fn optimize_block(instrs: &mut Vec<IrInstruction>, report: &mut OptimizationReport) -> bool {
    let mut changed = false;
    if propagate_local(instrs, report) {
        changed = true;
    }
    if fold_constants_and_branches(instrs, report) {
        changed = true;
    }
    if simplify_algebraic(instrs, report) {
        changed = true;
    }
    if eliminate_dead_code(instrs, report) {
        changed = true;
    }
    changed
}

/// Pass 1: Propagação local de constantes e cópias dentro de cada bloco básico.
fn propagate_local(instrs: &mut [IrInstruction], report: &mut OptimizationReport) -> bool {
    let mut changed = false;
    let mut const_map: HashMap<IrVar, IrConst> = HashMap::new();
    let mut copy_map: HashMap<IrVar, IrVar> = HashMap::new();

    for instr in instrs.iter_mut() {
        match instr {
            IrInstruction::Label(_)
            | IrInstruction::Jump(_)
            | IrInstruction::JumpIfFalse { .. }
            | IrInstruction::JumpIfTrue { .. }
            | IrInstruction::Return(_) => {
                // Antes de limpar o bloco básico, propaga nos operandos de desvio/retorno
                match instr {
                    IrInstruction::JumpIfFalse { cond, .. }
                    | IrInstruction::JumpIfTrue { cond, .. } => {
                        if rewrite_operand(cond, &const_map, &copy_map, report) {
                            changed = true;
                        }
                    }
                    IrInstruction::Return(Some(val)) => {
                        if rewrite_operand(val, &const_map, &copy_map, report) {
                            changed = true;
                        }
                    }
                    _ => {}
                }
                const_map.clear();
                copy_map.clear();
            }
            IrInstruction::Assign { dst, src } => {
                if rewrite_operand(src, &const_map, &copy_map, report) {
                    changed = true;
                }
                invalidate_var(dst, &mut const_map, &mut copy_map);
                match src {
                    IrOperand::Const(c) => {
                        const_map.insert(dst.clone(), c.coerce_to(dst.ty));
                    }
                    IrOperand::Var(v) if v != dst && v.ty == dst.ty => {
                        copy_map.insert(dst.clone(), v.clone());
                    }
                    _ => {}
                }
            }
            IrInstruction::Binary {
                dst, left, right, ..
            } => {
                if rewrite_operand(left, &const_map, &copy_map, report) {
                    changed = true;
                }
                if rewrite_operand(right, &const_map, &copy_map, report) {
                    changed = true;
                }
                invalidate_var(dst, &mut const_map, &mut copy_map);
            }
            IrInstruction::Unary { dst, operand, .. } => {
                if rewrite_operand(operand, &const_map, &copy_map, report) {
                    changed = true;
                }
                invalidate_var(dst, &mut const_map, &mut copy_map);
            }
            IrInstruction::Param(op) => {
                if rewrite_operand(op, &const_map, &copy_map, report) {
                    changed = true;
                }
            }
            IrInstruction::Print(args) => {
                for arg in args.iter_mut() {
                    if rewrite_operand(arg, &const_map, &copy_map, report) {
                        changed = true;
                    }
                }
            }
            IrInstruction::Call { dst, .. } => {
                // Chamada pode alterar variáveis globais (B0)
                const_map.retain(|k, _| k.scope != Some(ScopeId(0)));
                copy_map.retain(|k, v| {
                    k.scope != Some(ScopeId(0)) && v.scope != Some(ScopeId(0))
                });
                if let Some(d) = dst {
                    invalidate_var(d, &mut const_map, &mut copy_map);
                }
            }
            IrInstruction::Read(var) => {
                invalidate_var(var, &mut const_map, &mut copy_map);
            }
        }
    }

    changed
}

fn rewrite_operand(
    op: &mut IrOperand,
    const_map: &HashMap<IrVar, IrConst>,
    copy_map: &HashMap<IrVar, IrVar>,
    report: &mut OptimizationReport,
) -> bool {
    if let IrOperand::Var(v) = op {
        if let Some(c) = const_map.get(v) {
            let coerced = c.coerce_to(v.ty);
            report.propagations += 1;
            report.log.push(format!("Propagação de constante: {v} -> {coerced}"));
            *op = IrOperand::Const(coerced);
            return true;
        }
        if let Some(orig) = copy_map.get(v)
            && orig.ty == v.ty {
                report.propagations += 1;
                report.log.push(format!("Propagação de cópia: {v} -> {orig}"));
                *op = IrOperand::Var(orig.clone());
                return true;
            }
    }
    false
}

fn invalidate_var(
    var: &IrVar,
    const_map: &mut HashMap<IrVar, IrConst>,
    copy_map: &mut HashMap<IrVar, IrVar>,
) {
    const_map.remove(var);
    copy_map.remove(var);
    copy_map.retain(|_, target| target != var);
}

/// Pass 2: Dobramento de constantes e simplificação de desvios condicionais constantes.
fn fold_constants_and_branches(
    instrs: &mut Vec<IrInstruction>,
    report: &mut OptimizationReport,
) -> bool {
    let mut changed = false;
    let mut out = Vec::with_capacity(instrs.len());

    for instr in instrs.drain(..) {
        match instr {
            IrInstruction::Unary {
                dst,
                op,
                operand: IrOperand::Const(c),
            } => {
                if let Some(res) = eval_unary_const(op, &c) {
                    let coerced = res.coerce_to(dst.ty);
                    report.constants_folded += 1;
                    report.log.push(format!(
                        "Dobramento de constante: {op}{c} -> {coerced}"
                    ));
                    out.push(IrInstruction::Assign {
                        dst,
                        src: IrOperand::Const(coerced),
                    });
                    changed = true;
                } else {
                    out.push(IrInstruction::Unary {
                        dst,
                        op,
                        operand: IrOperand::Const(c),
                    });
                }
            }
            IrInstruction::Binary {
                dst,
                op,
                left: IrOperand::Const(l),
                right: IrOperand::Const(r),
            } => {
                if let Some(res) = eval_binary_const(op, &l, &r) {
                    let coerced = res.coerce_to(dst.ty);
                    report.constants_folded += 1;
                    report.log.push(format!(
                        "Dobramento de constante: {l} {op} {r} -> {coerced}"
                    ));
                    out.push(IrInstruction::Assign {
                        dst,
                        src: IrOperand::Const(coerced),
                    });
                    changed = true;
                } else {
                    out.push(IrInstruction::Binary {
                        dst,
                        op,
                        left: IrOperand::Const(l),
                        right: IrOperand::Const(r),
                    });
                }
            }
            IrInstruction::JumpIfFalse {
                cond: IrOperand::Const(IrConst::Bool(b)),
                target,
            } => {
                report.branches_simplified += 1;
                changed = true;
                if b {
                    report
                        .log
                        .push(format!("Desvio condicional eliminado (condição sempre verdadeira, salto para {target} removido)"));
                } else {
                    report.log.push(format!(
                        "Desvio condicional convertido em salto incondicional: ir_para {target}"
                    ));
                    out.push(IrInstruction::Jump(target));
                }
            }
            IrInstruction::JumpIfTrue {
                cond: IrOperand::Const(IrConst::Bool(b)),
                target,
            } => {
                report.branches_simplified += 1;
                changed = true;
                if b {
                    report.log.push(format!(
                        "Desvio condicional convertido em salto incondicional: ir_para {target}"
                    ));
                    out.push(IrInstruction::Jump(target));
                } else {
                    report.log.push(format!(
                        "Desvio condicional eliminado (condição sempre falsa, salto para {target} removido)"
                    ));
                }
            }
            other => out.push(other),
        }
    }

    *instrs = out;
    changed
}

/// Pass 3: Simplificações algébricas (identidades neutras, elemento absorvente, autoatribuição).
fn simplify_algebraic(instrs: &mut Vec<IrInstruction>, report: &mut OptimizationReport) -> bool {
    let mut changed = false;
    let mut out = Vec::with_capacity(instrs.len());

    for instr in instrs.drain(..) {
        match instr {
            IrInstruction::Assign { dst, src: IrOperand::Var(v) } if dst == v => {
                report.algebraic_simplifications += 1;
                report
                    .log
                    .push(format!("Autoatribuição removida: {dst} = {v}"));
                changed = true;
            }
            IrInstruction::Binary {
                dst,
                op,
                left,
                right,
            } => {
                if let Some(simplified) = simplify_binary_algebraic(&dst, op, &left, &right) {
                    report.algebraic_simplifications += 1;
                    report.log.push(format!(
                        "Simplificação algébrica: {left} {op} {right} -> {simplified}"
                    ));
                    out.push(IrInstruction::Assign {
                        dst,
                        src: simplified,
                    });
                    changed = true;
                } else {
                    out.push(IrInstruction::Binary {
                        dst,
                        op,
                        left,
                        right,
                    });
                }
            }
            other => out.push(other),
        }
    }

    *instrs = out;
    changed
}

fn is_zero(op: &IrOperand) -> bool {
    match op {
        IrOperand::Const(IrConst::Int(0, _)) => true,
        IrOperand::Const(IrConst::Float(v, _)) => *v == 0.0,
        _ => false,
    }
}

fn is_one(op: &IrOperand) -> bool {
    match op {
        IrOperand::Const(IrConst::Int(1, _)) => true,
        IrOperand::Const(IrConst::Float(v, _)) => *v == 1.0,
        _ => false,
    }
}

fn simplify_binary_algebraic(
    dst: &IrVar,
    op: IrBinOp,
    left: &IrOperand,
    right: &IrOperand,
) -> Option<IrOperand> {
    match op {
        IrBinOp::Add => {
            if is_zero(right) && left.ty() == dst.ty {
                Some(left.clone())
            } else if is_zero(left) && right.ty() == dst.ty {
                Some(right.clone())
            } else {
                None
            }
        }
        IrBinOp::Sub => {
            if is_zero(right) && left.ty() == dst.ty {
                Some(left.clone())
            } else if matches!(dst.ty, IrType::Int | IrType::Long) && left == right {
                Some(IrOperand::Const(IrConst::Int(0, dst.ty == IrType::Long)))
            } else {
                None
            }
        }
        IrBinOp::Mul => {
            if is_one(right) && left.ty() == dst.ty {
                Some(left.clone())
            } else if is_one(left) && right.ty() == dst.ty {
                Some(right.clone())
            } else if matches!(dst.ty, IrType::Int | IrType::Long)
                && (is_zero(left) || is_zero(right))
            {
                Some(IrOperand::Const(IrConst::Int(0, dst.ty == IrType::Long)))
            } else {
                None
            }
        }
        IrBinOp::Div => {
            if is_one(right) && left.ty() == dst.ty {
                Some(left.clone())
            } else {
                None
            }
        }
        IrBinOp::And => match (left, right) {
            (x, IrOperand::Const(IrConst::Bool(true)))
            | (IrOperand::Const(IrConst::Bool(true)), x)
                if x.ty() == dst.ty =>
            {
                Some(x.clone())
            }
            (_, IrOperand::Const(IrConst::Bool(false)))
            | (IrOperand::Const(IrConst::Bool(false)), _) => {
                Some(IrOperand::Const(IrConst::Bool(false)))
            }
            _ => None,
        },
        IrBinOp::Or => match (left, right) {
            (x, IrOperand::Const(IrConst::Bool(false)))
            | (IrOperand::Const(IrConst::Bool(false)), x)
                if x.ty() == dst.ty =>
            {
                Some(x.clone())
            }
            (_, IrOperand::Const(IrConst::Bool(true)))
            | (IrOperand::Const(IrConst::Bool(true)), _) => {
                Some(IrOperand::Const(IrConst::Bool(true)))
            }
            _ => None,
        },
        _ => None,
    }
}

/// Pass 4: Eliminação de código morto (blocos inalcançáveis, saltos redundantes, rótulos e escritas mortas).
fn eliminate_dead_code(instrs: &mut Vec<IrInstruction>, report: &mut OptimizationReport) -> bool {
    let mut changed = false;

    // 4.1 Remove instruções inalcançáveis após `Jump` ou `Return` até o próximo `Label`
    let mut reachable = Vec::with_capacity(instrs.len());
    let mut in_dead_zone = false;
    for instr in instrs.drain(..) {
        if let IrInstruction::Label(_) = &instr {
            in_dead_zone = false;
            reachable.push(instr);
            continue;
        }
        if in_dead_zone {
            report.dead_instructions_removed += 1;
            report
                .log
                .push(format!("Instrução inalcançável removida: {}", instr.to_string().trim()));
            changed = true;
            continue;
        }
        let terminates_block = matches!(instr, IrInstruction::Jump(_) | IrInstruction::Return(_));
        reachable.push(instr);
        if terminates_block {
            in_dead_zone = true;
        }
    }
    *instrs = reachable;

    // 4.2 Remove `Jump(L)` imediatamente seguido de `Label(L)`
    let mut fallthrough_cleaned = Vec::with_capacity(instrs.len());
    for i in 0..instrs.len() {
        if let IrInstruction::Jump(ref target) = instrs[i]
            && let Some(IrInstruction::Label(next_label)) = instrs.get(i + 1)
                && target == next_label {
                    report.branches_simplified += 1;
                    report.log.push(format!(
                        "Salto redundante removido: ir_para {target} antes de {target}:"
                    ));
                    changed = true;
                    continue;
                }
        fallthrough_cleaned.push(instrs[i].clone());
    }
    *instrs = fallthrough_cleaned;

    // 4.3 Remove rótulos nunca referenciados por nenhum salto
    let mut referenced_labels = HashSet::new();
    for instr in instrs.iter() {
        match instr {
            IrInstruction::Jump(t)
            | IrInstruction::JumpIfFalse { target: t, .. }
            | IrInstruction::JumpIfTrue { target: t, .. } => {
                referenced_labels.insert(t.clone());
            }
            _ => {}
        }
    }
    let before_labels = instrs.len();
    instrs.retain(|instr| {
        if let IrInstruction::Label(lbl) = instr
            && !referenced_labels.contains(lbl) {
                report.dead_instructions_removed += 1;
                report.log.push(format!("Rótulo não utilizado removido: {lbl}"));
                return false;
            }
        true
    });
    if instrs.len() != before_labels {
        changed = true;
    }

    // 4.4 Remove atribuições puras para temporários ou variáveis locais que nunca são lidas
    let mut read_vars = HashSet::new();
    for instr in instrs.iter() {
        collect_reads(instr, &mut read_vars);
    }

    let mut live_instrs = Vec::with_capacity(instrs.len());
    for instr in instrs.drain(..) {
        let dead_dst = match &instr {
            IrInstruction::Assign { dst, .. }
            | IrInstruction::Binary { dst, .. }
            | IrInstruction::Unary { dst, .. }
                if dst.scope != Some(ScopeId(0)) && !read_vars.contains(dst) =>
            {
                Some(dst.clone())
            }
            _ => None,
        };
        if let Some(dst) = dead_dst {
            report.dead_instructions_removed += 1;
            report.log.push(format!(
                "Atribuição morta eliminada (valor de {dst} nunca é lido)"
            ));
            changed = true;
        } else {
            live_instrs.push(instr);
        }
    }
    *instrs = live_instrs;

    changed
}

fn collect_reads(instr: &IrInstruction, reads: &mut HashSet<IrVar>) {
    let mut record = |op: &IrOperand| {
        if let IrOperand::Var(v) = op {
            reads.insert(v.clone());
        }
    };
    match instr {
        IrInstruction::Assign { src, .. } => record(src),
        IrInstruction::Binary { left, right, .. } => {
            record(left);
            record(right);
        }
        IrInstruction::Unary { operand, .. } => record(operand),
        IrInstruction::JumpIfFalse { cond, .. } | IrInstruction::JumpIfTrue { cond, .. } => {
            record(cond)
        }
        IrInstruction::Param(op) => record(op),
        IrInstruction::Return(Some(op)) => record(op),
        IrInstruction::Print(args) => {
            for a in args {
                record(a);
            }
        }
        IrInstruction::Label(_)
        | IrInstruction::Jump(_)
        | IrInstruction::Call { .. }
        | IrInstruction::Return(None)
        | IrInstruction::Read(_) => {}
    }
}

fn eval_unary_const(op: IrUnOp, c: &IrConst) -> Option<IrConst> {
    match (op, c) {
        (IrUnOp::Neg, IrConst::Int(v, is_long)) => {
            v.checked_neg().map(|neg| IrConst::Int(neg, *is_long))
        }
        (IrUnOp::Neg, IrConst::Float(v, is_f32)) => Some(IrConst::Float(-*v, *is_f32)),
        (IrUnOp::Not, IrConst::Bool(b)) => Some(IrConst::Bool(!*b)),
        _ => None,
    }
}

fn eval_binary_const(op: IrBinOp, left: &IrConst, right: &IrConst) -> Option<IrConst> {
    match (left, right) {
        (IrConst::Bool(a), IrConst::Bool(b)) => match op {
            IrBinOp::And => Some(IrConst::Bool(*a && *b)),
            IrBinOp::Or => Some(IrConst::Bool(*a || *b)),
            IrBinOp::Eq => Some(IrConst::Bool(*a == *b)),
            IrBinOp::Ne => Some(IrConst::Bool(*a != *b)),
            _ => None,
        },
        (IrConst::Char(a), IrConst::Char(b)) => match op {
            IrBinOp::Eq => Some(IrConst::Bool(*a == *b)),
            IrBinOp::Ne => Some(IrConst::Bool(*a != *b)),
            _ => None,
        },
        (IrConst::Texto(a), IrConst::Texto(b)) => match op {
            IrBinOp::Eq => Some(IrConst::Bool(a == b)),
            IrBinOp::Ne => Some(IrConst::Bool(a != b)),
            _ => None,
        },
        (IrConst::Int(a, l1), IrConst::Int(b, l2)) => {
            let is_long = *l1 || *l2;
            match op {
                IrBinOp::Add => a.checked_add(*b).map(|v| IrConst::Int(v, is_long)),
                IrBinOp::Sub => a.checked_sub(*b).map(|v| IrConst::Int(v, is_long)),
                IrBinOp::Mul => a.checked_mul(*b).map(|v| IrConst::Int(v, is_long)),
                IrBinOp::Div if *b != 0 => a.checked_div(*b).map(|v| IrConst::Int(v, is_long)),
                IrBinOp::Mod if *b != 0 => a.checked_rem(*b).map(|v| IrConst::Int(v, is_long)),
                IrBinOp::Eq => Some(IrConst::Bool(a == b)),
                IrBinOp::Ne => Some(IrConst::Bool(a != b)),
                IrBinOp::Lt => Some(IrConst::Bool(a < b)),
                IrBinOp::Gt => Some(IrConst::Bool(a > b)),
                IrBinOp::Le => Some(IrConst::Bool(a <= b)),
                IrBinOp::Ge => Some(IrConst::Bool(a >= b)),
                _ => None,
            }
        }
        _ if left.ty().is_numeric() && right.ty().is_numeric() => {
            let a = match left {
                IrConst::Int(v, _) => *v as f64,
                IrConst::Float(v, _) => *v,
                _ => return None,
            };
            let b = match right {
                IrConst::Int(v, _) => *v as f64,
                IrConst::Float(v, _) => *v,
                _ => return None,
            };
            let is_f32 = matches!(left, IrConst::Float(_, true))
                && !matches!(right, IrConst::Float(_, false))
                || matches!(right, IrConst::Float(_, true))
                    && !matches!(left, IrConst::Float(_, false));
            match op {
                IrBinOp::Add => Some(IrConst::Float(a + b, is_f32)),
                IrBinOp::Sub => Some(IrConst::Float(a - b, is_f32)),
                IrBinOp::Mul => Some(IrConst::Float(a * b, is_f32)),
                IrBinOp::Div if b != 0.0 => Some(IrConst::Float(a / b, is_f32)),
                IrBinOp::Eq => Some(IrConst::Bool(a == b)),
                IrBinOp::Ne => Some(IrConst::Bool(a != b)),
                IrBinOp::Lt => Some(IrConst::Bool(a < b)),
                IrBinOp::Gt => Some(IrConst::Bool(a > b)),
                IrBinOp::Le => Some(IrConst::Bool(a <= b)),
                IrBinOp::Ge => Some(IrConst::Bool(a >= b)),
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::build_ir;
    use crate::{analyze_semantics, parse, tokenize};

    #[test]
    fn test_otimizador_dobra_constantes_e_elimina_codigo_morto() {
        let code = r#"
vazio principal() {
    int a = 2 + 3 * 4;
    int b = a + 0;
    se (verdadeiro) {
        imprima(b);
    } senao {
        imprima(999);
    }
}
"#;
        let tokens = tokenize(code).unwrap();
        let program = parse(&tokens).unwrap();
        let table = analyze_semantics(&program).unwrap();
        let ir = build_ir(&program, &table);
        let (opt, report) = optimize_ir(&ir);

        assert!(opt.instruction_count() < ir.instruction_count());
        assert!(report.total_optimizations() > 0);
        let opt_str = opt.format_ir();
        assert!(opt_str.contains("imprima 14"));
        assert!(!opt_str.contains("999"));
    }
}
