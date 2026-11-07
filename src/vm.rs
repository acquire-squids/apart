use crate::{
    low_ir::{FnLabel, NativeFn, Register},
    targets::vm::{CopyableValue, Instruction, Instructive, Value, ValueIndex},
};

use std::{
    collections::HashSet,
    io::{BufRead, Write},
    mem,
};

struct CallFrame {
    fp: usize,
    from: u32,
    to: Register,
}

struct Vm {
    registers: Vec<CopyableValue>,
    values: Vec<Value>,
    allocated: usize,
    next_gc: usize,
    call_frames: Vec<CallFrame>,
    function_labels: Vec<FnLabel>,
    constants: Vec<ValueIndex>,
    ip: usize,
    sp: Option<usize>,
    fp: usize,
}

const DEFAULT_NEXT_GC: usize = 1_000_000;

impl Instructive for Vm {
    fn ip_mut(&mut self) -> &mut usize {
        &mut self.ip
    }

    fn constants_mut(&mut self) -> &mut Vec<ValueIndex> {
        &mut self.constants
    }

    fn values_mut(&mut self) -> &mut Vec<Value> {
        &mut self.values
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
        const { CopyableValue::Never };
        bytecode[..2]
            .as_array::<2>()
            .map(|array| u16::from_le_bytes(*array))
            .map(usize::from)
            .expect("maximum registers is unknown")
    ];

    let mut vm = Vm {
        registers,
        values: vec![],
        allocated: 0,
        next_gc: DEFAULT_NEXT_GC,
        call_frames: vec![CallFrame {
            fp: 0,
            from: 0,
            to: Register(0),
        }],
        function_labels: vec![],
        constants: vec![],
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
            ($crate::targets::vm::CopyableValue::U8(lhs), $crate::targets::vm::CopyableValue::U8(rhs)) => {
                $self.assign($to, $crate::targets::vm::CopyableValue::U8(lhs $op rhs));

                Ok(())
            }
            ($crate::targets::vm::CopyableValue::I8(lhs), $crate::targets::vm::CopyableValue::I8(rhs)) => {
                $self.assign($to, $crate::targets::vm::CopyableValue::I8(lhs $op rhs));

                Ok(())
            }
            ($crate::targets::vm::CopyableValue::U16(lhs), $crate::targets::vm::CopyableValue::U16(rhs)) => {
                $self.assign($to, $crate::targets::vm::CopyableValue::U16(lhs $op rhs));

                Ok(())
            }
            ($crate::targets::vm::CopyableValue::I16(lhs), $crate::targets::vm::CopyableValue::I16(rhs)) => {
                $self.assign($to, $crate::targets::vm::CopyableValue::I16(lhs $op rhs));

                Ok(())
            }
            ($crate::targets::vm::CopyableValue::U32(lhs), $crate::targets::vm::CopyableValue::U32(rhs)) => {
                $self.assign($to, $crate::targets::vm::CopyableValue::U32(lhs $op rhs));

                Ok(())
            }
            ($crate::targets::vm::CopyableValue::I32(lhs), $crate::targets::vm::CopyableValue::I32(rhs)) => {
                $self.assign($to, $crate::targets::vm::CopyableValue::I32(lhs $op rhs));

                Ok(())
            }
            ($crate::targets::vm::CopyableValue::U64(lhs), $crate::targets::vm::CopyableValue::U64(rhs)) => {
                $self.assign($to, $crate::targets::vm::CopyableValue::U64(lhs $op rhs));

                Ok(())
            }
            ($crate::targets::vm::CopyableValue::I64(lhs), $crate::targets::vm::CopyableValue::I64(rhs)) => {
                $self.assign($to, $crate::targets::vm::CopyableValue::I64(lhs $op rhs));

                Ok(())
            }
            ($crate::targets::vm::CopyableValue::F32(lhs), $crate::targets::vm::CopyableValue::F32(rhs)) => {
                $self.assign($to, $crate::targets::vm::CopyableValue::F32(lhs $op rhs));

                Ok(())
            }
            ($crate::targets::vm::CopyableValue::F64(lhs), $crate::targets::vm::CopyableValue::F64(rhs)) => {
                $self.assign($to, $crate::targets::vm::CopyableValue::F64(lhs $op rhs));

                Ok(())
            }
            (
                $crate::targets::vm::CopyableValue::U8(_)
                | $crate::targets::vm::CopyableValue::I8(_)
                | $crate::targets::vm::CopyableValue::U16(_)
                | $crate::targets::vm::CopyableValue::I16(_)
                | $crate::targets::vm::CopyableValue::U32(_)
                | $crate::targets::vm::CopyableValue::I32(_)
                | $crate::targets::vm::CopyableValue::U64(_)
                | $crate::targets::vm::CopyableValue::I64(_)
                | $crate::targets::vm::CopyableValue::F32(_)
                | $crate::targets::vm::CopyableValue::F64(_)
                | $crate::targets::vm::CopyableValue::Boolean(_)
                | $crate::targets::vm::CopyableValue::Unit
                | $crate::targets::vm::CopyableValue::Fn { .. }
                | $crate::targets::vm::CopyableValue::NativeFn(_)
                | $crate::targets::vm::CopyableValue::ValueIndex(_)
                | $crate::targets::vm::CopyableValue::Never
                | $crate::targets::vm::CopyableValue::Register(_),
                $crate::targets::vm::CopyableValue::U8(_)
                | $crate::targets::vm::CopyableValue::I8(_)
                | $crate::targets::vm::CopyableValue::U16(_)
                | $crate::targets::vm::CopyableValue::I16(_)
                | $crate::targets::vm::CopyableValue::U32(_)
                | $crate::targets::vm::CopyableValue::I32(_)
                | $crate::targets::vm::CopyableValue::U64(_)
                | $crate::targets::vm::CopyableValue::I64(_)
                | $crate::targets::vm::CopyableValue::F32(_)
                | $crate::targets::vm::CopyableValue::F64(_)
                | $crate::targets::vm::CopyableValue::Boolean(_)
                | $crate::targets::vm::CopyableValue::Unit
                | $crate::targets::vm::CopyableValue::Fn { .. }
                | $crate::targets::vm::CopyableValue::NativeFn(_)
                | $crate::targets::vm::CopyableValue::ValueIndex(_)
                | $crate::targets::vm::CopyableValue::Never
                | $crate::targets::vm::CopyableValue::Register(_)
            ) => {
                writeln!(
                    $out,
                    "invalid operands for {} (lhs is {}, rhs is {})",
                    stringify!($op),
                    $self.value_to_string($lhs, &mut std::collections::HashSet::new()),
                    $self.value_to_string($rhs, &mut std::collections::HashSet::new())
                )
                .map_err(|_| $crate::vm::Interrupt::Panic)?;

                $self.panic($out, None)
            }
        }
    }};
}

macro_rules! binary_op_comparison {
    (
        $self:ident, $out:ident, $to:ident, $op:tt, $lhs:expr, $rhs:expr $(,)?
    ) => {{
        match ($lhs, $rhs) {
            ($crate::targets::vm::CopyableValue::U8(lhs), $crate::targets::vm::CopyableValue::U8(rhs)) => {
                $self.assign($to, $crate::targets::vm::CopyableValue::Boolean(lhs $op rhs));

                Ok(())
            }
            ($crate::targets::vm::CopyableValue::I8(lhs), $crate::targets::vm::CopyableValue::I8(rhs)) => {
                $self.assign($to, $crate::targets::vm::CopyableValue::Boolean(lhs $op rhs));

                Ok(())
            }
            ($crate::targets::vm::CopyableValue::U16(lhs), $crate::targets::vm::CopyableValue::U16(rhs)) => {
                $self.assign($to, $crate::targets::vm::CopyableValue::Boolean(lhs $op rhs));

                Ok(())
            }
            ($crate::targets::vm::CopyableValue::I16(lhs), $crate::targets::vm::CopyableValue::I16(rhs)) => {
                $self.assign($to, $crate::targets::vm::CopyableValue::Boolean(lhs $op rhs));

                Ok(())
            }
            ($crate::targets::vm::CopyableValue::U32(lhs), $crate::targets::vm::CopyableValue::U32(rhs)) => {
                $self.assign($to, $crate::targets::vm::CopyableValue::Boolean(lhs $op rhs));

                Ok(())
            }
            ($crate::targets::vm::CopyableValue::I32(lhs), $crate::targets::vm::CopyableValue::I32(rhs)) => {
                $self.assign($to, $crate::targets::vm::CopyableValue::Boolean(lhs $op rhs));

                Ok(())
            }
            ($crate::targets::vm::CopyableValue::U64(lhs), $crate::targets::vm::CopyableValue::U64(rhs)) => {
                $self.assign($to, $crate::targets::vm::CopyableValue::Boolean(lhs $op rhs));

                Ok(())
            }
            ($crate::targets::vm::CopyableValue::I64(lhs), $crate::targets::vm::CopyableValue::I64(rhs)) => {
                $self.assign($to, $crate::targets::vm::CopyableValue::Boolean(lhs $op rhs));

                Ok(())
            }
            ($crate::targets::vm::CopyableValue::F32(lhs), $crate::targets::vm::CopyableValue::F32(rhs)) => {
                $self.assign($to, $crate::targets::vm::CopyableValue::Boolean(lhs $op rhs));

                Ok(())
            }
            ($crate::targets::vm::CopyableValue::F64(lhs), $crate::targets::vm::CopyableValue::F64(rhs)) => {
                $self.assign($to, $crate::targets::vm::CopyableValue::Boolean(lhs $op rhs));

                Ok(())
            }
            (
                $crate::targets::vm::CopyableValue::U8(_)
                | $crate::targets::vm::CopyableValue::I8(_)
                | $crate::targets::vm::CopyableValue::U16(_)
                | $crate::targets::vm::CopyableValue::I16(_)
                | $crate::targets::vm::CopyableValue::U32(_)
                | $crate::targets::vm::CopyableValue::I32(_)
                | $crate::targets::vm::CopyableValue::U64(_)
                | $crate::targets::vm::CopyableValue::I64(_)
                | $crate::targets::vm::CopyableValue::F32(_)
                | $crate::targets::vm::CopyableValue::F64(_)
                | $crate::targets::vm::CopyableValue::Boolean(_)
                | $crate::targets::vm::CopyableValue::Unit
                | $crate::targets::vm::CopyableValue::Fn { .. }
                | $crate::targets::vm::CopyableValue::NativeFn(_)
                | $crate::targets::vm::CopyableValue::ValueIndex(_)
                | $crate::targets::vm::CopyableValue::Never
                | $crate::targets::vm::CopyableValue::Register(_),
                $crate::targets::vm::CopyableValue::U8(_)
                | $crate::targets::vm::CopyableValue::I8(_)
                | $crate::targets::vm::CopyableValue::U16(_)
                | $crate::targets::vm::CopyableValue::I16(_)
                | $crate::targets::vm::CopyableValue::U32(_)
                | $crate::targets::vm::CopyableValue::I32(_)
                | $crate::targets::vm::CopyableValue::U64(_)
                | $crate::targets::vm::CopyableValue::I64(_)
                | $crate::targets::vm::CopyableValue::F32(_)
                | $crate::targets::vm::CopyableValue::F64(_)
                | $crate::targets::vm::CopyableValue::Boolean(_)
                | $crate::targets::vm::CopyableValue::Unit
                | $crate::targets::vm::CopyableValue::Fn { .. }
                | $crate::targets::vm::CopyableValue::NativeFn(_)
                | $crate::targets::vm::CopyableValue::ValueIndex(_)
                | $crate::targets::vm::CopyableValue::Never
                | $crate::targets::vm::CopyableValue::Register(_)
            ) => {
                writeln!(
                    $out,
                    "invalid operands for {} (lhs is {}, rhs is {})",
                    stringify!($op),
                    $self.value_to_string($lhs, &mut std::collections::HashSet::new()),
                    $self.value_to_string($rhs, &mut std::collections::HashSet::new())
                )
                .map_err(|_| $crate::vm::Interrupt::Panic)?;

                $self.panic($out, None)
            }
        }
    }};
}

impl Vm {
    fn value_to_string(&self, value: CopyableValue, seen: &mut HashSet<ValueIndex>) -> String {
        if let CopyableValue::ValueIndex(value_index) = value
            && let Some(value) = self.values.get(usize::from(value_index))
        {
            if !seen.insert(value_index) {
                return format!("{value_index:?}");
            }

            match value {
                Value::Compound(values) => {
                    let mut buffer = "Compound(".to_string();

                    if let Some(first_value) = values.first() {
                        buffer.push_str(self.value_to_string(*first_value, seen).as_str());

                        for value in values.iter().skip(1) {
                            buffer.push_str(", ");

                            buffer.push_str(self.value_to_string(*value, seen).as_str());
                        }
                    }

                    buffer.push(')');

                    buffer
                }
                Value::TaggedCompound {
                    tag,
                    fields: values,
                } => {
                    let mut buffer = format!("TaggedCompound {{ tag: {tag}, fields: [");

                    if let Some(first_value) = values.first() {
                        buffer.push_str(self.value_to_string(*first_value, seen).as_str());

                        for value in values.iter().skip(1) {
                            buffer.push_str(", ");

                            buffer.push_str(self.value_to_string(*value, seen).as_str());
                        }
                    }

                    buffer.push_str("] }");

                    buffer
                }
                Value::String(text) => {
                    format!("String({text:?})")
                }
                Value::Vec(values) => {
                    let mut buffer = "Vec([".to_string();

                    if let Some(first_value) = values.first() {
                        buffer.push_str(self.value_to_string(*first_value, seen).as_str());

                        for value in values.iter().skip(1) {
                            buffer.push_str(", ");

                            buffer.push_str(self.value_to_string(*value, seen).as_str());
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
                self.value_to_string(*first_value, &mut HashSet::new())
                    .as_str(),
            );

            for value in self.registers.iter().skip(self.fp + 1) {
                buffer.push_str(", ");

                buffer.push_str(self.value_to_string(*value, &mut HashSet::new()).as_str());
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

                self.constants.clear();

                self.gc();

                assert_eq!(self.allocated, 0, "{} BYTES LEAKED", self.allocated);
            })?;

            if self.call_frames.is_empty() {
                break;
            }
        }

        self.registers.clear();

        self.constants.clear();

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
            Instruction::FnLabel { fn_label } => {
                self.function_labels.push(FnLabel::new(
                    fn_label.file_name().to_string(),
                    fn_label.function_label().to_string(),
                ));

                Ok(())
            }
            Instruction::NotRegister { to, operand } => {
                let operand = self.get_register(*operand);

                self.op_logical_not(out, *to, operand)
            }
            Instruction::NotValue { to, operand } => {
                let operand = self.dereference_value(*operand, true);

                self.op_logical_not(out, *to, operand)
            }
            Instruction::NegateRegister { to, operand } => {
                let operand = self.get_register(*operand);

                self.op_arithmetic_negate(out, *to, operand)
            }
            Instruction::NegateValue { to, operand } => {
                let operand = self.dereference_value(*operand, true);

                self.op_arithmetic_negate(out, *to, operand)
            }
            Instruction::MultiplyRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_arithmetic_multiply(out, *to, lhs, rhs)
            }
            Instruction::MultiplyRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.op_arithmetic_multiply(out, *to, lhs, rhs)
            }
            Instruction::MultiplyValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.op_arithmetic_multiply(out, *to, lhs, rhs)
            }
            Instruction::MultiplyValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.op_arithmetic_multiply(out, *to, lhs, rhs)
            }
            Instruction::DivideRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_arithmetic_divide(out, *to, lhs, rhs)
            }
            Instruction::DivideRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.op_arithmetic_divide(out, *to, lhs, rhs)
            }
            Instruction::DivideValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.op_arithmetic_divide(out, *to, lhs, rhs)
            }
            Instruction::DivideValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.op_arithmetic_divide(out, *to, lhs, rhs)
            }
            Instruction::RemainderRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_arithmetic_remainder(out, *to, lhs, rhs)
            }
            Instruction::RemainderRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.op_arithmetic_remainder(out, *to, lhs, rhs)
            }
            Instruction::RemainderValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.op_arithmetic_remainder(out, *to, lhs, rhs)
            }
            Instruction::RemainderValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.op_arithmetic_remainder(out, *to, lhs, rhs)
            }
            Instruction::AddRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_arithmetic_add(out, *to, lhs, rhs)
            }
            Instruction::AddRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.op_arithmetic_add(out, *to, lhs, rhs)
            }
            Instruction::AddValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.op_arithmetic_add(out, *to, lhs, rhs)
            }
            Instruction::AddValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.op_arithmetic_add(out, *to, lhs, rhs)
            }
            Instruction::SubtractRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_arithmetic_subtract(out, *to, lhs, rhs)
            }
            Instruction::SubtractRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.op_arithmetic_subtract(out, *to, lhs, rhs)
            }
            Instruction::SubtractValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.op_arithmetic_subtract(out, *to, lhs, rhs)
            }
            Instruction::SubtractValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.op_arithmetic_subtract(out, *to, lhs, rhs)
            }
            Instruction::LessRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_comparison_less(out, *to, lhs, rhs)
            }
            Instruction::LessRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.op_comparison_less(out, *to, lhs, rhs)
            }
            Instruction::LessValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.op_comparison_less(out, *to, lhs, rhs)
            }
            Instruction::LessValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.op_comparison_less(out, *to, lhs, rhs)
            }
            Instruction::GreaterRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_comparison_greater(out, *to, lhs, rhs)
            }
            Instruction::GreaterRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.op_comparison_greater(out, *to, lhs, rhs)
            }
            Instruction::GreaterValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.op_comparison_greater(out, *to, lhs, rhs)
            }
            Instruction::GreaterValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.op_comparison_greater(out, *to, lhs, rhs)
            }
            Instruction::LessOrEqualRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_comparison_less_or_equal(out, *to, lhs, rhs)
            }
            Instruction::LessOrEqualRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.op_comparison_less_or_equal(out, *to, lhs, rhs)
            }
            Instruction::LessOrEqualValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.op_comparison_less_or_equal(out, *to, lhs, rhs)
            }
            Instruction::LessOrEqualValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.op_comparison_less_or_equal(out, *to, lhs, rhs)
            }
            Instruction::GreaterOrEqualRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_comparison_greater_or_equal(out, *to, lhs, rhs)
            }
            Instruction::GreaterOrEqualRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.op_comparison_greater_or_equal(out, *to, lhs, rhs)
            }
            Instruction::GreaterOrEqualValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.op_comparison_greater_or_equal(out, *to, lhs, rhs)
            }
            Instruction::GreaterOrEqualValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.op_comparison_greater_or_equal(out, *to, lhs, rhs)
            }
            Instruction::EqualRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_comparison_equal(out, *to, lhs, rhs)
            }
            Instruction::EqualRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.op_comparison_equal(out, *to, lhs, rhs)
            }
            Instruction::EqualValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.op_comparison_equal(out, *to, lhs, rhs)
            }
            Instruction::EqualValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.op_comparison_equal(out, *to, lhs, rhs)
            }
            Instruction::NotEqualRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_comparison_not_equal(out, *to, lhs, rhs)
            }
            Instruction::NotEqualRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.op_comparison_not_equal(out, *to, lhs, rhs)
            }
            Instruction::NotEqualValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.op_comparison_not_equal(out, *to, lhs, rhs)
            }
            Instruction::NotEqualValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.op_comparison_not_equal(out, *to, lhs, rhs)
            }
            Instruction::AndRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_logical_and(out, *to, lhs, rhs)
            }
            Instruction::AndRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.op_logical_and(out, *to, lhs, rhs)
            }
            Instruction::AndValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.op_logical_and(out, *to, lhs, rhs)
            }
            Instruction::AndValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.op_logical_and(out, *to, lhs, rhs)
            }
            Instruction::OrRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.op_logical_or(out, *to, lhs, rhs)
            }
            Instruction::OrRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.op_logical_or(out, *to, lhs, rhs)
            }
            Instruction::OrValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.op_logical_or(out, *to, lhs, rhs)
            }
            Instruction::OrValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.op_logical_or(out, *to, lhs, rhs)
            }
            Instruction::AssignRegister { to, operand } => {
                let operand = self.get_register(*operand);

                self.op_assign(out, *to, operand)
            }
            Instruction::AssignValue { to, operand } => {
                let operand = self.dereference_value(*operand, true);

                self.op_assign(out, *to, operand)
            }
            Instruction::PushRegister { operand } => {
                let operand = self.get_register(*operand);

                self.op_push(out, operand)
            }
            Instruction::PushValue { operand } => {
                let operand = self.dereference_value(*operand, true);

                self.op_push(out, operand)
            }
            Instruction::AccessRegister { to, index, of } => {
                let of = self.get_register(*of);

                self.op_access(out, *to, *index, of)
            }
            Instruction::AccessValue { to, index, of } => {
                let of = self.dereference_value(*of, true);

                self.op_access(out, *to, *index, of)
            }
            Instruction::AccessAssignRegisterRegister { to, index, value } => {
                let to = self.get_register(*to);
                let value = self.get_register(*value);

                self.op_access_assign(out, to, *index, value)
            }
            Instruction::AccessAssignRegisterValue { to, index, value } => {
                let to = self.get_register(*to);
                let value = self.dereference_value(*value, true);

                self.op_access_assign(out, to, *index, value)
            }
            Instruction::AccessAssignValueRegister { to, index, value } => {
                let to = self.dereference_value(*to, true);
                let value = self.get_register(*value);

                self.op_access_assign(out, to, *index, value)
            }
            Instruction::AccessAssignValueValue { to, index, value } => {
                let to = self.dereference_value(*to, true);
                let value = self.dereference_value(*value, true);

                self.op_access_assign(out, to, *index, value)
            }
            Instruction::GetTagRegister { to, of } => {
                let of = self.get_register(*of);

                self.op_get_tag(out, *to, of)
            }
            Instruction::GetTagValue { to, of } => {
                let of = self.dereference_value(*of, true);

                self.op_get_tag(out, *to, of)
            }
            Instruction::CallRegister { to, callee, .. } => {
                let callee = self.get_register(*callee);

                self.op_call(out, *to, callee)
            }
            Instruction::CallValue { to, callee, .. } => {
                let callee = self.dereference_value(*callee, true);

                self.op_call(out, *to, callee)
            }
            Instruction::Jump { destination } => self.op_jump(out, *destination),
            Instruction::BranchRegister {
                condition,
                when_true,
                otherwise,
            } => {
                let condition = self.get_register(*condition);

                self.op_branch(out, condition, *when_true, *otherwise)
            }
            Instruction::BranchValue {
                condition,
                when_true,
                otherwise,
            } => {
                let condition = self.dereference_value(*condition, true);

                self.op_branch(out, condition, *when_true, *otherwise)
            }
            Instruction::ReturnRegister { value } => {
                let value = self.get_register(*value);

                self.op_return(out, value)
            }
            Instruction::ReturnValue { value } => {
                let value = self.dereference_value(*value, true);

                self.op_return(out, value)
            }
            Instruction::I64
            | Instruction::F64
            | Instruction::Boolean
            | Instruction::Unit
            | Instruction::Fn
            | Instruction::NativeFn
            | Instruction::Compound
            | Instruction::TaggedCompound
            | Instruction::U8
            | Instruction::I8
            | Instruction::U16
            | Instruction::I16
            | Instruction::U32
            | Instruction::I32
            | Instruction::U64
            | Instruction::F32
            | Instruction::String
            | Instruction::Vec
            | Instruction::Register
            | Instruction::Never => {
                panic!("got a value where an instruction was expected")
            }
        }
    }

    fn op_logical_not<O>(
        &mut self,
        out: &mut O,
        to: Register,
        operand: CopyableValue,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        match operand {
            CopyableValue::Boolean(value) => {
                self.assign(to, CopyableValue::Boolean(!value));

                Ok(())
            }
            CopyableValue::U8(_)
            | CopyableValue::I8(_)
            | CopyableValue::U16(_)
            | CopyableValue::I16(_)
            | CopyableValue::U32(_)
            | CopyableValue::I32(_)
            | CopyableValue::U64(_)
            | CopyableValue::I64(_)
            | CopyableValue::F32(_)
            | CopyableValue::F64(_)
            | CopyableValue::Unit
            | CopyableValue::Fn { .. }
            | CopyableValue::NativeFn(_)
            | CopyableValue::ValueIndex(_)
            | CopyableValue::Never
            | CopyableValue::Register(_) => {
                writeln!(
                    out,
                    "invalid operand for logical not (got {})",
                    self.value_to_string(operand, &mut HashSet::new())
                )
                .map_err(|_| Interrupt::Panic)?;

                self.panic(out, None)
            }
        }
    }

    fn op_arithmetic_negate<O>(
        &mut self,
        out: &mut O,
        to: Register,
        operand: CopyableValue,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        match operand {
            CopyableValue::I8(value) => {
                self.assign(to, CopyableValue::I8(-value));

                Ok(())
            }
            CopyableValue::I16(value) => {
                self.assign(to, CopyableValue::I16(-value));

                Ok(())
            }
            CopyableValue::I32(value) => {
                self.assign(to, CopyableValue::I32(-value));

                Ok(())
            }
            CopyableValue::I64(value) => {
                self.assign(to, CopyableValue::I64(-value));

                Ok(())
            }
            CopyableValue::F32(value) => {
                self.assign(to, CopyableValue::F32(-value));

                Ok(())
            }
            CopyableValue::F64(value) => {
                self.assign(to, CopyableValue::F64(-value));

                Ok(())
            }
            CopyableValue::U8(_)
            | CopyableValue::U16(_)
            | CopyableValue::U32(_)
            | CopyableValue::U64(_)
            | CopyableValue::Boolean(_)
            | CopyableValue::Unit
            | CopyableValue::Fn { .. }
            | CopyableValue::NativeFn(_)
            | CopyableValue::ValueIndex(_)
            | CopyableValue::Never
            | CopyableValue::Register(_) => {
                writeln!(
                    out,
                    "invalid operand for arithmetic negate (got {})",
                    self.value_to_string(operand, &mut HashSet::new())
                )
                .map_err(|_| Interrupt::Panic)?;

                self.panic(out, None)
            }
        }
    }

    fn op_arithmetic_multiply<O>(
        &mut self,
        out: &mut O,
        to: Register,
        lhs: CopyableValue,
        rhs: CopyableValue,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        binary_op_arithmetic!(self, out, to, *, lhs, rhs)
    }

    fn op_arithmetic_divide<O>(
        &mut self,
        out: &mut O,
        to: Register,
        lhs: CopyableValue,
        rhs: CopyableValue,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        binary_op_arithmetic!(self, out, to, /, lhs, rhs)
    }

    fn op_arithmetic_remainder<O>(
        &mut self,
        out: &mut O,
        to: Register,
        lhs: CopyableValue,
        rhs: CopyableValue,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        binary_op_arithmetic!(self, out, to, %, lhs, rhs)
    }

    fn op_arithmetic_add<O>(
        &mut self,
        out: &mut O,
        to: Register,
        lhs: CopyableValue,
        rhs: CopyableValue,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        binary_op_arithmetic!(self, out, to, +, lhs, rhs)
    }

    fn op_arithmetic_subtract<O>(
        &mut self,
        out: &mut O,
        to: Register,
        lhs: CopyableValue,
        rhs: CopyableValue,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        binary_op_arithmetic!(self, out, to, -, lhs, rhs)
    }

    fn op_comparison_less<O>(
        &mut self,
        out: &mut O,
        to: Register,
        lhs: CopyableValue,
        rhs: CopyableValue,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        binary_op_comparison!(self, out, to, <, lhs, rhs)
    }

    fn op_comparison_greater<O>(
        &mut self,
        out: &mut O,
        to: Register,
        lhs: CopyableValue,
        rhs: CopyableValue,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        binary_op_comparison!(self, out, to, >, lhs, rhs)
    }

    fn op_comparison_less_or_equal<O>(
        &mut self,
        out: &mut O,
        to: Register,
        lhs: CopyableValue,
        rhs: CopyableValue,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        binary_op_comparison!(self, out, to, <=, lhs, rhs)
    }

    fn op_comparison_greater_or_equal<O>(
        &mut self,
        out: &mut O,
        to: Register,
        lhs: CopyableValue,
        rhs: CopyableValue,
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
        to: Register,
        lhs: CopyableValue,
        rhs: CopyableValue,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        self.assign(to, CopyableValue::Boolean(self.values_eq(lhs, rhs)));

        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)]
    fn op_comparison_not_equal<O>(
        &mut self,
        _: &mut O,
        to: Register,
        lhs: CopyableValue,
        rhs: CopyableValue,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        self.assign(to, CopyableValue::Boolean(!self.values_eq(lhs, rhs)));

        Ok(())
    }

    fn op_logical_and<O>(
        &mut self,
        out: &mut O,
        to: Register,
        lhs: CopyableValue,
        rhs: CopyableValue,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        if let (CopyableValue::Boolean(lhs), CopyableValue::Boolean(rhs)) = (lhs, rhs) {
            self.assign(to, CopyableValue::Boolean(lhs && rhs));

            Ok(())
        } else {
            writeln!(
                out,
                "invalid operands for logical and (lhs is {}, rhs is {})",
                self.value_to_string(lhs, &mut HashSet::new()),
                self.value_to_string(lhs, &mut HashSet::new())
            )
            .map_err(|_| Interrupt::Panic)?;

            self.panic(out, None)
        }
    }

    fn op_logical_or<O>(
        &mut self,
        out: &mut O,
        to: Register,
        lhs: CopyableValue,
        rhs: CopyableValue,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        if let (CopyableValue::Boolean(lhs), CopyableValue::Boolean(rhs)) = (lhs, rhs) {
            self.assign(to, CopyableValue::Boolean(lhs || rhs));

            Ok(())
        } else {
            writeln!(
                out,
                "invalid operands for logical or (lhs is {}, rhs is {})",
                self.value_to_string(lhs, &mut HashSet::new()),
                self.value_to_string(lhs, &mut HashSet::new())
            )
            .map_err(|_| Interrupt::Panic)?;

            self.panic(out, None)
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    fn op_assign<O>(
        &mut self,
        _: &mut O,
        to: Register,
        operand: CopyableValue,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        self.assign(to, operand);

        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)]
    fn op_push<O>(&mut self, _: &mut O, operand: CopyableValue) -> Result<(), Interrupt>
    where
        O: Write,
    {
        if self.sp.is_none() {
            self.sp = Some(self.registers.len());
        }

        self.registers.push(operand);

        Ok(())
    }

    fn op_access<O>(
        &mut self,
        out: &mut O,
        to: Register,
        index: u8,
        of: CopyableValue,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::ValueIndex(value_index) = of
            && let Value::Compound(fields) | Value::TaggedCompound { fields, .. } =
                &self.values[usize::from(value_index)]
        {
            self.assign(to, fields[usize::from(index)]);

            Ok(())
        } else {
            writeln!(
                out,
                "tried access to a field of something other than a compound (got {})",
                self.value_to_string(of, &mut HashSet::new())
            )
            .map_err(|_| Interrupt::Panic)?;

            self.panic(out, None)
        }
    }

    fn op_access_assign<O>(
        &mut self,
        out: &mut O,
        to: CopyableValue,
        index: u8,
        value: CopyableValue,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::ValueIndex(value_index) = to
            && let Value::Compound(fields) | Value::TaggedCompound { fields, .. } =
                &mut self.values[usize::from(value_index)]
        {
            fields[usize::from(index)] = value;

            Ok(())
        } else {
            writeln!(
                out,
                "tried to assign to a field of something other than a compound (got {})",
                self.value_to_string(to, &mut HashSet::new())
            )
            .map_err(|_| Interrupt::Panic)?;

            self.panic(out, None)
        }
    }

    fn op_get_tag<O>(
        &mut self,
        out: &mut O,
        to: Register,
        of: CopyableValue,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::ValueIndex(value_index) = of
            && let Value::TaggedCompound { tag, .. } = self.values[usize::from(value_index)]
        {
            self.assign(to, CopyableValue::U16(tag));

            Ok(())
        } else {
            writeln!(
                out,
                "tried to get the tag of something other than a tagged compound (got {})",
                self.value_to_string(of, &mut HashSet::new())
            )
            .map_err(|_| Interrupt::Panic)?;

            self.panic(out, None)
        }
    }

    fn op_call<O>(
        &mut self,
        out: &mut O,
        to: Register,
        callee: CopyableValue,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        let sp = self.sp.take().unwrap_or(self.registers.len());

        match callee {
            CopyableValue::Fn {
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

                self.ip = usize::try_from(address).expect("call address was not a valid usize");

                self.fp = sp;

                self.call_frames.push(call_frame);

                self.registers.resize_with(
                    self.registers.len() + usize::from(max_register_count),
                    || CopyableValue::Never,
                );

                Ok(())
            }
            CopyableValue::NativeFn(native_fn) => {
                let call_arguments = self.registers.drain(sp..).collect::<Vec<_>>();

                self.native_fn_call(call_arguments.as_slice(), native_fn, out)
                    .map(|value| {
                        self.assign(to, value);
                    })
            }
            _ => {
                writeln!(
                    out,
                    "tried to call an uncallable (got {})",
                    self.value_to_string(callee, &mut HashSet::new())
                )
                .map_err(|_| Interrupt::Panic)?;

                self.panic(out, None)
            }
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
            writeln!(out, "jump address was not valid").map_err(|_| Interrupt::Panic)?;

            self.panic(out, None)
        }
    }

    fn op_branch<O>(
        &mut self,
        out: &mut O,
        condition: CopyableValue,
        when_true: u32,
        otherwise: u32,
    ) -> Result<(), Interrupt>
    where
        O: Write,
    {
        match condition {
            CopyableValue::Boolean(true) => self.op_jump(out, when_true),
            CopyableValue::Boolean(false) => self.op_jump(out, otherwise),
            _ => {
                writeln!(
                    out,
                    "branch condition was not a boolean (got {})",
                    self.value_to_string(condition, &mut HashSet::new())
                )
                .map_err(|_| Interrupt::Panic)?;

                self.panic(out, None)
            }
        }
    }

    fn op_return<O>(&mut self, out: &mut O, value: CopyableValue) -> Result<(), Interrupt>
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
                writeln!(out, "return address was not valid").map_err(|_| Interrupt::Panic)?;

                self.panic(out, None)
            }
        } else {
            writeln!(out, "tried to return from outside of all functions")
                .map_err(|_| Interrupt::Panic)?;

            self.panic(out, None)
        }
    }

    fn assign(&mut self, to: Register, value: CopyableValue) {
        let offset = self.fp + usize::from(u16::from(to));

        self.registers[offset] = value;
    }

    fn get_register(&self, from: Register) -> CopyableValue {
        self.registers[self.fp + usize::from(u16::from(from))]
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

    fn dereference_value(&mut self, value: CopyableValue, make_constant: bool) -> CopyableValue {
        match value {
            CopyableValue::Register(offset) => self.get_register(Register(offset)),
            CopyableValue::ValueIndex(value_index) => {
                match &self.values[usize::from(value_index)] {
                    Value::Compound(values) if make_constant => {
                        let dereferenced_values = values
                            .clone()
                            .into_iter()
                            .map(|value| self.dereference_value(value, make_constant))
                            .collect::<Vec<_>>();

                        self.values.push(Value::Compound(dereferenced_values));

                        let value_index =
                            CopyableValue::ValueIndex(ValueIndex(self.values.len() - 1));

                        self.allocated += self.size_of_value(value_index);

                        value_index
                    }
                    Value::TaggedCompound {
                        fields: values,
                        tag,
                    } if make_constant => {
                        let tag = *tag;

                        let dereferenced_values = values
                            .clone()
                            .into_iter()
                            .map(|value| self.dereference_value(value, make_constant))
                            .collect::<Vec<_>>();

                        self.values.push(Value::TaggedCompound {
                            fields: dereferenced_values,
                            tag,
                        });

                        let value_index =
                            CopyableValue::ValueIndex(ValueIndex(self.values.len() - 1));

                        self.allocated += self.size_of_value(value_index);

                        value_index
                    }
                    Value::String(text) if make_constant => {
                        self.values.push(Value::String(text.clone()));

                        let value_index =
                            CopyableValue::ValueIndex(ValueIndex(self.values.len() - 1));

                        self.allocated += self.size_of_value(value_index);

                        value_index
                    }
                    _ => value,
                }
            }
            _ => value,
        }
    }

    #[allow(dead_code)]
    fn option_some(&mut self, value: CopyableValue) -> CopyableValue {
        self.values.push(Value::TaggedCompound {
            fields: vec![value],
            tag: 1,
        });

        let value_index = CopyableValue::ValueIndex(ValueIndex(self.values.len() - 1));

        self.allocated += self.size_of_value(value_index);

        value_index
    }

    #[allow(dead_code)]
    fn option_none(&mut self) -> CopyableValue {
        self.values.push(Value::TaggedCompound {
            fields: vec![],
            tag: 0,
        });

        let value_index = CopyableValue::ValueIndex(ValueIndex(self.values.len() - 1));

        self.allocated += self.size_of_value(value_index);

        value_index
    }

    #[allow(dead_code)]
    fn result_ok(&mut self, value: CopyableValue) -> CopyableValue {
        self.values.push(Value::TaggedCompound {
            fields: vec![value],
            tag: 1,
        });

        let value_index = CopyableValue::ValueIndex(ValueIndex(self.values.len() - 1));

        self.allocated += self.size_of_value(value_index);

        value_index
    }

    #[allow(dead_code)]
    fn result_err(&mut self) -> CopyableValue {
        self.values.push(Value::TaggedCompound {
            fields: vec![],
            tag: 0,
        });

        let value_index = CopyableValue::ValueIndex(ValueIndex(self.values.len() - 1));

        self.allocated += self.size_of_value(value_index);

        value_index
    }

    #[allow(clippy::too_many_lines)]
    fn native_fn_call<O>(
        &mut self,
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
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

    fn values_eq(&self, lhs: CopyableValue, rhs: CopyableValue) -> bool {
        match (lhs, rhs) {
            (CopyableValue::U8(lhs), CopyableValue::U8(rhs)) => lhs == rhs,
            (CopyableValue::I8(lhs), CopyableValue::I8(rhs)) => lhs == rhs,
            (CopyableValue::U16(lhs), CopyableValue::U16(rhs)) => lhs == rhs,
            (CopyableValue::I16(lhs), CopyableValue::I16(rhs)) => lhs == rhs,
            (CopyableValue::U32(lhs), CopyableValue::U32(rhs)) => lhs == rhs,
            (CopyableValue::I32(lhs), CopyableValue::I32(rhs)) => lhs == rhs,
            (CopyableValue::U64(lhs), CopyableValue::U64(rhs)) => lhs == rhs,
            (CopyableValue::I64(lhs), CopyableValue::I64(rhs)) => lhs == rhs,
            (CopyableValue::F32(lhs), CopyableValue::F32(rhs)) => lhs == rhs,
            (CopyableValue::F64(lhs), CopyableValue::F64(rhs)) => lhs == rhs,
            (CopyableValue::Boolean(lhs), CopyableValue::Boolean(rhs)) => lhs == rhs,
            (CopyableValue::Unit, CopyableValue::Unit) => true,
            (CopyableValue::Fn { address: lhs, .. }, CopyableValue::Fn { address: rhs, .. }) => {
                lhs == rhs
            }
            (CopyableValue::NativeFn(lhs), CopyableValue::NativeFn(rhs)) => lhs == rhs,
            (CopyableValue::ValueIndex(lhs), CopyableValue::ValueIndex(rhs)) => {
                match (
                    &self.values[usize::from(lhs)],
                    &self.values[usize::from(rhs)],
                ) {
                    (Value::String(lhs), Value::String(rhs)) => lhs == rhs,
                    (
                        Value::TaggedCompound { tag: lhs, .. },
                        Value::TaggedCompound { tag: rhs, .. },
                    ) if lhs != rhs => false,
                    (Value::Compound(lhs), Value::Compound(rhs))
                    | (
                        Value::TaggedCompound { fields: lhs, .. },
                        Value::TaggedCompound { fields: rhs, .. },
                    )
                    | (Value::Vec(lhs), Value::Vec(rhs)) => {
                        for (lhs, rhs) in lhs.iter().zip(rhs) {
                            if !self.values_eq(*lhs, *rhs) {
                                return false;
                            }
                        }

                        true
                    }
                    (_, _) => panic!("(EQUALITY {lhs:?} {rhs:?})"),
                }
            }
            (_, _) => panic!("(EQUALITY {lhs:?} {rhs:?})"),
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
        let mut marked = vec![const { None }; self.values.len()];
        let mut marked_count = 0;

        for value_index in &self.constants {
            self.mark_value(
                &mut marked,
                CopyableValue::ValueIndex(*value_index),
                &mut marked_count,
            );
        }

        for value in &self.registers {
            self.mark_value(&mut marked, *value, &mut marked_count);
        }

        (marked, marked_count)
    }

    fn mark_value(
        &self,
        marked: &mut [Option<ValueIndex>],
        value: CopyableValue,
        marked_count: &mut usize,
    ) {
        if let CopyableValue::ValueIndex(index) = value {
            match &self.values[usize::from(index)] {
                Value::String(_) => {}
                Value::Compound(values)
                | Value::TaggedCompound { fields: values, .. }
                | Value::Vec(values) => {
                    for value in values {
                        self.mark_value(marked, *value, marked_count);
                    }
                }
            }

            if marked[usize::from(index)].is_none() {
                marked[usize::from(index)] = Some(ValueIndex(*marked_count));

                *marked_count += 1;
            }
        }
    }

    fn sweep(&mut self, marked: &[Option<ValueIndex>], marked_count: usize) {
        let mut values = Vec::with_capacity(marked_count);

        for value_index in &self.constants {
            self.retain_value(&mut values, CopyableValue::ValueIndex(*value_index), marked);
        }

        self.registers = self
            .registers
            .iter()
            .map(|value| self.retain_value(&mut values, *value, marked))
            .collect::<Vec<_>>();

        values.sort_by_key(|(index, _)| *index);
        values.dedup_by_key(|(index, _)| *index);

        self.allocated = 0;

        self.values = values
            .into_iter()
            .fold(vec![], |mut accum, (value_index, value)| {
                self.allocated += self.size_of_value(CopyableValue::ValueIndex(value_index));

                accum.push(value);

                accum
            });
    }

    fn retain_value(
        &self,
        replacement_values: &mut Vec<(ValueIndex, Value)>,
        value: CopyableValue,
        marked: &[Option<ValueIndex>],
    ) -> CopyableValue {
        if let CopyableValue::ValueIndex(index) = value {
            marked
                .get(usize::from(index))
                .and_then(|marked| *marked)
                .map_or(CopyableValue::Unit, |replacement_index| {
                    match &self.values[usize::from(index)] {
                        Value::String(text) => replacement_values
                            .push((replacement_index, Value::String(text.clone()))),
                        Value::Vec(values) => {
                            let values = values
                                .iter()
                                .map(|value| self.retain_value(replacement_values, *value, marked))
                                .collect::<Vec<_>>();

                            replacement_values.push((replacement_index, Value::Vec(values)));
                        }
                        Value::Compound(values) => {
                            let values = values
                                .iter()
                                .map(|value| self.retain_value(replacement_values, *value, marked))
                                .collect::<Vec<_>>();

                            replacement_values.push((replacement_index, Value::Compound(values)));
                        }
                        Value::TaggedCompound {
                            fields: values,
                            tag,
                        } => {
                            let values = values
                                .iter()
                                .map(|value| self.retain_value(replacement_values, *value, marked))
                                .collect::<Vec<_>>();

                            replacement_values.push((
                                replacement_index,
                                Value::TaggedCompound {
                                    fields: values,
                                    tag: *tag,
                                },
                            ));
                        }
                    }

                    CopyableValue::ValueIndex(replacement_index)
                })
        } else {
            value
        }
    }

    fn size_of_value(&self, value: CopyableValue) -> usize {
        if let CopyableValue::ValueIndex(index) = value {
            match &self.values[usize::from(index)] {
                Value::Compound(values) => {
                    values
                        .iter()
                        .fold(0, |accum, value| accum + mem::size_of_val(value))
                        + mem::size_of_val(&self.values[usize::from(index)])
                }
                Value::TaggedCompound { fields: values, .. } => {
                    values
                        .iter()
                        .fold(0, |accum, value| accum + mem::size_of_val(value))
                        + mem::size_of_val(&self.values[usize::from(index)])
                }
                Value::String(text) => {
                    mem::size_of_val(text) + mem::size_of_val(&self.values[usize::from(index)])
                }
                Value::Vec(values) => {
                    values
                        .iter()
                        .fold(0, |accum, value| accum + mem::size_of_val(value))
                        + mem::size_of_val(&self.values[usize::from(index)])
                }
            }
        } else {
            mem::size_of_val(&value)
        }
    }
}

macro_rules! native_fn_print_debug {
    (
        $name:ident, $value_variant:ident $(,)?
    ) => {
        fn $name<O>(
            &mut self,
            call_arguments: &[$crate::targets::vm::CopyableValue],
            native_fn: $crate::low_ir::NativeFn,
            out: &mut O,
        ) -> Result<$crate::targets::vm::CopyableValue, $crate::vm::Interrupt>
        where
            O: Write,
        {
            if let $crate::targets::vm::CopyableValue::$value_variant(value) = &call_arguments[0] {
                writeln!(out, "{value:?}").expect("failed to write to output");

                Ok($crate::targets::vm::CopyableValue::Unit)
            } else {
                self.native_fn_invalid_arguments(call_arguments, native_fn, out)
            }
        }
    };
}

impl Vm {
    fn native_fn_invalid_arguments<O>(
        &self,
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        writeln!(
            out,
            "invalid arguments: {native_fn:?}({})",
            call_arguments
                .iter()
                .map(|argument| { self.value_to_string(*argument, &mut HashSet::new()) })
                .collect::<Vec<_>>()
                .join(", ")
        )
        .expect("failed to write to output");

        self.panic(out, None).map(|()| CopyableValue::Never)
    }

    native_fn_print_debug!(native_fn_print_f32, F32);
    native_fn_print_debug!(native_fn_print_f64, F64);

    fn native_fn_print_string<O>(
        &self,
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::ValueIndex(value_index) = &call_arguments[0]
            && let Some(Value::String(value)) = self.values.get(usize::from(*value_index))
        {
            writeln!(out, "{value}").expect("failed to write to output");

            Ok(CopyableValue::Unit)
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_extend_string<O>(
        &mut self,
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::ValueIndex(target) = &call_arguments[0]
            && let CopyableValue::ValueIndex(extension) = &call_arguments[1]
            && let Some(Value::String(extension)) =
                self.values.get(usize::from(*extension)).cloned()
            && let Some(Value::String(target)) = self.values.get_mut(usize::from(*target))
        {
            target.push_str(extension.as_str());

            Ok(CopyableValue::Unit)
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_slice_string<O>(
        &mut self,
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::ValueIndex(value) = &call_arguments[0]
            && let Some(Value::String(value)) = self.values.get(usize::from(*value))
            && let CopyableValue::U64(start) = &call_arguments[1]
            && let CopyableValue::U64(end) = &call_arguments[2]
            && let Ok(start) = usize::try_from(*start)
            && let Ok(end) = usize::try_from(*end)
        {
            if let Some(value) = value.get(start..end) {
                self.values.push(Value::String(value.to_string()));

                let value_index = CopyableValue::ValueIndex(ValueIndex(self.values.len() - 1));

                self.allocated += self.size_of_value(value_index);

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
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::ValueIndex(value) = &call_arguments[0]
            && let Some(Value::String(value)) = self.values.get(usize::from(*value))
            && let Ok(length) = u64::try_from(value.len())
        {
            Ok(CopyableValue::U64(length))
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_truncate_string<O>(
        &mut self,
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::ValueIndex(value) = &call_arguments[0]
            && let Some(Value::String(value)) = self.values.get_mut(usize::from(*value))
            && let CopyableValue::U64(end) = &call_arguments[1]
            && let Ok(end) = usize::try_from(*end)
        {
            value.truncate(end);

            Ok(CopyableValue::Unit)
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_floor_char_boundary_string<O>(
        &self,
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::ValueIndex(value) = &call_arguments[0]
            && let Some(Value::String(value)) = self.values.get(usize::from(*value))
            && let CopyableValue::U64(boundary) = &call_arguments[1]
            && let Ok(boundary) = usize::try_from(*boundary)
            && let Ok(boundary) = u64::try_from(value.floor_char_boundary(boundary))
        {
            Ok(CopyableValue::U64(boundary))
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_ceil_char_boundary_string<O>(
        &self,
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::ValueIndex(value) = &call_arguments[0]
            && let Some(Value::String(value)) = self.values.get(usize::from(*value))
            && let CopyableValue::U64(boundary) = &call_arguments[1]
            && let Ok(boundary) = usize::try_from(*boundary)
            && let Ok(boundary) = u64::try_from(value.ceil_char_boundary(boundary))
        {
            Ok(CopyableValue::U64(boundary))
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    #[allow(clippy::unnecessary_wraps, clippy::unused_self)]
    fn native_fn_clock<O>(
        &self,
        _: &[CopyableValue],
        _: NativeFn,
        _: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        Ok(CopyableValue::F64(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time somehow went backwards")
                .as_secs_f64()
                * 1000.0,
        ))
    }

    #[allow(clippy::unnecessary_wraps, clippy::unused_self)]
    fn native_fn_sleep<O>(
        &self,
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::U64(millis) = &call_arguments[0] {
            std::thread::sleep(std::time::Duration::from_millis(*millis));

            Ok(CopyableValue::Unit)
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_log_f64<O>(
        &self,
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::F64(value) = &call_arguments[0] {
            Ok(CopyableValue::F64(f64::log(*value, core::f64::consts::E)))
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_floor_f64<O>(
        &self,
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::F64(value) = &call_arguments[0] {
            Ok(CopyableValue::F64(f64::floor(*value)))
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_to_u64_f64<O>(
        &self,
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::F64(value) = &call_arguments[0] {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let value = *value as u64;

            Ok(CopyableValue::U64(value))
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_from_u64_f64<O>(
        &self,
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::U64(value) = &call_arguments[0] {
            #[allow(clippy::cast_precision_loss)]
            let value = *value as f64;

            Ok(CopyableValue::F64(value))
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    fn native_fn_new_vec<O>(
        &mut self,
        _: &[CopyableValue],
        _: NativeFn,
        _: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        self.values.push(Value::Vec(vec![]));

        Ok(CopyableValue::ValueIndex(ValueIndex(self.values.len() - 1)))
    }

    fn native_fn_get_vec<O>(
        &mut self,
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::ValueIndex(value) = &call_arguments[0]
            && let Some(Value::Vec(value)) = self.values.get(usize::from(*value))
            && let CopyableValue::U64(index) = &call_arguments[1]
            && let Ok(index) = usize::try_from(*index)
        {
            if let Some(value) = value.get(index) {
                Ok(self.option_some(*value))
            } else {
                Ok(self.option_none())
            }
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_extend_vec<O>(
        &mut self,
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::ValueIndex(target) = &call_arguments[0]
            && let CopyableValue::ValueIndex(extension) = &call_arguments[1]
            && let Some(Value::Vec(extension)) = self.values.get(usize::from(*extension)).cloned()
            && let Some(Value::Vec(target)) = self.values.get_mut(usize::from(*target))
        {
            for value in &extension {
                self.allocated += mem::size_of_val(value);
            }

            target.extend(extension);

            Ok(CopyableValue::Unit)
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_slice_vec<O>(
        &mut self,
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::ValueIndex(value) = &call_arguments[0]
            && let Some(Value::Vec(value)) = self.values.get(usize::from(*value))
            && let CopyableValue::U64(start) = &call_arguments[1]
            && let CopyableValue::U64(end) = &call_arguments[2]
            && let Ok(start) = usize::try_from(*start)
            && let Ok(end) = usize::try_from(*end)
        {
            if let Some(value) = value.get(start..end) {
                self.values.push(Value::Vec(value.to_vec()));

                let value_index = CopyableValue::ValueIndex(ValueIndex(self.values.len() - 1));

                self.allocated += self.size_of_value(value_index);

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
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::ValueIndex(value) = &call_arguments[0]
            && let Some(Value::Vec(value)) = self.values.get(usize::from(*value))
            && let Ok(length) = u64::try_from(value.len())
        {
            Ok(CopyableValue::U64(length))
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_truncate_vec<O>(
        &mut self,
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::ValueIndex(value) = &call_arguments[0]
            && let Some(Value::Vec(value)) = self.values.get_mut(usize::from(*value))
            && let CopyableValue::U64(end) = &call_arguments[1]
            && let Ok(end) = usize::try_from(*end)
        {
            for value in value.drain(end..) {
                self.allocated -= mem::size_of_val(&value);
            }

            Ok(CopyableValue::Unit)
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_push_vec<O>(
        &mut self,
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::ValueIndex(value_index) = &call_arguments[0]
            && let Some(Value::Vec(value)) = self.values.get_mut(usize::from(*value_index))
        {
            value.push(call_arguments[1]);

            self.allocated += mem::size_of_val(&call_arguments[1]);

            Ok(CopyableValue::Unit)
        } else {
            self.native_fn_invalid_arguments(call_arguments, native_fn, out)
        }
    }

    fn native_fn_pop_vec<O>(
        &mut self,
        call_arguments: &[CopyableValue],
        native_fn: NativeFn,
        out: &mut O,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        if let CopyableValue::ValueIndex(value_index) = &call_arguments[0]
            && let Some(Value::Vec(value)) = self.values.get_mut(usize::from(*value_index))
        {
            if let Some(value) = value.pop() {
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
