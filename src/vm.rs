use crate::{
    middle_ir::NativeFn,
    targets::vm::{ApartFnLabel, Instructive, ValueIndex, prelude::*},
};

use std::{
    collections::HashSet,
    io::{BufRead, Write},
    mem,
};

struct CallFrame {
    fp: usize,
    from: u32,
    to: u16,
}

struct Vm {
    registers: Vec<Value>,
    arena: Vec<GarbageCollectedValue>,
    allocated: usize,
    next_gc: usize,
    call_frames: Vec<CallFrame>,
    function_labels: Vec<ApartFnLabel>,
    literals: Vec<ValueIndex>,
    ip: usize,
    sp: Option<usize>,
    fp: usize,
}

const DEFAULT_NEXT_GC: usize = 1_000_000;

impl Instructive for Vm {
    fn ip_mut(&mut self) -> &mut usize {
        &mut self.ip
    }

    fn literals_mut(&mut self) -> &mut Vec<ValueIndex> {
        &mut self.literals
    }

    fn arena_mut(&mut self) -> &mut Vec<GarbageCollectedValue> {
        &mut self.arena
    }

    fn ip(&self) -> usize {
        self.ip
    }

    fn literals(&self) -> &[ValueIndex] {
        self.literals.as_slice()
    }

    fn arena(&self) -> &[GarbageCollectedValue] {
        self.arena.as_slice()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Interrupt {
    Panic,
}

/// # Errors
/// Returns an `Interrupt` under the following conditions:
/// - an `Instruction::Panic` was reached (`Interrupt::Panic`)
///
/// # Panics
/// Panics if passed a program that wasn't successfully compiled for the same version
pub fn run<O>(bytecode: &[u8], out: &mut O) -> Result<(), Interrupt>
where
    O: Write,
{
    let registers = vec![
        const { Value::Never };
        bytecode[..2]
            .as_array::<2>()
            .map(|array| u16::from_le_bytes(*array))
            .map(usize::from)
            .expect("maximum registers is unknown")
    ];

    let mut vm = Vm {
        registers,
        arena: vec![],
        allocated: 0,
        next_gc: DEFAULT_NEXT_GC,
        call_frames: vec![CallFrame {
            fp: 0,
            from: 0,
            to: 0,
        }],
        function_labels: vec![],
        literals: vec![],
        ip: 0,
        sp: None,
        fp: 0,
    };

    let instructions = crate::targets::vm::decompile(bytecode, &mut vm);

    vm.run(instructions.as_slice(), out)
}

macro_rules! binary_op_arithmetic {
    (
        $self:ident, $out:ident, $to:ident, $op:tt, $lhs:expr, $rhs:expr $(,)?
    ) => {{
        match ($lhs, $rhs) {
            ($crate::targets::vm::Value::U8 { value: lhs }, $crate::targets::vm::Value::U8 { value: rhs }) => {
                $self.assign($to, $crate::targets::vm::Value::U8 { value: lhs $op rhs });

                Ok(())
            }
            ($crate::targets::vm::Value::I8 { value: lhs }, $crate::targets::vm::Value::I8 { value: rhs }) => {
                $self.assign($to, $crate::targets::vm::Value::I8 { value: lhs $op rhs });

                Ok(())
            }
            ($crate::targets::vm::Value::U16 { value: lhs }, $crate::targets::vm::Value::U16 { value: rhs }) => {
                $self.assign($to, $crate::targets::vm::Value::U16 { value: lhs $op rhs });

                Ok(())
            }
            ($crate::targets::vm::Value::I16 { value: lhs }, $crate::targets::vm::Value::I16 { value: rhs }) => {
                $self.assign($to, $crate::targets::vm::Value::I16 { value: lhs $op rhs });

                Ok(())
            }
            ($crate::targets::vm::Value::U32 { value: lhs }, $crate::targets::vm::Value::U32 { value: rhs }) => {
                $self.assign($to, $crate::targets::vm::Value::U32 { value: lhs $op rhs });

                Ok(())
            }
            ($crate::targets::vm::Value::I32 { value: lhs }, $crate::targets::vm::Value::I32 { value: rhs }) => {
                $self.assign($to, $crate::targets::vm::Value::I32 { value: lhs $op rhs });

                Ok(())
            }
            ($crate::targets::vm::Value::U64 { value: lhs }, $crate::targets::vm::Value::U64 { value: rhs }) => {
                $self.assign($to, $crate::targets::vm::Value::U64 { value: lhs $op rhs });

                Ok(())
            }
            ($crate::targets::vm::Value::I64 { value: lhs }, $crate::targets::vm::Value::I64 { value: rhs }) => {
                $self.assign($to, $crate::targets::vm::Value::I64 { value: lhs $op rhs });

                Ok(())
            }
            ($crate::targets::vm::Value::F32 { value: lhs }, $crate::targets::vm::Value::F32 { value: rhs }) => {
                $self.assign($to, $crate::targets::vm::Value::F32 { value: lhs $op rhs });

                Ok(())
            }
            ($crate::targets::vm::Value::F64 { value: lhs }, $crate::targets::vm::Value::F64 { value: rhs }) => {
                $self.assign($to, $crate::targets::vm::Value::F64 { value: lhs $op rhs });

                Ok(())
            }
            (
                lhs @ ($crate::targets::vm::Value::U8 { .. }
                | $crate::targets::vm::Value::I8 { .. }
                | $crate::targets::vm::Value::U16 { .. }
                | $crate::targets::vm::Value::I16 { .. }
                | $crate::targets::vm::Value::U32 { .. }
                | $crate::targets::vm::Value::I32 { .. }
                | $crate::targets::vm::Value::U64 { .. }
                | $crate::targets::vm::Value::I64 { .. }
                | $crate::targets::vm::Value::F32 { .. }
                | $crate::targets::vm::Value::F64 { .. }
                | $crate::targets::vm::Value::Boolean { .. }
                | $crate::targets::vm::Value::Unit
                | $crate::targets::vm::Value::ApartFn { .. }
                | $crate::targets::vm::Value::ApartNativeFn { .. }
                | $crate::targets::vm::Value::ValueIndex { .. }
                | $crate::targets::vm::Value::Never
                | $crate::targets::vm::Value::ApartRegister { .. }),
                rhs @ ($crate::targets::vm::Value::U8 { .. }
                | $crate::targets::vm::Value::I8 { .. }
                | $crate::targets::vm::Value::U16 { .. }
                | $crate::targets::vm::Value::I16 { .. }
                | $crate::targets::vm::Value::U32 { .. }
                | $crate::targets::vm::Value::I32 { .. }
                | $crate::targets::vm::Value::U64 { .. }
                | $crate::targets::vm::Value::I64 { .. }
                | $crate::targets::vm::Value::F32 { .. }
                | $crate::targets::vm::Value::F64 { .. }
                | $crate::targets::vm::Value::Boolean { .. }
                | $crate::targets::vm::Value::Unit
                | $crate::targets::vm::Value::ApartFn { .. }
                | $crate::targets::vm::Value::ApartNativeFn { .. }
                | $crate::targets::vm::Value::ValueIndex { .. }
                | $crate::targets::vm::Value::Never
                | $crate::targets::vm::Value::ApartRegister { .. }),
            ) => {
                writeln!(
                    $out,
                    "invalid operands for {} (lhs is {}, rhs is {})",
                    stringify!($op),
                    $self.value_to_string(&lhs, &mut std::collections::HashSet::new()),
                    $self.value_to_string(&rhs, &mut std::collections::HashSet::new())
                )
                .map_err(|_| Interrupt::Panic)
                .and_then(|()| $self.panic($out, None))
            }
        }
    }};
}

macro_rules! binary_op_comparison {
    (
        $self:ident, $out:ident, $to:ident, $op:tt, $lhs:expr, $rhs:expr $(,)?
    ) => {{
        match ($lhs, $rhs) {
            ($crate::targets::vm::Value::U8 { value: lhs }, $crate::targets::vm::Value::U8 { value: rhs }) => {
                $self.assign($to, $crate::targets::vm::Value::Boolean { value: lhs $op rhs });

                Ok(())
            }
            ($crate::targets::vm::Value::I8 { value: lhs }, $crate::targets::vm::Value::I8 { value: rhs }) => {
                $self.assign($to, $crate::targets::vm::Value::Boolean { value: lhs $op rhs });

                Ok(())
            }
            ($crate::targets::vm::Value::U16 { value: lhs }, $crate::targets::vm::Value::U16 { value: rhs }) => {
                $self.assign($to, $crate::targets::vm::Value::Boolean { value: lhs $op rhs });

                Ok(())
            }
            ($crate::targets::vm::Value::I16 { value: lhs }, $crate::targets::vm::Value::I16 { value: rhs }) => {
                $self.assign($to, $crate::targets::vm::Value::Boolean { value: lhs $op rhs });

                Ok(())
            }
            ($crate::targets::vm::Value::U32 { value: lhs }, $crate::targets::vm::Value::U32 { value: rhs }) => {
                $self.assign($to, $crate::targets::vm::Value::Boolean { value: lhs $op rhs });

                Ok(())
            }
            ($crate::targets::vm::Value::I32 { value: lhs }, $crate::targets::vm::Value::I32 { value: rhs }) => {
                $self.assign($to, $crate::targets::vm::Value::Boolean { value: lhs $op rhs });

                Ok(())
            }
            ($crate::targets::vm::Value::U64 { value: lhs }, $crate::targets::vm::Value::U64 { value: rhs }) => {
                $self.assign($to, $crate::targets::vm::Value::Boolean { value: lhs $op rhs });

                Ok(())
            }
            ($crate::targets::vm::Value::I64 { value: lhs }, $crate::targets::vm::Value::I64 { value: rhs }) => {
                $self.assign($to, $crate::targets::vm::Value::Boolean { value: lhs $op rhs });

                Ok(())
            }
            ($crate::targets::vm::Value::F32 { value: lhs }, $crate::targets::vm::Value::F32 { value: rhs }) => {
                $self.assign($to, $crate::targets::vm::Value::Boolean { value: lhs $op rhs });

                Ok(())
            }
            ($crate::targets::vm::Value::F64 { value: lhs }, $crate::targets::vm::Value::F64 { value: rhs }) => {
                $self.assign($to, $crate::targets::vm::Value::Boolean { value: lhs $op rhs });

                Ok(())
            }
            (
                lhs @ ($crate::targets::vm::Value::U8 { .. }
                | $crate::targets::vm::Value::I8 { .. }
                | $crate::targets::vm::Value::U16 { .. }
                | $crate::targets::vm::Value::I16 { .. }
                | $crate::targets::vm::Value::U32 { .. }
                | $crate::targets::vm::Value::I32 { .. }
                | $crate::targets::vm::Value::U64 { .. }
                | $crate::targets::vm::Value::I64 { .. }
                | $crate::targets::vm::Value::F32 { .. }
                | $crate::targets::vm::Value::F64 { .. }
                | $crate::targets::vm::Value::Boolean { .. }
                | $crate::targets::vm::Value::Unit
                | $crate::targets::vm::Value::ApartFn { .. }
                | $crate::targets::vm::Value::ApartNativeFn { .. }
                | $crate::targets::vm::Value::ValueIndex { .. }
                | $crate::targets::vm::Value::Never
                | $crate::targets::vm::Value::ApartRegister { .. }),
                rhs @ ($crate::targets::vm::Value::U8 { .. }
                | $crate::targets::vm::Value::I8 { .. }
                | $crate::targets::vm::Value::U16 { .. }
                | $crate::targets::vm::Value::I16 { .. }
                | $crate::targets::vm::Value::U32 { .. }
                | $crate::targets::vm::Value::I32 { .. }
                | $crate::targets::vm::Value::U64 { .. }
                | $crate::targets::vm::Value::I64 { .. }
                | $crate::targets::vm::Value::F32 { .. }
                | $crate::targets::vm::Value::F64 { .. }
                | $crate::targets::vm::Value::Boolean { .. }
                | $crate::targets::vm::Value::Unit
                | $crate::targets::vm::Value::ApartFn { .. }
                | $crate::targets::vm::Value::ApartNativeFn { .. }
                | $crate::targets::vm::Value::ValueIndex { .. }
                | $crate::targets::vm::Value::Never
                | $crate::targets::vm::Value::ApartRegister { .. }),
            ) => {
                writeln!(
                    $out,
                    "invalid operands for {} (lhs is {}, rhs is {})",
                    stringify!($op),
                    $self.value_to_string(&lhs, &mut std::collections::HashSet::new()),
                    $self.value_to_string(&rhs, &mut std::collections::HashSet::new())
                )
                .map_err(|_| Interrupt::Panic)
                .and_then(|()| $self.panic($out, None))
            }
        }
    }};
}

impl Vm {
    fn value_to_string(&self, value: &Value, seen: &mut HashSet<ValueIndex>) -> String {
        if let Value::ValueIndex { index: value_index } = value
            && let Some(value) = self.arena.get(usize::from(*value_index))
        {
            if !seen.insert(*value_index) {
                return format!("{value_index:?}");
            }

            match value {
                GarbageCollectedValue::Compound { fields, .. } => {
                    let mut buffer = "Compound(".to_string();

                    if let Some(first_field) = fields.first() {
                        buffer.push_str(self.value_to_string(first_field, seen).as_str());

                        for field in fields.iter().skip(1) {
                            buffer.push_str(", ");

                            buffer.push_str(self.value_to_string(field, seen).as_str());
                        }
                    }

                    buffer.push(')');

                    buffer
                }
                GarbageCollectedValue::TaggedCompound { tag, fields, .. } => {
                    let mut buffer = format!("TaggedCompound {{ tag: {tag}, fields: [");

                    if let Some(first_field) = fields.first() {
                        buffer.push_str(self.value_to_string(first_field, seen).as_str());

                        for field in fields.iter().skip(1) {
                            buffer.push_str(", ");

                            buffer.push_str(self.value_to_string(field, seen).as_str());
                        }
                    }

                    buffer.push_str("] }");

                    buffer
                }
                GarbageCollectedValue::ApartString { value: text } => {
                    format!("String({text:?})")
                }
                GarbageCollectedValue::ApartVec { values, .. } => {
                    let mut buffer = "Vec([".to_string();

                    if let Some(first_value) = values.first() {
                        buffer.push_str(self.value_to_string(first_value, seen).as_str());

                        for value in values.iter().skip(1) {
                            buffer.push_str(", ");

                            buffer.push_str(self.value_to_string(value, seen).as_str());
                        }
                    }

                    buffer.push_str("])");

                    buffer
                }
            }
        } else {
            format!("{value:?}")
        }
    }

    fn registers_to_string(&self) -> String {
        let mut buffer = "[".to_string();

        if let Some(first_value) = self.registers[(self.fp)..].first() {
            buffer.push_str(
                self.value_to_string(first_value, &mut HashSet::new())
                    .as_str(),
            );

            for value in self.registers.iter().skip(self.fp + 1) {
                buffer.push_str(", ");

                buffer.push_str(self.value_to_string(value, &mut HashSet::new()).as_str());
            }
        }

        buffer.push(']');

        buffer
    }

    fn run<O>(&mut self, instructions: &[Instruction], out: &mut O) -> Result<(), Interrupt>
    where
        O: Write,
    {
        while let Some(instruction) = instructions.get(self.ip) {
            if cfg!(feature = "step") && !cfg!(test) {
                println!("@{}: {:?}", self.ip, instruction);

                println!();

                println!("    {}", self.registers_to_string());

                println!();

                println!("    {:?}", self.sp);

                std::io::stdin().lock().lines().next();
            }

            self.ip += 1;

            self.step(instruction, out).inspect_err(|_| {
                self.registers.clear();

                self.literals.clear();

                self.gc();

                assert_eq!(self.allocated, 0, "{} BYTES LEAKED", self.allocated);
            })?;

            if self.call_frames.is_empty() {
                break;
            }
        }

        self.registers.clear();

        self.literals.clear();

        self.gc();

        assert_eq!(self.allocated, 0, "{} BYTES LEAKED", self.allocated);

        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub fn step<O>(&mut self, instruction: &Instruction, out: &mut O) -> Result<(), Interrupt>
    where
        O: Write,
    {
        match instruction {
            Instruction::NoOp | Instruction::Label => Ok(()),
            Instruction::Panic => self.panic(out, None),
            Instruction::ApartFnLabel { fn_label } => {
                self.function_labels.push(ApartFnLabel::new(
                    fn_label.file_name().to_string(),
                    fn_label.function_label().to_string(),
                ));

                Ok(())
            }
            Instruction::Not { to, operand } => {
                let operand = self.get_register(*operand);

                self.op_logical_not(out, *to, &operand)
            }
            Instruction::NotValue { to, operand } => self
                .dereference_value(out, operand, true)
                .and_then(|operand| self.op_logical_not(out, *to, &operand)),
            Instruction::Negate { to, operand } => {
                let operand = self.get_register(*operand);

                self.op_arithmetic_negate(out, *to, &operand)
            }
            Instruction::NegateValue { to, operand } => self
                .dereference_value(out, operand, true)
                .and_then(|operand| self.op_arithmetic_negate(out, *to, &operand)),
            Instruction::Multiply { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_arithmetic_multiply(out, *to, &lhs, &rhs)
            }
            Instruction::MultiplyRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);

                self.dereference_value(out, rhs, true)
                    .and_then(|rhs| self.op_arithmetic_multiply(out, *to, &lhs, &rhs))
            }
            Instruction::MultiplyValueRegister { to, lhs, rhs } => {
                let rhs = self.get_register(*rhs);

                self.dereference_value(out, lhs, true)
                    .and_then(|lhs| self.op_arithmetic_multiply(out, *to, &lhs, &rhs))
            }
            Instruction::MultiplyValueValue { to, lhs, rhs } => {
                self.dereference_value(out, lhs, true).and_then(|lhs| {
                    self.dereference_value(out, rhs, true)
                        .and_then(|rhs| self.op_arithmetic_multiply(out, *to, &lhs, &rhs))
                })
            }
            Instruction::Divide { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_arithmetic_divide(out, *to, &lhs, &rhs)
            }
            Instruction::DivideRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);

                self.dereference_value(out, rhs, true)
                    .and_then(|rhs| self.op_arithmetic_divide(out, *to, &lhs, &rhs))
            }
            Instruction::DivideValueRegister { to, lhs, rhs } => {
                let rhs = self.get_register(*rhs);

                self.dereference_value(out, lhs, true)
                    .and_then(|lhs| self.op_arithmetic_divide(out, *to, &lhs, &rhs))
            }
            Instruction::DivideValueValue { to, lhs, rhs } => {
                self.dereference_value(out, lhs, true).and_then(|lhs| {
                    self.dereference_value(out, rhs, true)
                        .and_then(|rhs| self.op_arithmetic_divide(out, *to, &lhs, &rhs))
                })
            }
            Instruction::Remainder { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_arithmetic_remainder(out, *to, &lhs, &rhs)
            }
            Instruction::RemainderRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);

                self.dereference_value(out, rhs, true)
                    .and_then(|rhs| self.op_arithmetic_remainder(out, *to, &lhs, &rhs))
            }
            Instruction::RemainderValueRegister { to, lhs, rhs } => {
                let rhs = self.get_register(*rhs);

                self.dereference_value(out, lhs, true)
                    .and_then(|lhs| self.op_arithmetic_remainder(out, *to, &lhs, &rhs))
            }
            Instruction::RemainderValueValue { to, lhs, rhs } => {
                self.dereference_value(out, lhs, true).and_then(|lhs| {
                    self.dereference_value(out, rhs, true)
                        .and_then(|rhs| self.op_arithmetic_remainder(out, *to, &lhs, &rhs))
                })
            }
            Instruction::Add { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_arithmetic_add(out, *to, &lhs, &rhs)
            }
            Instruction::AddRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);

                self.dereference_value(out, rhs, true)
                    .and_then(|rhs| self.op_arithmetic_add(out, *to, &lhs, &rhs))
            }
            Instruction::AddValueRegister { to, lhs, rhs } => {
                let rhs = self.get_register(*rhs);

                self.dereference_value(out, lhs, true)
                    .and_then(|lhs| self.op_arithmetic_add(out, *to, &lhs, &rhs))
            }
            Instruction::AddValueValue { to, lhs, rhs } => {
                self.dereference_value(out, lhs, true).and_then(|lhs| {
                    self.dereference_value(out, rhs, true)
                        .and_then(|rhs| self.op_arithmetic_add(out, *to, &lhs, &rhs))
                })
            }
            Instruction::Subtract { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_arithmetic_subtract(out, *to, &lhs, &rhs)
            }
            Instruction::SubtractRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);

                self.dereference_value(out, rhs, true)
                    .and_then(|rhs| self.op_arithmetic_subtract(out, *to, &lhs, &rhs))
            }
            Instruction::SubtractValueRegister { to, lhs, rhs } => {
                let rhs = self.get_register(*rhs);

                self.dereference_value(out, lhs, true)
                    .and_then(|lhs| self.op_arithmetic_subtract(out, *to, &lhs, &rhs))
            }
            Instruction::SubtractValueValue { to, lhs, rhs } => {
                self.dereference_value(out, lhs, true).and_then(|lhs| {
                    self.dereference_value(out, rhs, true)
                        .and_then(|rhs| self.op_arithmetic_subtract(out, *to, &lhs, &rhs))
                })
            }
            Instruction::Less { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_comparison_less(out, *to, &lhs, &rhs)
            }
            Instruction::LessRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);

                self.dereference_value(out, rhs, true)
                    .and_then(|rhs| self.op_comparison_less(out, *to, &lhs, &rhs))
            }
            Instruction::LessValueRegister { to, lhs, rhs } => {
                let rhs = self.get_register(*rhs);

                self.dereference_value(out, lhs, true)
                    .and_then(|lhs| self.op_comparison_less(out, *to, &lhs, &rhs))
            }
            Instruction::LessValueValue { to, lhs, rhs } => {
                self.dereference_value(out, lhs, true).and_then(|lhs| {
                    self.dereference_value(out, rhs, true)
                        .and_then(|rhs| self.op_comparison_less(out, *to, &lhs, &rhs))
                })
            }
            Instruction::Greater { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_comparison_greater(out, *to, &lhs, &rhs)
            }
            Instruction::GreaterRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);

                self.dereference_value(out, rhs, true)
                    .and_then(|rhs| self.op_comparison_greater(out, *to, &lhs, &rhs))
            }
            Instruction::GreaterValueRegister { to, lhs, rhs } => {
                let rhs = self.get_register(*rhs);

                self.dereference_value(out, lhs, true)
                    .and_then(|lhs| self.op_comparison_greater(out, *to, &lhs, &rhs))
            }
            Instruction::GreaterValueValue { to, lhs, rhs } => {
                self.dereference_value(out, lhs, true).and_then(|lhs| {
                    self.dereference_value(out, rhs, true)
                        .and_then(|rhs| self.op_comparison_greater(out, *to, &lhs, &rhs))
                })
            }
            Instruction::LessOrEqual { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_comparison_less_or_equal(out, *to, &lhs, &rhs)
            }
            Instruction::LessOrEqualRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);

                self.dereference_value(out, rhs, true)
                    .and_then(|rhs| self.op_comparison_less_or_equal(out, *to, &lhs, &rhs))
            }
            Instruction::LessOrEqualValueRegister { to, lhs, rhs } => {
                let rhs = self.get_register(*rhs);

                self.dereference_value(out, lhs, true)
                    .and_then(|lhs| self.op_comparison_less_or_equal(out, *to, &lhs, &rhs))
            }
            Instruction::LessOrEqualValueValue { to, lhs, rhs } => {
                self.dereference_value(out, lhs, true).and_then(|lhs| {
                    self.dereference_value(out, rhs, true)
                        .and_then(|rhs| self.op_comparison_less_or_equal(out, *to, &lhs, &rhs))
                })
            }
            Instruction::GreaterOrEqual { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_comparison_greater_or_equal(out, *to, &lhs, &rhs)
            }
            Instruction::GreaterOrEqualRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);

                self.dereference_value(out, rhs, true)
                    .and_then(|rhs| self.op_comparison_greater_or_equal(out, *to, &lhs, &rhs))
            }
            Instruction::GreaterOrEqualValueRegister { to, lhs, rhs } => {
                let rhs = self.get_register(*rhs);

                self.dereference_value(out, lhs, true)
                    .and_then(|lhs| self.op_comparison_greater_or_equal(out, *to, &lhs, &rhs))
            }
            Instruction::GreaterOrEqualValueValue { to, lhs, rhs } => {
                self.dereference_value(out, lhs, true).and_then(|lhs| {
                    self.dereference_value(out, rhs, true)
                        .and_then(|rhs| self.op_comparison_greater_or_equal(out, *to, &lhs, &rhs))
                })
            }
            Instruction::Equal { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_comparison_equal(out, *to, &lhs, &rhs)
            }
            Instruction::EqualRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);

                self.dereference_value(out, rhs, true)
                    .and_then(|rhs| self.op_comparison_equal(out, *to, &lhs, &rhs))
            }
            Instruction::EqualValueRegister { to, lhs, rhs } => {
                let rhs = self.get_register(*rhs);

                self.dereference_value(out, lhs, true)
                    .and_then(|lhs| self.op_comparison_equal(out, *to, &lhs, &rhs))
            }
            Instruction::EqualValueValue { to, lhs, rhs } => {
                self.dereference_value(out, lhs, true).and_then(|lhs| {
                    self.dereference_value(out, rhs, true)
                        .and_then(|rhs| self.op_comparison_equal(out, *to, &lhs, &rhs))
                })
            }
            Instruction::NotEqual { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_comparison_not_equal(out, *to, &lhs, &rhs)
            }
            Instruction::NotEqualRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);

                self.dereference_value(out, rhs, true)
                    .and_then(|rhs| self.op_comparison_not_equal(out, *to, &lhs, &rhs))
            }
            Instruction::NotEqualValueRegister { to, lhs, rhs } => {
                let rhs = self.get_register(*rhs);

                self.dereference_value(out, lhs, true)
                    .and_then(|lhs| self.op_comparison_not_equal(out, *to, &lhs, &rhs))
            }
            Instruction::NotEqualValueValue { to, lhs, rhs } => {
                self.dereference_value(out, lhs, true).and_then(|lhs| {
                    self.dereference_value(out, rhs, true)
                        .and_then(|rhs| self.op_comparison_not_equal(out, *to, &lhs, &rhs))
                })
            }
            Instruction::And { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_logical_and(out, *to, &lhs, &rhs)
            }
            Instruction::AndRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);

                self.dereference_value(out, rhs, true)
                    .and_then(|rhs| self.op_logical_and(out, *to, &lhs, &rhs))
            }
            Instruction::AndValueRegister { to, lhs, rhs } => {
                let rhs = self.get_register(*rhs);

                self.dereference_value(out, lhs, true)
                    .and_then(|lhs| self.op_logical_and(out, *to, &lhs, &rhs))
            }
            Instruction::AndValueValue { to, lhs, rhs } => {
                self.dereference_value(out, lhs, true).and_then(|lhs| {
                    self.dereference_value(out, rhs, true)
                        .and_then(|rhs| self.op_logical_and(out, *to, &lhs, &rhs))
                })
            }
            Instruction::Or { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_logical_or(out, *to, &lhs, &rhs)
            }
            Instruction::OrRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);

                self.dereference_value(out, rhs, true)
                    .and_then(|rhs| self.op_logical_or(out, *to, &lhs, &rhs))
            }
            Instruction::OrValueRegister { to, lhs, rhs } => {
                let rhs = self.get_register(*rhs);

                self.dereference_value(out, lhs, true)
                    .and_then(|lhs| self.op_logical_or(out, *to, &lhs, &rhs))
            }
            Instruction::OrValueValue { to, lhs, rhs } => {
                self.dereference_value(out, lhs, true).and_then(|lhs| {
                    self.dereference_value(out, rhs, true)
                        .and_then(|rhs| self.op_logical_or(out, *to, &lhs, &rhs))
                })
            }
            Instruction::Assign { to, operand } => {
                let operand = self.get_register(*operand);

                self.op_assign(out, *to, operand)
            }
            Instruction::AssignValue { to, operand } => self
                .dereference_value(out, operand, true)
                .and_then(|operand| self.op_assign(out, *to, operand)),
            Instruction::Push { operand } => {
                let operand = self.get_register(*operand);

                self.op_push(out, operand)
            }
            Instruction::PushValue { operand } => self
                .dereference_value(out, operand, true)
                .and_then(|operand| self.op_push(out, operand)),
            Instruction::Access { to, index, of } => {
                let of = self.get_register(*of);

                self.op_access(out, *to, *index, &of)
            }
            Instruction::AccessValue { to, index, of } => self
                .dereference_value(out, of, true)
                .and_then(|of| self.op_access(out, *to, *index, &of)),
            Instruction::AccessAssign { to, index, value } => {
                let to = self.get_register(*to);
                let value = self.get_register(*value);

                self.op_access_assign(out, &to, *index, value)
            }
            Instruction::AccessAssignRegisterValue { to, index, value } => {
                let to = self.get_register(*to);

                self.dereference_value(out, value, true)
                    .and_then(|value| self.op_access_assign(out, &to, *index, value))
            }
            Instruction::AccessAssignValueRegister { to, index, value } => {
                let value = self.get_register(*value);

                self.dereference_value(out, to, true)
                    .and_then(|to| self.op_access_assign(out, &to, *index, value))
            }
            Instruction::AccessAssignValueValue { to, index, value } => {
                self.dereference_value(out, to, true).and_then(|to| {
                    self.dereference_value(out, value, true)
                        .and_then(|value| self.op_access_assign(out, &to, *index, value))
                })
            }
            Instruction::GetTag { to, of } => {
                let of = self.get_register(*of);

                self.op_get_tag(out, *to, &of)
            }
            Instruction::GetTagValue { to, of } => self
                .dereference_value(out, of, true)
                .and_then(|of| self.op_get_tag(out, *to, &of)),
            Instruction::Call { to, callee, .. } => {
                let callee = self.get_register(*callee);

                self.op_call(out, *to, &callee)
            }
            Instruction::CallValue { to, callee, .. } => self
                .dereference_value(out, callee, true)
                .and_then(|callee| self.op_call(out, *to, &callee)),
            Instruction::Jump { destination } => self.op_jump(out, *destination),
            Instruction::Branch {
                condition,
                when_true,
                otherwise,
            } => {
                let condition = self.get_register(*condition);

                self.op_branch(out, &condition, *when_true, *otherwise)
            }
            Instruction::BranchValue {
                condition,
                when_true,
                otherwise,
            } => self
                .dereference_value(out, condition, true)
                .and_then(|condition| self.op_branch(out, &condition, *when_true, *otherwise)),
            Instruction::Return { value } => {
                let value = self.get_register(*value);

                self.op_return(out, value)
            }
            Instruction::ReturnValue { value } => self
                .dereference_value(out, value, true)
                .and_then(|value| self.op_return(out, value)),
        }
    }

    fn op_logical_not<O>(&mut self, out: &mut O, to: u16, operand: &Value) -> Result<(), Interrupt>
    where
        O: Write,
    {
        match operand {
            Value::Boolean { value } => {
                self.assign(to, Value::Boolean { value: !value });

                Ok(())
            }
            Value::U8 { .. }
            | Value::I8 { .. }
            | Value::U16 { .. }
            | Value::I16 { .. }
            | Value::U32 { .. }
            | Value::I32 { .. }
            | Value::U64 { .. }
            | Value::I64 { .. }
            | Value::F32 { .. }
            | Value::F64 { .. }
            | Value::Unit
            | Value::ApartFn { .. }
            | Value::ApartNativeFn { .. }
            | Value::ValueIndex { .. }
            | Value::Never
            | Value::ApartRegister { .. } => writeln!(
                out,
                "invalid operand for logical not (got {})",
                self.value_to_string(operand, &mut HashSet::new())
            )
            .map_err(|_| Interrupt::Panic)
            .and_then(|()| self.panic(out, None)),
        }
    }

    fn op_arithmetic_negate<O>(
        &mut self,
        out: &mut O,
        to: u16,
        operand: &Value,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        match operand {
            Value::I8 { value } => {
                self.assign(to, Value::I8 { value: -value });

                Ok(())
            }
            Value::I16 { value } => {
                self.assign(to, Value::I16 { value: -value });

                Ok(())
            }
            Value::I32 { value } => {
                self.assign(to, Value::I32 { value: -value });

                Ok(())
            }
            Value::I64 { value } => {
                self.assign(to, Value::I64 { value: -value });

                Ok(())
            }
            Value::F32 { value } => {
                self.assign(to, Value::F32 { value: -value });

                Ok(())
            }
            Value::F64 { value } => {
                self.assign(to, Value::F64 { value: -value });

                Ok(())
            }
            Value::U8 { .. }
            | Value::U16 { .. }
            | Value::U32 { .. }
            | Value::U64 { .. }
            | Value::Boolean { .. }
            | Value::Unit
            | Value::ApartFn { .. }
            | Value::ApartNativeFn { .. }
            | Value::ValueIndex { .. }
            | Value::Never
            | Value::ApartRegister { .. } => writeln!(
                out,
                "invalid operand for arithmetic negate (got {})",
                self.value_to_string(operand, &mut HashSet::new())
            )
            .map_err(|_| Interrupt::Panic)
            .and_then(|()| self.panic(out, None)),
        }
    }

    fn op_arithmetic_multiply<O>(
        &mut self,
        out: &mut O,
        to: u16,
        lhs: &Value,
        rhs: &Value,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        binary_op_arithmetic!(self, out, to, *, lhs, rhs)
    }

    fn op_arithmetic_divide<O>(
        &mut self,
        out: &mut O,
        to: u16,
        lhs: &Value,
        rhs: &Value,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        binary_op_arithmetic!(self, out, to, /, lhs, rhs)
    }

    fn op_arithmetic_remainder<O>(
        &mut self,
        out: &mut O,
        to: u16,
        lhs: &Value,
        rhs: &Value,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        binary_op_arithmetic!(self, out, to, %, lhs, rhs)
    }

    fn op_arithmetic_add<O>(
        &mut self,
        out: &mut O,
        to: u16,
        lhs: &Value,
        rhs: &Value,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        binary_op_arithmetic!(self, out, to, +, lhs, rhs)
    }

    fn op_arithmetic_subtract<O>(
        &mut self,
        out: &mut O,
        to: u16,
        lhs: &Value,
        rhs: &Value,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        binary_op_arithmetic!(self, out, to, -, lhs, rhs)
    }

    fn op_comparison_less<O>(
        &mut self,
        out: &mut O,
        to: u16,
        lhs: &Value,
        rhs: &Value,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        binary_op_comparison!(self, out, to, <, lhs, rhs)
    }

    fn op_comparison_greater<O>(
        &mut self,
        out: &mut O,
        to: u16,
        lhs: &Value,
        rhs: &Value,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        binary_op_comparison!(self, out, to, >, lhs, rhs)
    }

    fn op_comparison_less_or_equal<O>(
        &mut self,
        out: &mut O,
        to: u16,
        lhs: &Value,
        rhs: &Value,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        binary_op_comparison!(self, out, to, <=, lhs, rhs)
    }

    fn op_comparison_greater_or_equal<O>(
        &mut self,
        out: &mut O,
        to: u16,
        lhs: &Value,
        rhs: &Value,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        binary_op_comparison!(self, out, to, >=, lhs, rhs)
    }

    #[allow(clippy::unnecessary_wraps)]
    fn op_comparison_equal<O>(
        &mut self,
        _: &mut O,
        to: u16,
        lhs: &Value,
        rhs: &Value,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        self.assign(
            to,
            Value::Boolean {
                value: self.values_eq(lhs, rhs),
            },
        );

        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)]
    fn op_comparison_not_equal<O>(
        &mut self,
        _: &mut O,
        to: u16,
        lhs: &Value,
        rhs: &Value,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        self.assign(
            to,
            Value::Boolean {
                value: !self.values_eq(lhs, rhs),
            },
        );

        Ok(())
    }

    fn op_logical_and<O>(
        &mut self,
        out: &mut O,
        to: u16,
        lhs: &Value,
        rhs: &Value,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        if let (Value::Boolean { value: lhs }, Value::Boolean { value: rhs }) = (&lhs, &rhs) {
            self.assign(
                to,
                Value::Boolean {
                    value: *lhs && *rhs,
                },
            );

            Ok(())
        } else {
            writeln!(
                out,
                "invalid operands for logical and (lhs is {}, rhs is {})",
                self.value_to_string(lhs, &mut HashSet::new()),
                self.value_to_string(rhs, &mut HashSet::new())
            )
            .map_err(|_| Interrupt::Panic)
            .and_then(|()| self.panic(out, None))
        }
    }

    fn op_logical_or<O>(
        &mut self,
        out: &mut O,
        to: u16,
        lhs: &Value,
        rhs: &Value,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        if let (Value::Boolean { value: lhs }, Value::Boolean { value: rhs }) = (&lhs, &rhs) {
            self.assign(
                to,
                Value::Boolean {
                    value: *lhs || *rhs,
                },
            );

            Ok(())
        } else {
            writeln!(
                out,
                "invalid operands for logical or (lhs is {}, rhs is {})",
                self.value_to_string(lhs, &mut HashSet::new()),
                self.value_to_string(rhs, &mut HashSet::new())
            )
            .map_err(|_| Interrupt::Panic)
            .and_then(|()| self.panic(out, None))
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    fn op_assign<O>(&mut self, _: &mut O, to: u16, operand: Value) -> Result<(), Interrupt>
    where
        O: Write,
    {
        self.assign(to, operand);

        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)]
    fn op_push<O>(&mut self, _: &mut O, operand: Value) -> Result<(), Interrupt>
    where
        O: Write,
    {
        if self.sp.is_none() {
            self.sp = Some(self.registers.len());
        }

        self.registers.push(operand);

        Ok(())
    }

    fn op_access<O>(&mut self, out: &mut O, to: u16, index: u8, of: &Value) -> Result<(), Interrupt>
    where
        O: Write,
    {
        if let Value::ValueIndex { index: value_index } = of
            && let GarbageCollectedValue::Compound { fields, .. }
            | GarbageCollectedValue::TaggedCompound { fields, .. } =
                &self.arena[usize::from(*value_index)]
        {
            self.assign(to, fields[usize::from(index)]);

            Ok(())
        } else {
            writeln!(
                out,
                "tried access to a field of something other than a compound (got {})",
                self.value_to_string(of, &mut HashSet::new())
            )
            .map_err(|_| Interrupt::Panic)
            .and_then(|()| self.panic(out, None))
        }
    }

    fn op_access_assign<O>(
        &mut self,
        out: &mut O,
        to: &Value,
        index: u8,
        value: Value,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        if let Value::ValueIndex { index: value_index } = to
            && let GarbageCollectedValue::Compound { fields, .. }
            | GarbageCollectedValue::TaggedCompound { fields, .. } =
                &mut self.arena[usize::from(*value_index)]
        {
            fields[usize::from(index)] = value;

            Ok(())
        } else {
            writeln!(
                out,
                "tried to assign to a field of something other than a compound (got {})",
                self.value_to_string(to, &mut HashSet::new())
            )
            .map_err(|_| Interrupt::Panic)
            .and_then(|()| self.panic(out, None))
        }
    }

    fn op_get_tag<O>(&mut self, out: &mut O, to: u16, of: &Value) -> Result<(), Interrupt>
    where
        O: Write,
    {
        if let Value::ValueIndex { index: value_index } = of
            && let GarbageCollectedValue::TaggedCompound { tag, .. } =
                self.arena[usize::from(*value_index)]
        {
            self.assign(to, Value::U16 { value: tag });

            Ok(())
        } else {
            writeln!(
                out,
                "tried to get the tag of something other than a tagged compound (got {})",
                self.value_to_string(of, &mut HashSet::new())
            )
            .map_err(|_| Interrupt::Panic)
            .and_then(|()| self.panic(out, None))
        }
    }

    fn op_call<O>(&mut self, out: &mut O, to: u16, callee: &Value) -> Result<(), Interrupt>
    where
        O: Write,
    {
        let sp = self.sp.take().unwrap_or(self.registers.len());

        match callee {
            Value::ApartFn {
                address,
                max_register_count,
            } => {
                if self.should_gc() {
                    self.gc();
                }

                let call_frame = CallFrame {
                    from: u32::try_from(self.ip).expect("return address was not a valid u32"),
                    fp: sp,
                    to,
                };

                self.ip = usize::try_from(*address).expect("call address was not a valid usize");

                self.fp = sp;

                self.call_frames.push(call_frame);

                self.registers.resize_with(
                    self.registers.len() + usize::from(*max_register_count),
                    || Value::Never,
                );

                Ok(())
            }
            Value::ApartNativeFn { value: native_fn } => {
                let call_arguments = self.registers.drain(sp..).collect::<Vec<_>>();

                self.native_fn_call(call_arguments.as_slice(), *native_fn, out)
                    .map(|value| {
                        self.assign(to, value);
                    })
            }
            _ => writeln!(
                out,
                "tried to call an uncallable (got {})",
                self.value_to_string(callee, &mut HashSet::new())
            )
            .map_err(|_| Interrupt::Panic)
            .and_then(|()| self.panic(out, None)),
        }
    }

    fn op_jump<O>(&mut self, out: &mut O, destination: u32) -> Result<(), Interrupt>
    where
        O: Write,
    {
        if let Ok(destination) = usize::try_from(destination) {
            self.ip = destination;

            Ok(())
        } else {
            writeln!(out, "jump address was not valid")
                .map_err(|_| Interrupt::Panic)
                .and_then(|()| self.panic(out, None))
        }
    }

    #[inline(never)]
    fn op_branch<O>(
        &mut self,
        out: &mut O,
        condition: &Value,
        when_true: u32,
        otherwise: u32,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        match condition {
            Value::Boolean { value: true } => self.op_jump(out, when_true),
            Value::Boolean { value: false } => self.op_jump(out, otherwise),
            _ => writeln!(
                out,
                "branch condition was not a boolean (got {})",
                self.value_to_string(condition, &mut HashSet::new())
            )
            .map_err(|_| Interrupt::Panic)
            .and_then(|()| self.panic(out, None)),
        }
    }

    fn op_return<O>(&mut self, out: &mut O, value: Value) -> Result<(), Interrupt>
    where
        O: Write,
    {
        if let Some(call_frame) = self.call_frames.pop() {
            if let Ok(return_to) = usize::try_from(call_frame.from) {
                self.ip = return_to;

                self.fp = self.call_frames.last().map_or(0, |frame| frame.fp);

                self.registers.truncate(call_frame.fp);

                self.function_labels.pop();

                if !self.call_frames.is_empty() {
                    self.assign(call_frame.to, value);
                }

                if self.should_gc() {
                    self.gc();
                }

                Ok(())
            } else {
                writeln!(out, "return address was not valid")
                    .map_err(|_| Interrupt::Panic)
                    .and_then(|()| self.panic(out, None))
            }
        } else {
            writeln!(out, "tried to return from outside of all functions")
                .map_err(|_| Interrupt::Panic)
                .and_then(|()| self.panic(out, None))
        }
    }

    #[allow(clippy::needless_pass_by_ref_mut)]
    fn assign(&mut self, to: u16, value: Value) {
        let offset = self.fp + usize::from(to);

        self.registers[offset] = value;
    }

    fn get_register(&self, from: u16) -> Value {
        self.registers[self.fp + usize::from(from)]
    }

    fn panic<O>(&self, out: &mut O, location: Option<(usize, usize)>) -> Result<(), Interrupt>
    where
        O: Write,
    {
        if let Some(fn_label) = self.function_labels.last() {
            if let Some((line_number, column)) = location {
                writeln!(
                    out,
                    "VM panicked at {}:{line_number}:{column} in {}",
                    fn_label.file_name(),
                    fn_label.function_label(),
                )
            } else {
                writeln!(
                    out,
                    "VM panicked at {} in {}",
                    fn_label.file_name(),
                    fn_label.function_label()
                )
            }
        } else {
            writeln!(out, "VM panicked")
        }
        .expect("failed to write to output");

        Err(Interrupt::Panic)
    }

    #[allow(
        clippy::too_many_lines,
        clippy::unnecessary_wraps,
        clippy::only_used_in_recursion
    )]
    fn dereference_value<O>(
        &mut self,
        out: &mut O,
        value: &Value,
        make_literal: bool,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        match value {
            Value::ApartRegister { index } => Ok(self.get_register(*index)),
            Value::ValueIndex { index: value_index } => {
                match &self.arena[usize::from(*value_index)] {
                    GarbageCollectedValue::Compound {
                        field_count,
                        fields,
                    } if make_literal => {
                        let field_count = *field_count;

                        let fields =
                            fields
                                .clone()
                                .into_iter()
                                .try_fold(vec![], |mut accum, field| {
                                    self.dereference_value(out, &field, make_literal)
                                        .map(|value| {
                                            accum.push(value);

                                            accum
                                        })
                                })?;

                        self.arena.push(GarbageCollectedValue::Compound {
                            field_count,
                            fields: fields.into_boxed_slice(),
                        });

                        let value_index = Value::ValueIndex {
                            index: ValueIndex::from(self.arena.len() - 1),
                        };

                        self.allocated += self.size_of_value(&value_index);

                        Ok(value_index)
                    }
                    GarbageCollectedValue::TaggedCompound {
                        tag,
                        field_count,
                        fields,
                    } if make_literal => {
                        let tag = *tag;

                        let field_count = *field_count;

                        let fields =
                            fields
                                .clone()
                                .into_iter()
                                .try_fold(vec![], |mut accum, field| {
                                    self.dereference_value(out, &field, make_literal)
                                        .map(|value| {
                                            accum.push(value);

                                            accum
                                        })
                                })?;

                        self.arena.push(GarbageCollectedValue::TaggedCompound {
                            tag,
                            field_count,
                            fields: fields.into_boxed_slice(),
                        });

                        let value_index = Value::ValueIndex {
                            index: ValueIndex::from(self.arena.len() - 1),
                        };

                        self.allocated += self.size_of_value(&value_index);

                        Ok(value_index)
                    }
                    GarbageCollectedValue::ApartString { value: text } if make_literal => {
                        self.arena.push(GarbageCollectedValue::ApartString {
                            value: text.clone(),
                        });

                        let value_index = Value::ValueIndex {
                            index: ValueIndex::from(self.arena.len() - 1),
                        };

                        self.allocated += self.size_of_value(&value_index);

                        Ok(value_index)
                    }
                    GarbageCollectedValue::ApartVec { length, values } if make_literal => {
                        let length = *length;

                        let values =
                            values
                                .clone()
                                .into_iter()
                                .try_fold(vec![], |mut accum, value| {
                                    self.dereference_value(out, &value, make_literal)
                                        .map(|value| {
                                            accum.push(value);

                                            accum
                                        })
                                })?;

                        self.arena
                            .push(GarbageCollectedValue::ApartVec { length, values });

                        let value_index = Value::ValueIndex {
                            index: ValueIndex::from(self.arena.len() - 1),
                        };

                        self.allocated += self.size_of_value(&value_index);

                        Ok(value_index)
                    }
                    _ => Ok(*value),
                }
            }
            _ => Ok(*value),
        }
    }

    #[allow(dead_code)]
    fn option_some(&mut self, value: Value) -> Value {
        self.arena.push(GarbageCollectedValue::TaggedCompound {
            tag: 1,
            field_count: 1,
            fields: vec![value].into_boxed_slice(),
        });

        let value_index = Value::ValueIndex {
            index: ValueIndex::from(self.arena.len() - 1),
        };

        self.allocated += self.size_of_value(&value_index);

        value_index
    }

    #[allow(dead_code)]
    fn option_none(&mut self) -> Value {
        self.arena.push(GarbageCollectedValue::TaggedCompound {
            tag: 0,
            field_count: 0,
            fields: vec![].into_boxed_slice(),
        });

        let value_index = Value::ValueIndex {
            index: ValueIndex::from(self.arena.len() - 1),
        };

        self.allocated += self.size_of_value(&value_index);

        value_index
    }

    #[allow(dead_code)]
    fn result_ok(&mut self, value: Value) -> Value {
        self.arena.push(GarbageCollectedValue::TaggedCompound {
            tag: 1,
            field_count: 1,
            fields: vec![value].into_boxed_slice(),
        });

        let value_index = Value::ValueIndex {
            index: ValueIndex::from(self.arena.len() - 1),
        };

        self.allocated += self.size_of_value(&value_index);

        value_index
    }

    #[allow(dead_code)]
    fn result_err(&mut self, value: Value) -> Value {
        self.arena.push(GarbageCollectedValue::TaggedCompound {
            tag: 0,
            field_count: 1,
            fields: vec![value].into_boxed_slice(),
        });

        let value_index = Value::ValueIndex {
            index: ValueIndex::from(self.arena.len() - 1),
        };

        self.allocated += self.size_of_value(&value_index);

        value_index
    }

    #[allow(clippy::too_many_lines)]
    fn native_fn_call<O>(
        &mut self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        match native_fn {
            NativeFn::PrintF32 => self.native_fn_print_f32(call_arguments, native_fn, out),
            NativeFn::PrintF64 => self.native_fn_print_f64(call_arguments, native_fn, out),
            NativeFn::PrintString => self.native_fn_print_string(call_arguments, native_fn, out),
            NativeFn::ExtendString => self.native_fn_extend_string(call_arguments, native_fn, out),
            NativeFn::SliceString => self.native_fn_slice_string(call_arguments, native_fn, out),
            NativeFn::LengthString => self.native_fn_length_string(call_arguments, native_fn, out),
            NativeFn::TruncateString => {
                self.native_fn_truncate_string(call_arguments, native_fn, out)
            }
            NativeFn::FloorCharBoundaryString => {
                self.native_fn_floor_char_boundary_string(call_arguments, native_fn, out)
            }
            NativeFn::CeilCharBoundaryString => {
                self.native_fn_ceil_char_boundary_string(call_arguments, native_fn, out)
            }
            NativeFn::Clock => self.native_fn_clock(call_arguments, native_fn, out),
            NativeFn::Sleep => self.native_fn_sleep(call_arguments, native_fn, out),
            NativeFn::NewVec => self.native_fn_new_vec(call_arguments, native_fn, out),
            NativeFn::GetVec => self.native_fn_get_vec(call_arguments, native_fn, out),
            NativeFn::ExtendVec => self.native_fn_extend_vec(call_arguments, native_fn, out),
            NativeFn::SliceVec => self.native_fn_slice_vec(call_arguments, native_fn, out),
            NativeFn::LengthVec => self.native_fn_length_vec(call_arguments, native_fn, out),
            NativeFn::TruncateVec => self.native_fn_truncate_vec(call_arguments, native_fn, out),
            NativeFn::PushVec => self.native_fn_push_vec(call_arguments, native_fn, out),
            NativeFn::PopVec => self.native_fn_pop_vec(call_arguments, native_fn, out),
            NativeFn::LogF64 => self.native_fn_log_f64(call_arguments, native_fn, out),
            NativeFn::FloorF64 => self.native_fn_floor_f64(call_arguments, native_fn, out),
            NativeFn::ToU64F64 => self.native_fn_to_u64_f64(call_arguments, native_fn, out),
            NativeFn::FromU64F64 => self.native_fn_from_u64_f64(call_arguments, native_fn, out),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn values_eq(&self, lhs: &Value, rhs: &Value) -> bool {
        match (lhs, rhs) {
            (Value::U8 { value: lhs }, Value::U8 { value: rhs }) => lhs == rhs,
            (Value::I8 { value: lhs }, Value::I8 { value: rhs }) => lhs == rhs,
            (Value::U16 { value: lhs }, Value::U16 { value: rhs }) => lhs == rhs,
            (Value::I16 { value: lhs }, Value::I16 { value: rhs }) => lhs == rhs,
            (Value::I32 { value: lhs }, Value::I32 { value: rhs }) => lhs == rhs,
            (Value::U64 { value: lhs }, Value::U64 { value: rhs }) => lhs == rhs,
            (Value::I64 { value: lhs }, Value::I64 { value: rhs }) => lhs == rhs,
            (Value::F32 { value: lhs }, Value::F32 { value: rhs }) => lhs == rhs,
            (Value::F64 { value: lhs }, Value::F64 { value: rhs }) => lhs == rhs,
            (Value::Boolean { value: lhs }, Value::Boolean { value: rhs }) => lhs == rhs,
            (Value::Unit, Value::Unit) => true,
            (Value::ApartFn { address: lhs, .. }, Value::ApartFn { address: rhs, .. })
            | (Value::U32 { value: lhs }, Value::U32 { value: rhs }) => lhs == rhs,
            (Value::ApartNativeFn { value: lhs }, Value::ApartNativeFn { value: rhs }) => {
                lhs == rhs
            }
            (Value::ValueIndex { index: lhs }, Value::ValueIndex { index: rhs }) => {
                match (
                    &self.arena[usize::from(*lhs)],
                    &self.arena[usize::from(*rhs)],
                ) {
                    (
                        GarbageCollectedValue::ApartString { value: lhs },
                        GarbageCollectedValue::ApartString { value: rhs },
                    ) => lhs == rhs,
                    (
                        GarbageCollectedValue::TaggedCompound { tag: lhs, .. },
                        GarbageCollectedValue::TaggedCompound { tag: rhs, .. },
                    ) if lhs != rhs => false,
                    (
                        GarbageCollectedValue::Compound {
                            field_count: lhs, ..
                        },
                        GarbageCollectedValue::Compound {
                            field_count: rhs, ..
                        },
                    )
                    | (
                        GarbageCollectedValue::TaggedCompound {
                            field_count: lhs, ..
                        },
                        GarbageCollectedValue::TaggedCompound {
                            field_count: rhs, ..
                        },
                    ) if lhs != rhs => false,
                    (
                        GarbageCollectedValue::Compound { fields: lhs, .. },
                        GarbageCollectedValue::Compound { fields: rhs, .. },
                    )
                    | (
                        GarbageCollectedValue::TaggedCompound { fields: lhs, .. },
                        GarbageCollectedValue::TaggedCompound { fields: rhs, .. },
                    ) => {
                        for (lhs, rhs) in lhs.into_iter().zip(rhs) {
                            if !self.values_eq(lhs, rhs) {
                                return false;
                            }
                        }

                        true
                    }
                    (
                        GarbageCollectedValue::ApartVec { length: lhs, .. },
                        GarbageCollectedValue::ApartVec { length: rhs, .. },
                    ) if lhs != rhs => false,
                    (
                        GarbageCollectedValue::ApartVec { values: lhs, .. },
                        GarbageCollectedValue::ApartVec { values: rhs, .. },
                    ) => {
                        for (lhs, rhs) in lhs.iter().zip(rhs) {
                            if !self.values_eq(lhs, rhs) {
                                return false;
                            }
                        }

                        true
                    }
                    (_, _) => panic!(
                        "(EQUALITY {} {})",
                        self.value_to_string(
                            &Value::ValueIndex { index: *lhs },
                            &mut HashSet::new()
                        ),
                        self.value_to_string(
                            &Value::ValueIndex { index: *rhs },
                            &mut HashSet::new()
                        )
                    ),
                }
            }
            (lhs, rhs) => panic!(
                "(EQUALITY {} {})",
                self.value_to_string(lhs, &mut HashSet::new()),
                self.value_to_string(rhs, &mut HashSet::new())
            ),
        }
    }

    const fn should_gc(&self) -> bool {
        if cfg!(feature = "stress_gc") {
            true
        } else {
            self.allocated >= self.next_gc
        }
    }

    fn gc(&mut self) {
        let (marked, marked_count) = self.mark();

        self.sweep(marked.as_slice(), marked_count);

        self.next_gc = (self.allocated * 2).max(DEFAULT_NEXT_GC);
    }

    fn mark(&self) -> (Vec<Option<ValueIndex>>, usize) {
        let mut marked = vec![const { None }; self.arena.len()];
        let mut marked_count = 0;

        for value_index in &self.literals {
            self.mark_value(
                &mut marked,
                &Value::ValueIndex {
                    index: *value_index,
                },
                &mut marked_count,
            );
        }

        for value in &self.registers {
            self.mark_value(&mut marked, value, &mut marked_count);
        }

        (marked, marked_count)
    }

    fn mark_value(
        &self,
        marked: &mut [Option<ValueIndex>],
        value: &Value,
        marked_count: &mut usize,
    ) {
        if let Value::ValueIndex { index } = value {
            match &self.arena[usize::from(*index)] {
                GarbageCollectedValue::Compound { fields, .. }
                | GarbageCollectedValue::TaggedCompound { fields, .. } => {
                    for field in fields {
                        self.mark_value(marked, field, marked_count);
                    }
                }
                GarbageCollectedValue::ApartVec { values, .. } => {
                    for value in values {
                        self.mark_value(marked, value, marked_count);
                    }
                }
                GarbageCollectedValue::ApartString { .. } => {}
            }

            if marked[usize::from(*index)].is_none() {
                marked[usize::from(*index)] = Some(ValueIndex::from(*marked_count));

                *marked_count += 1;
            }
        }
    }

    fn sweep(&mut self, marked: &[Option<ValueIndex>], marked_count: usize) {
        let mut values = Vec::with_capacity(marked_count);

        for value_index in &self.literals {
            self.retain_value(
                &mut values,
                Value::ValueIndex {
                    index: *value_index,
                },
                marked,
            );
        }

        self.registers = self
            .registers
            .iter()
            .copied()
            .map(|value| self.retain_value(&mut values, value, marked))
            .collect::<Vec<_>>();

        values.sort_by_key(|(index, _)| *index);
        values.dedup_by_key(|(index, _)| *index);

        self.allocated = 0;

        self.arena = values
            .into_iter()
            .fold(vec![], |mut accum, (value_index, value)| {
                self.allocated += self.size_of_value(&Value::ValueIndex { index: value_index });

                accum.push(value);

                accum
            });
    }

    fn retain_value(
        &self,
        replacement_values: &mut Vec<(ValueIndex, GarbageCollectedValue)>,
        value: Value,
        marked: &[Option<ValueIndex>],
    ) -> Value {
        if let Value::ValueIndex { index } = value {
            marked
                .get(usize::from(index))
                .and_then(|marked| *marked)
                .map_or(Value::Unit, |replacement_index| {
                    match &self.arena[usize::from(index)] {
                        GarbageCollectedValue::ApartString { value: text } => {
                            replacement_values.push((
                                replacement_index,
                                GarbageCollectedValue::ApartString {
                                    value: text.clone(),
                                },
                            ));
                        }
                        GarbageCollectedValue::ApartVec { length, values } => {
                            let values = values
                                .iter()
                                .copied()
                                .map(|value| self.retain_value(replacement_values, value, marked))
                                .collect::<Vec<_>>();

                            replacement_values.push((
                                replacement_index,
                                GarbageCollectedValue::ApartVec {
                                    length: *length,
                                    values,
                                },
                            ));
                        }
                        GarbageCollectedValue::Compound {
                            field_count,
                            fields,
                        } => {
                            let fields = fields
                                .iter()
                                .copied()
                                .map(|field| self.retain_value(replacement_values, field, marked))
                                .collect::<Vec<_>>();

                            replacement_values.push((
                                replacement_index,
                                GarbageCollectedValue::Compound {
                                    field_count: *field_count,
                                    fields: fields.into_boxed_slice(),
                                },
                            ));
                        }
                        GarbageCollectedValue::TaggedCompound {
                            tag,
                            field_count,
                            fields,
                        } => {
                            let fields = fields
                                .iter()
                                .copied()
                                .map(|field| self.retain_value(replacement_values, field, marked))
                                .collect::<Vec<_>>();

                            replacement_values.push((
                                replacement_index,
                                GarbageCollectedValue::TaggedCompound {
                                    tag: *tag,
                                    field_count: *field_count,
                                    fields: fields.into_boxed_slice(),
                                },
                            ));
                        }
                    }

                    Value::ValueIndex {
                        index: replacement_index,
                    }
                })
        } else {
            value
        }
    }

    fn size_of_value(&self, value: &Value) -> usize {
        if let Value::ValueIndex { index } = value {
            match &self.arena[usize::from(*index)] {
                GarbageCollectedValue::Compound { fields, .. } => {
                    fields
                        .iter()
                        .fold(0, |accum, field| accum + mem::size_of_val(field))
                        + mem::size_of_val(&self.arena[usize::from(*index)])
                }
                GarbageCollectedValue::TaggedCompound { fields, .. } => {
                    fields
                        .iter()
                        .fold(0, |accum, field| accum + mem::size_of_val(field))
                        + mem::size_of_val(&self.arena[usize::from(*index)])
                }
                GarbageCollectedValue::ApartString { value: text } => {
                    mem::size_of_val(text) + mem::size_of_val(&self.arena[usize::from(*index)])
                }
                GarbageCollectedValue::ApartVec { values, .. } => {
                    values
                        .iter()
                        .fold(0, |accum, value| accum + mem::size_of_val(value))
                        + mem::size_of_val(&self.arena[usize::from(*index)])
                }
            }
        } else {
            mem::size_of_val(value)
        }
    }
}

macro_rules! native_fn_print_debug {
    (
        $name:ident, $value_variant:ident $(,)?
    ) => {
        fn $name<O>(
            &mut self,
            call_arguments: &[$crate::targets::vm::Value],
            native_fn: $crate::middle_ir::NativeFn,
            out: &mut O,
        ) -> Result<$crate::targets::vm::Value, $crate::vm::Interrupt>
        where
            O: Write,
        {
            if let $crate::targets::vm::Value::$value_variant { value: value } = &call_arguments[0]
            {
                writeln!(out, "{value:?}").expect("failed to write to output");

                Ok($crate::targets::vm::Value::Unit)
            } else {
                self.native_fn_invalid_arguments(call_arguments, native_fn, out)
            }
        }
    };
}

impl Vm {
    fn native_fn_invalid_arguments<O>(
        &self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        writeln!(
            out,
            "invalid arguments: {native_fn:?}({})",
            call_arguments
                .iter()
                .map(|argument| { self.value_to_string(argument, &mut HashSet::new()) })
                .collect::<Vec<_>>()
                .join(", ")
        )
        .map_err(|_| Interrupt::Panic)
        .and_then(|()| self.panic(out, None))
        .map(|()| Value::Never)
    }

    native_fn_print_debug!(native_fn_print_f32, F32);
    native_fn_print_debug!(native_fn_print_f64, F64);

    fn native_fn_print_string<O>(
        &self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        if let Value::ValueIndex { index: value_index } = &call_arguments[0]
            && let Some(GarbageCollectedValue::ApartString { value }) =
                self.arena.get(usize::from(*value_index))
        {
            writeln!(out, "{value}").expect("failed to write to output");

            Ok(Value::Unit)
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_extend_string<O>(
        &mut self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        if let Value::ValueIndex { index: target } = &call_arguments[0]
            && let Value::ValueIndex { index: extension } = &call_arguments[1]
            && let Some(GarbageCollectedValue::ApartString { value: extension }) =
                self.arena.get(usize::from(*extension)).cloned()
            && let Some(GarbageCollectedValue::ApartString { value: target }) =
                self.arena.get_mut(usize::from(*target))
        {
            target.push_str(extension.as_str());

            Ok(Value::Unit)
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_slice_string<O>(
        &mut self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        if let Value::ValueIndex { index } = &call_arguments[0]
            && let Some(GarbageCollectedValue::ApartString { value }) =
                self.arena.get(usize::from(*index))
            && let Value::U64 { value: start } = &call_arguments[1]
            && let Value::U64 { value: end } = &call_arguments[2]
            && let Ok(start) = usize::try_from(*start)
            && let Ok(end) = usize::try_from(*end)
        {
            if let Some(value) = value.get(start..end) {
                self.arena.push(GarbageCollectedValue::ApartString {
                    value: value.to_string(),
                });

                let value_index = Value::ValueIndex {
                    index: ValueIndex::from(self.arena.len() - 1),
                };

                self.allocated += self.size_of_value(&value_index);

                Ok(self.option_some(value_index))
            } else {
                Ok(self.option_none())
            }
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_length_string<O>(
        &self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        if let Value::ValueIndex { index } = &call_arguments[0]
            && let Some(GarbageCollectedValue::ApartString { value }) =
                self.arena.get(usize::from(*index))
            && let Ok(length) = u64::try_from(value.len())
        {
            Ok(Value::U64 { value: length })
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_truncate_string<O>(
        &mut self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        if let Value::ValueIndex { index } = &call_arguments[0]
            && let Some(GarbageCollectedValue::ApartString { value }) =
                self.arena.get_mut(usize::from(*index))
            && let Value::U64 { value: end } = &call_arguments[1]
            && let Ok(end) = usize::try_from(*end)
        {
            value.truncate(end);

            Ok(Value::Unit)
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_floor_char_boundary_string<O>(
        &self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        if let Value::ValueIndex { index } = &call_arguments[0]
            && let Some(GarbageCollectedValue::ApartString { value }) =
                self.arena.get(usize::from(*index))
            && let Value::U64 { value: boundary } = &call_arguments[1]
            && let Ok(boundary) = usize::try_from(*boundary)
            && let Ok(boundary) = u64::try_from(value.floor_char_boundary(boundary))
        {
            Ok(Value::U64 { value: boundary })
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_ceil_char_boundary_string<O>(
        &self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        if let Value::ValueIndex { index } = &call_arguments[0]
            && let Some(GarbageCollectedValue::ApartString { value }) =
                self.arena.get(usize::from(*index))
            && let Value::U64 { value: boundary } = &call_arguments[1]
            && let Ok(boundary) = usize::try_from(*boundary)
            && let Ok(boundary) = u64::try_from(value.ceil_char_boundary(boundary))
        {
            Ok(Value::U64 { value: boundary })
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    #[allow(clippy::unnecessary_wraps, clippy::unused_self)]
    fn native_fn_clock<O>(&self, _: &[Value], _: NativeFn, _: &mut O) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        Ok(Value::F64 {
            value: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time somehow went backwards")
                .as_secs_f64()
                * 1000.0,
        })
    }

    #[allow(clippy::unnecessary_wraps, clippy::unused_self)]
    fn native_fn_sleep<O>(
        &self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        if let Value::U64 { value: millis } = &call_arguments[0] {
            std::thread::sleep(std::time::Duration::from_millis(*millis));

            Ok(Value::Unit)
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_log_f64<O>(
        &self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        if let Value::F64 { value } = &call_arguments[0] {
            Ok(Value::F64 {
                value: f64::log(*value, core::f64::consts::E),
            })
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_floor_f64<O>(
        &self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        if let Value::F64 { value } = &call_arguments[0] {
            Ok(Value::F64 {
                value: f64::floor(*value),
            })
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_to_u64_f64<O>(
        &self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        if let Value::F64 { value } = &call_arguments[0] {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let value = *value as u64;

            Ok(Value::U64 { value })
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_from_u64_f64<O>(
        &self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        if let Value::U64 { value } = &call_arguments[0] {
            #[allow(clippy::cast_precision_loss)]
            let value = *value as f64;

            Ok(Value::F64 { value })
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    fn native_fn_new_vec<O>(
        &mut self,
        _: &[Value],
        _: NativeFn,
        _: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        self.arena.push(GarbageCollectedValue::ApartVec {
            length: 0,
            values: vec![],
        });

        Ok(Value::ValueIndex {
            index: ValueIndex::from(self.arena.len() - 1),
        })
    }

    fn native_fn_get_vec<O>(
        &mut self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        if let Value::ValueIndex { index } = &call_arguments[0]
            && let Some(GarbageCollectedValue::ApartVec { values, .. }) =
                self.arena.get(usize::from(*index))
            && let Value::U64 { value: index } = &call_arguments[1]
            && let Ok(index) = usize::try_from(*index)
        {
            if let Some(value) = values.get(index).copied() {
                Ok(self.option_some(value))
            } else {
                Ok(self.option_none())
            }
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_extend_vec<O>(
        &mut self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        if let Value::ValueIndex { index: target } = &call_arguments[0]
            && let Value::ValueIndex { index: extension } = &call_arguments[1]
            && let Some(GarbageCollectedValue::ApartVec {
                length: extension_length,
                values: extension,
            }) = self.arena.get(usize::from(*extension)).cloned()
            && let Some(GarbageCollectedValue::ApartVec {
                length: target_length,
                values: target,
            }) = self.arena.get_mut(usize::from(*target))
        {
            for value in &extension {
                self.allocated += mem::size_of_val(value);
            }

            *target_length += extension_length;
            target.extend(extension);

            Ok(Value::Unit)
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_slice_vec<O>(
        &mut self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        if let Value::ValueIndex { index } = &call_arguments[0]
            && let Some(GarbageCollectedValue::ApartVec { values, .. }) =
                self.arena.get(usize::from(*index))
            && let Value::U64 { value: start } = &call_arguments[1]
            && let Value::U64 { value: end } = &call_arguments[2]
            && let Ok(start) = usize::try_from(*start)
            && let Ok(end) = usize::try_from(*end)
        {
            if let Some(value) = values.get(start..end) {
                self.arena.push(GarbageCollectedValue::ApartVec {
                    length: u64::try_from(end - start).unwrap_or_else(|_| {
                        panic!("since each operand is a valid u64, this should be fine")
                    }),
                    values: value.to_vec(),
                });

                let value_index = Value::ValueIndex {
                    index: ValueIndex::from(self.arena.len() - 1),
                };

                self.allocated += self.size_of_value(&value_index);

                Ok(self.option_some(value_index))
            } else {
                Ok(self.option_none())
            }
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_length_vec<O>(
        &self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        if let Value::ValueIndex { index } = &call_arguments[0]
            && let Some(GarbageCollectedValue::ApartVec { length, .. }) =
                self.arena.get(usize::from(*index))
        {
            Ok(Value::U64 { value: *length })
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_truncate_vec<O>(
        &mut self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        if let Value::ValueIndex { index } = &call_arguments[0]
            && let Some(GarbageCollectedValue::ApartVec { length, values }) =
                self.arena.get_mut(usize::from(*index))
            && let Value::U64 { value: end } = &call_arguments[1]
            && let Ok(end) = usize::try_from(*end)
        {
            *length = u64::try_from(end)
                .unwrap_or_else(|_| panic!("since this was originally a u64, it should be fine"));

            for value in values.drain(end..) {
                self.allocated -= mem::size_of_val(&value);
            }

            Ok(Value::Unit)
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_push_vec<O>(
        &mut self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        if let Value::ValueIndex { index: value_index } = &call_arguments[0]
            && let Some(GarbageCollectedValue::ApartVec { length, values }) =
                self.arena.get_mut(usize::from(*value_index))
        {
            *length += 1;

            values.push(call_arguments[1]);

            self.allocated += mem::size_of_val(&call_arguments[1]);

            Ok(Value::Unit)
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_pop_vec<O>(
        &mut self,
        call_arguments: &[Value],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<Value, Interrupt>
    where
        O: Write,
    {
        if let Value::ValueIndex { index: value_index } = &call_arguments[0]
            && let Some(GarbageCollectedValue::ApartVec { length, values }) =
                self.arena.get_mut(usize::from(*value_index))
        {
            if let Some(value) = values.pop() {
                *length -= 1;

                self.allocated -= mem::size_of_val(&value);

                Ok(self.option_some(value))
            } else {
                Ok(self.option_none())
            }
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }
}
