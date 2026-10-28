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
    max_register_count: u16,
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
    sp: usize,
    fp: usize,
}

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
        next_gc: 1_000_000,
        call_frames: vec![CallFrame {
            fp: 0,
            from: 0,
            max_register_count: 0,
            to: Register(0),
        }],
        function_labels: vec![],
        constants: vec![],
        ip: 0,
        sp: 0,
        fp: 0,
    };

    let instructions = crate::targets::vm::decompile(bytecode, &mut vm);

    vm.run(instructions.as_slice(), out)
}

macro_rules! binary_op_arithmetic {
    (
        $op:tt, $lhs:ident, $rhs:ident $(,)?
    ) => {{
        match ($lhs, $rhs) {
            ($crate::targets::vm::CopyableValue::U8(lhs), $crate::targets::vm::CopyableValue::U8(rhs)) => {
                $crate::targets::vm::CopyableValue::U8(lhs $op rhs)
            }
            ($crate::targets::vm::CopyableValue::I8(lhs), $crate::targets::vm::CopyableValue::I8(rhs)) => {
                $crate::targets::vm::CopyableValue::I8(lhs $op rhs)
            }
            ($crate::targets::vm::CopyableValue::U16(lhs), $crate::targets::vm::CopyableValue::U16(rhs)) => {
                $crate::targets::vm::CopyableValue::U16(lhs $op rhs)
            }
            ($crate::targets::vm::CopyableValue::I16(lhs), $crate::targets::vm::CopyableValue::I16(rhs)) => {
                $crate::targets::vm::CopyableValue::I16(lhs $op rhs)
            }
            ($crate::targets::vm::CopyableValue::U32(lhs), $crate::targets::vm::CopyableValue::U32(rhs)) => {
                $crate::targets::vm::CopyableValue::U32(lhs $op rhs)
            }
            ($crate::targets::vm::CopyableValue::I32(lhs), $crate::targets::vm::CopyableValue::I32(rhs)) => {
                $crate::targets::vm::CopyableValue::I32(lhs $op rhs)
            }
            ($crate::targets::vm::CopyableValue::U64(lhs), $crate::targets::vm::CopyableValue::U64(rhs)) => {
                $crate::targets::vm::CopyableValue::U64(lhs $op rhs)
            }
            ($crate::targets::vm::CopyableValue::I64(lhs), $crate::targets::vm::CopyableValue::I64(rhs)) => {
                $crate::targets::vm::CopyableValue::I64(lhs $op rhs)
            }
            ($crate::targets::vm::CopyableValue::F32(lhs), $crate::targets::vm::CopyableValue::F32(rhs)) => {
                $crate::targets::vm::CopyableValue::F32(lhs $op rhs)
            }
            ($crate::targets::vm::CopyableValue::F64(lhs), $crate::targets::vm::CopyableValue::F64(rhs)) => {
                $crate::targets::vm::CopyableValue::F64(lhs $op rhs)
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
            ) => panic!("incorrect operands for {} {:?} {:?}", stringify!($op), $lhs, $rhs),
        }
    }};
}

macro_rules! binary_op_comparison {
    (
        $op:tt, $lhs:ident, $rhs:ident $(,)?
    ) => {{
        match ($lhs, $rhs) {
            ($crate::targets::vm::CopyableValue::U8(lhs), $crate::targets::vm::CopyableValue::U8(rhs)) => {
                $crate::targets::vm::CopyableValue::Boolean(lhs $op rhs)
            }
            ($crate::targets::vm::CopyableValue::I8(lhs), $crate::targets::vm::CopyableValue::I8(rhs)) => {
                $crate::targets::vm::CopyableValue::Boolean(lhs $op rhs)
            }
            ($crate::targets::vm::CopyableValue::U16(lhs), $crate::targets::vm::CopyableValue::U16(rhs)) => {
                $crate::targets::vm::CopyableValue::Boolean(lhs $op rhs)
            }
            ($crate::targets::vm::CopyableValue::I16(lhs), $crate::targets::vm::CopyableValue::I16(rhs)) => {
                $crate::targets::vm::CopyableValue::Boolean(lhs $op rhs)
            }
            ($crate::targets::vm::CopyableValue::U32(lhs), $crate::targets::vm::CopyableValue::U32(rhs)) => {
                $crate::targets::vm::CopyableValue::Boolean(lhs $op rhs)
            }
            ($crate::targets::vm::CopyableValue::I32(lhs), $crate::targets::vm::CopyableValue::I32(rhs)) => {
                $crate::targets::vm::CopyableValue::Boolean(lhs $op rhs)
            }
            ($crate::targets::vm::CopyableValue::U64(lhs), $crate::targets::vm::CopyableValue::U64(rhs)) => {
                $crate::targets::vm::CopyableValue::Boolean(lhs $op rhs)
            }
            ($crate::targets::vm::CopyableValue::I64(lhs), $crate::targets::vm::CopyableValue::I64(rhs)) => {
                $crate::targets::vm::CopyableValue::Boolean(lhs $op rhs)
            }
            ($crate::targets::vm::CopyableValue::F32(lhs), $crate::targets::vm::CopyableValue::F32(rhs)) => {
                $crate::targets::vm::CopyableValue::Boolean(lhs $op rhs)
            }
            ($crate::targets::vm::CopyableValue::F64(lhs), $crate::targets::vm::CopyableValue::F64(rhs)) => {
                $crate::targets::vm::CopyableValue::Boolean(lhs $op rhs)
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
            ) => panic!("incorrect operands for {} {:?} {:?}", stringify!($op), $lhs, $rhs),
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

                self.gc();

                assert_eq!(self.allocated, 0, "{} BYTES LEAKED", self.allocated);
            })?;

            if self.call_frames.is_empty() {
                break;
            }
        }

        self.registers.clear();

        self.gc();

        assert_eq!(self.allocated, 0, "{} BYTES LEAKED", self.allocated);

        Ok(())
    }

    #[allow(clippy::too_many_lines, clippy::inline_always)]
    #[inline(always)]
    pub fn step<O>(&mut self, instruction: &Instruction, out: &mut O) -> Result<(), Interrupt>
    where
        O: Write,
    {
        match instruction {
            Instruction::NoOp | Instruction::Label => {}
            Instruction::Panic => return self.panic(out, None),
            Instruction::FnLabel { fn_label } => {
                self.function_labels.push(FnLabel::new(
                    fn_label.file_name().to_string(),
                    fn_label.function_label().to_string(),
                ));
            }
            Instruction::NotRegister { to, operand } => {
                let operand = self.get_register(*operand);

                match operand {
                    CopyableValue::Boolean(value) => {
                        self.assign(*to, CopyableValue::Boolean(!value));
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
                        panic!("incorrect argument for logical not {operand:?}")
                    }
                }
            }
            Instruction::NotValue { to, operand } => {
                let operand = self.dereference_value(*operand, true);

                match operand {
                    CopyableValue::Boolean(value) => {
                        self.assign(*to, CopyableValue::Boolean(!value));
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
                        panic!("incorrect argument for logical not {operand:?}")
                    }
                }
            }
            Instruction::NegateRegister { to, operand } => {
                let operand = self.get_register(*operand);

                match operand {
                    CopyableValue::I8(value) => {
                        self.assign(*to, CopyableValue::I8(-value));
                    }
                    CopyableValue::I16(value) => {
                        self.assign(*to, CopyableValue::I16(-value));
                    }
                    CopyableValue::I32(value) => {
                        self.assign(*to, CopyableValue::I32(-value));
                    }
                    CopyableValue::I64(value) => {
                        self.assign(*to, CopyableValue::I64(-value));
                    }
                    CopyableValue::F32(value) => {
                        self.assign(*to, CopyableValue::F32(-value));
                    }
                    CopyableValue::F64(value) => {
                        self.assign(*to, CopyableValue::F64(-value));
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
                        panic!("incorrect argument for negate {operand:?}")
                    }
                }
            }
            Instruction::NegateValue { to, operand } => {
                let operand = self.dereference_value(*operand, true);

                match operand {
                    CopyableValue::I8(value) => {
                        self.assign(*to, CopyableValue::I8(-value));
                    }
                    CopyableValue::I16(value) => {
                        self.assign(*to, CopyableValue::I16(-value));
                    }
                    CopyableValue::I32(value) => {
                        self.assign(*to, CopyableValue::I32(-value));
                    }
                    CopyableValue::I64(value) => {
                        self.assign(*to, CopyableValue::I64(-value));
                    }
                    CopyableValue::F32(value) => {
                        self.assign(*to, CopyableValue::F32(-value));
                    }
                    CopyableValue::F64(value) => {
                        self.assign(*to, CopyableValue::F64(-value));
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
                        panic!("incorrect argument for negate {operand:?}")
                    }
                }
            }
            Instruction::MultiplyRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.assign(*to, binary_op_arithmetic!(*, lhs, rhs));
            }
            Instruction::MultiplyRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, binary_op_arithmetic!(*, lhs, rhs));
            }
            Instruction::MultiplyValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.assign(*to, binary_op_arithmetic!(*, lhs, rhs));
            }
            Instruction::MultiplyValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, binary_op_arithmetic!(*, lhs, rhs));
            }
            Instruction::DivideRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.assign(*to, binary_op_arithmetic!(/, lhs, rhs));
            }
            Instruction::DivideRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, binary_op_arithmetic!(/, lhs, rhs));
            }
            Instruction::DivideValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.assign(*to, binary_op_arithmetic!(/, lhs, rhs));
            }
            Instruction::DivideValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, binary_op_arithmetic!(/, lhs, rhs));
            }
            Instruction::RemainderRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.assign(*to, binary_op_arithmetic!(%, lhs, rhs));
            }
            Instruction::RemainderRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, binary_op_arithmetic!(%, lhs, rhs));
            }
            Instruction::RemainderValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.assign(*to, binary_op_arithmetic!(%, lhs, rhs));
            }
            Instruction::RemainderValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, binary_op_arithmetic!(%, lhs, rhs));
            }
            Instruction::AddRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.assign(*to, binary_op_arithmetic!(+, lhs, rhs));
            }
            Instruction::AddRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, binary_op_arithmetic!(+, lhs, rhs));
            }
            Instruction::AddValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.assign(*to, binary_op_arithmetic!(+, lhs, rhs));
            }
            Instruction::AddValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, binary_op_arithmetic!(+, lhs, rhs));
            }
            Instruction::SubtractRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.assign(*to, binary_op_arithmetic!(-, lhs, rhs));
            }
            Instruction::SubtractRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, binary_op_arithmetic!(-, lhs, rhs));
            }
            Instruction::SubtractValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.assign(*to, binary_op_arithmetic!(-, lhs, rhs));
            }
            Instruction::SubtractValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, binary_op_arithmetic!(-, lhs, rhs));
            }
            Instruction::LessRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.assign(*to, binary_op_comparison!(<, lhs, rhs));
            }
            Instruction::LessRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, binary_op_comparison!(<, lhs, rhs));
            }
            Instruction::LessValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.assign(*to, binary_op_comparison!(<, lhs, rhs));
            }
            Instruction::LessValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, binary_op_comparison!(<, lhs, rhs));
            }
            Instruction::GreaterRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.assign(*to, binary_op_comparison!(>, lhs, rhs));
            }
            Instruction::GreaterRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, binary_op_comparison!(>, lhs, rhs));
            }
            Instruction::GreaterValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.assign(*to, binary_op_comparison!(>, lhs, rhs));
            }
            Instruction::GreaterValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, binary_op_comparison!(>, lhs, rhs));
            }
            Instruction::LessOrEqualRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.assign(*to, binary_op_comparison!(<=, lhs, rhs));
            }
            Instruction::LessOrEqualRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, binary_op_comparison!(<=, lhs, rhs));
            }
            Instruction::LessOrEqualValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.assign(*to, binary_op_comparison!(<=, lhs, rhs));
            }
            Instruction::LessOrEqualValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, binary_op_comparison!(<=, lhs, rhs));
            }
            Instruction::GreaterOrEqualRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.assign(*to, binary_op_comparison!(>=, lhs, rhs));
            }
            Instruction::GreaterOrEqualRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, binary_op_comparison!(>=, lhs, rhs));
            }
            Instruction::GreaterOrEqualValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.assign(*to, binary_op_comparison!(>=, lhs, rhs));
            }
            Instruction::GreaterOrEqualValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, binary_op_comparison!(>=, lhs, rhs));
            }
            Instruction::EqualRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.assign(*to, CopyableValue::Boolean(self.values_eq(lhs, rhs)));
            }
            Instruction::EqualRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, CopyableValue::Boolean(self.values_eq(lhs, rhs)));
            }
            Instruction::EqualValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.assign(*to, CopyableValue::Boolean(self.values_eq(lhs, rhs)));
            }
            Instruction::EqualValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, CopyableValue::Boolean(self.values_eq(lhs, rhs)));
            }
            Instruction::NotEqualRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                self.assign(*to, CopyableValue::Boolean(!self.values_eq(lhs, rhs)));
            }
            Instruction::NotEqualRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, CopyableValue::Boolean(!self.values_eq(lhs, rhs)));
            }
            Instruction::NotEqualValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                self.assign(*to, CopyableValue::Boolean(!self.values_eq(lhs, rhs)));
            }
            Instruction::NotEqualValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                self.assign(*to, CopyableValue::Boolean(!self.values_eq(lhs, rhs)));
            }
            Instruction::AndRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                if let (CopyableValue::Boolean(lhs), CopyableValue::Boolean(rhs)) = (lhs, rhs) {
                    {
                        self.assign(*to, CopyableValue::Boolean(lhs && rhs));
                    }
                } else {
                    panic!(
                        "incorrect operands for {} {:?} {:?}",
                        stringify!(&&),
                        lhs,
                        rhs
                    )
                }
            }
            Instruction::AndRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                if let (CopyableValue::Boolean(lhs), CopyableValue::Boolean(rhs)) = (lhs, rhs) {
                    {
                        self.assign(*to, CopyableValue::Boolean(lhs && rhs));
                    }
                } else {
                    panic!(
                        "incorrect operands for {} {:?} {:?}",
                        stringify!(&&),
                        lhs,
                        rhs
                    )
                }
            }
            Instruction::AndValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                if let (CopyableValue::Boolean(lhs), CopyableValue::Boolean(rhs)) = (lhs, rhs) {
                    {
                        self.assign(*to, CopyableValue::Boolean(lhs && rhs));
                    }
                } else {
                    panic!(
                        "incorrect operands for {} {:?} {:?}",
                        stringify!(&&),
                        lhs,
                        rhs
                    )
                }
            }
            Instruction::AndValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                if let (CopyableValue::Boolean(lhs), CopyableValue::Boolean(rhs)) = (lhs, rhs) {
                    {
                        self.assign(*to, CopyableValue::Boolean(lhs && rhs));
                    }
                } else {
                    panic!(
                        "incorrect operands for {} {:?} {:?}",
                        stringify!(&&),
                        lhs,
                        rhs
                    )
                }
            }
            Instruction::OrRegisterRegister { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.get_register(*rhs);

                if let (CopyableValue::Boolean(lhs), CopyableValue::Boolean(rhs)) = (lhs, rhs) {
                    {
                        self.assign(*to, CopyableValue::Boolean(lhs || rhs));
                    }
                } else {
                    panic!(
                        "incorrect operands for {} {:?} {:?}",
                        stringify!(||),
                        lhs,
                        rhs
                    )
                }
            }
            Instruction::OrRegisterValue { to, lhs, rhs } => {
                let lhs = self.get_register(*lhs);
                let rhs = self.dereference_value(*rhs, true);

                if let (CopyableValue::Boolean(lhs), CopyableValue::Boolean(rhs)) = (lhs, rhs) {
                    {
                        self.assign(*to, CopyableValue::Boolean(lhs || rhs));
                    }
                } else {
                    panic!(
                        "incorrect operands for {} {:?} {:?}",
                        stringify!(||),
                        lhs,
                        rhs
                    )
                }
            }
            Instruction::OrValueRegister { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.get_register(*rhs);

                if let (CopyableValue::Boolean(lhs), CopyableValue::Boolean(rhs)) = (lhs, rhs) {
                    {
                        self.assign(*to, CopyableValue::Boolean(lhs || rhs));
                    }
                } else {
                    panic!(
                        "incorrect operands for {} {:?} {:?}",
                        stringify!(||),
                        lhs,
                        rhs
                    )
                }
            }
            Instruction::OrValueValue { to, lhs, rhs } => {
                let lhs = self.dereference_value(*lhs, true);
                let rhs = self.dereference_value(*rhs, true);

                if let (CopyableValue::Boolean(lhs), CopyableValue::Boolean(rhs)) = (lhs, rhs) {
                    {
                        self.assign(*to, CopyableValue::Boolean(lhs || rhs));
                    }
                } else {
                    panic!(
                        "incorrect operands for {} {:?} {:?}",
                        stringify!(||),
                        lhs,
                        rhs
                    )
                }
            }
            Instruction::AssignRegister { to, operand } => {
                let operand = self.get_register(*operand);

                self.assign(*to, operand);
            }
            Instruction::AssignValue { to, operand } => {
                let operand = self.dereference_value(*operand, true);

                self.assign(*to, operand);
            }
            Instruction::PopN { count } => {
                self.sp -= usize::from(*count);
            }
            Instruction::PushRegister { operand } => {
                let operand = self.get_register(*operand);

                self.registers[self.sp] = operand;

                self.sp += 1;
            }
            Instruction::PushValue { operand } => {
                let operand = self.dereference_value(*operand, true);

                self.registers[self.sp] = operand;

                self.sp += 1;
            }
            Instruction::AccessRegister { to, index, of } => {
                let of = self.get_register(*of);

                let value = if let CopyableValue::ValueIndex(value_index) = of {
                    match self.values.get(usize::from(value_index)) {
                        Some(Value::Compound(fields) | Value::TaggedCompound { fields, .. }) => {
                            fields.get(usize::from(*index)).map_or_else(
                                || {
                                    panic!("tried to access a field that doesn't exist {of:?} @ {index}");
                                },
                                |field| *field,
                            )
                        }
                        _ => panic!(
                            "tried access to a field of something other than a compound {of:?} @ {index}"
                        ),
                    }
                } else {
                    panic!(
                        "tried to access a field of something other than a compound {of:?} @ {index}"
                    );
                };

                self.assign(*to, value);
            }
            Instruction::AccessValue { to, index, of } => {
                let of = self.dereference_value(*of, true);

                let value = if let CopyableValue::ValueIndex(value_index) = of {
                    match self.values.get(usize::from(value_index)) {
                        Some(Value::Compound(fields) | Value::TaggedCompound { fields, .. }) => {
                            fields.get(usize::from(*index)).map_or_else(
                                || {
                                    panic!("tried to access a field that doesn't exist {of:?} @ {index}");
                                },
                                |field| *field,
                            )
                        }
                        _ => panic!(
                            "tried access to a field of something other than a compound {of:?} @ {index}"
                        ),
                    }
                } else {
                    panic!(
                        "tried to access a field of something other than a compound {of:?} @ {index}"
                    );
                };

                self.assign(*to, value);
            }
            Instruction::AccessAssignRegisterRegister { to, index, value } => {
                let to = self.get_register(*to);

                let value = self.get_register(*value);

                if let CopyableValue::ValueIndex(value_index) = to {
                    match self.values.get_mut(usize::from(value_index)) {
                        Some(Value::Compound(fields) | Value::TaggedCompound { fields, .. }) => {
                            fields[usize::from(*index)] = value;
                        }
                        _ => panic!(
                            "tried to assign to a field of something other than a compound {to:?} @ {index}"
                        ),
                    }
                } else {
                    panic!(
                        "tried to assign to a field of something other than a compound {to:?} @ {index}"
                    );
                }
            }
            Instruction::AccessAssignRegisterValue { to, index, value } => {
                let to = self.get_register(*to);

                let value = self.dereference_value(*value, true);

                if let CopyableValue::ValueIndex(value_index) = to {
                    match self.values.get_mut(usize::from(value_index)) {
                        Some(Value::Compound(fields) | Value::TaggedCompound { fields, .. }) => {
                            fields[usize::from(*index)] = value;
                        }
                        _ => panic!(
                            "tried to assign to a field of something other than a compound {to:?} @ {index}"
                        ),
                    }
                } else {
                    panic!(
                        "tried to assign to a field of something other than a compound {to:?} @ {index}"
                    );
                }
            }
            Instruction::AccessAssignValueRegister { to, index, value } => {
                let to = self.dereference_value(*to, true);

                let value = self.get_register(*value);

                if let CopyableValue::ValueIndex(value_index) = to {
                    match self.values.get_mut(usize::from(value_index)) {
                        Some(Value::Compound(fields) | Value::TaggedCompound { fields, .. }) => {
                            fields[usize::from(*index)] = value;
                        }
                        _ => panic!(
                            "tried to assign to a field of something other than a compound {to:?} @ {index}"
                        ),
                    }
                } else {
                    panic!(
                        "tried to assign to a field of something other than a compound {to:?} @ {index}"
                    );
                }
            }
            Instruction::AccessAssignValueValue { to, index, value } => {
                let to = self.dereference_value(*to, true);

                let value = self.dereference_value(*value, true);

                if let CopyableValue::ValueIndex(value_index) = to {
                    match self.values.get_mut(usize::from(value_index)) {
                        Some(Value::Compound(fields) | Value::TaggedCompound { fields, .. }) => {
                            fields[usize::from(*index)] = value;
                        }
                        _ => panic!(
                            "tried to assign to a field of something other than a compound {to:?} @ {index}"
                        ),
                    }
                } else {
                    panic!(
                        "tried to assign to a field of something other than a compound {to:?} @ {index}"
                    );
                }
            }
            Instruction::GetTagRegister { to, of } => {
                let of = self.get_register(*of);

                let value = if let CopyableValue::ValueIndex(value_index) = of
                    && let Some(Value::TaggedCompound { tag, .. }) =
                        self.values.get(usize::from(value_index))
                {
                    CopyableValue::U16(*tag)
                } else {
                    panic!("tried to get the tag of something other than a tagged compound {of:?}");
                };

                self.assign(*to, value);
            }
            Instruction::GetTagValue { to, of } => {
                let of = self.dereference_value(*of, true);

                let value = if let CopyableValue::ValueIndex(value_index) = of
                    && let Some(Value::TaggedCompound { tag, .. }) =
                        self.values.get(usize::from(value_index))
                {
                    CopyableValue::U16(*tag)
                } else {
                    panic!("tried to get the tag of something other than a tagged compound {of:?}");
                };

                self.assign(*to, value);
            }
            Instruction::CallRegister { to, arity, callee } => {
                let callee = self.get_register(*callee);

                match callee {
                    CopyableValue::Fn {
                        address,
                        max_register_count,
                    } => {
                        if self.should_gc() {
                            self.gc();
                        }

                        let call_frame = CallFrame {
                            from: u32::try_from(self.ip)
                                .expect("return address was not a valid u32"),
                            fp: self.sp - usize::from(*arity),
                            max_register_count,
                            to: *to,
                        };

                        self.ip =
                            usize::try_from(address).expect("call address was not a valid usize");

                        self.fp = self.sp - usize::from(*arity);

                        self.call_frames.push(call_frame);

                        self.registers.resize_with(
                            self.registers.len() + usize::from(max_register_count),
                            || CopyableValue::Never,
                        );
                    }
                    CopyableValue::NativeFn(index) => {
                        let call_arguments =
                            self.registers[(self.sp - usize::from(*arity))..(self.sp)].to_vec();

                        self.sp -= usize::from(*arity);

                        let value =
                            self.native_fn_call(call_arguments.as_slice(), u16::from(index), out);

                        self.assign(*to, value);
                    }
                    _ => {
                        panic!("tried to call an uncallable {callee:?}");
                    }
                }
            }
            Instruction::CallValue { to, arity, callee } => {
                let callee = self.dereference_value(*callee, true);

                match callee {
                    CopyableValue::Fn {
                        address,
                        max_register_count,
                    } => {
                        if self.should_gc() {
                            self.gc();
                        }

                        let call_frame = CallFrame {
                            from: u32::try_from(self.ip)
                                .expect("return address was not a valid u32"),
                            fp: self.sp - usize::from(*arity),
                            max_register_count,
                            to: *to,
                        };

                        self.ip =
                            usize::try_from(address).expect("call address was not a valid usize");

                        self.fp = self.sp - usize::from(*arity);

                        self.call_frames.push(call_frame);

                        self.registers.resize_with(
                            self.registers.len() + usize::from(max_register_count),
                            || CopyableValue::Never,
                        );
                    }
                    CopyableValue::NativeFn(index) => {
                        let call_arguments =
                            self.registers[(self.sp - usize::from(*arity))..(self.sp)].to_vec();

                        self.sp -= usize::from(*arity);

                        let value =
                            self.native_fn_call(call_arguments.as_slice(), u16::from(index), out);

                        self.assign(*to, value);
                    }
                    _ => {
                        panic!("tried to call an uncallable {callee:?}");
                    }
                }
            }
            Instruction::Jump { destination } => {
                self.ip =
                    usize::try_from(*destination).expect("jump address was not a valid usize");
            }
            Instruction::BranchRegister {
                condition,
                when_true,
                otherwise,
            } => {
                let condition = self.get_register(*condition);

                match condition {
                    CopyableValue::Boolean(true) => {
                        self.ip = usize::try_from(*when_true)
                            .expect("jump address was not a valid usize");
                    }
                    CopyableValue::Boolean(false) => {
                        self.ip = usize::try_from(*otherwise)
                            .expect("jump address was not a valid usize");
                    }
                    _ => panic!("branch condition was not a boolean {condition:?}"),
                }
            }
            Instruction::BranchValue {
                condition,
                when_true,
                otherwise,
            } => {
                let condition = self.dereference_value(*condition, true);

                match condition {
                    CopyableValue::Boolean(true) => {
                        self.ip = usize::try_from(*when_true)
                            .expect("jump address was not a valid usize");
                    }
                    CopyableValue::Boolean(false) => {
                        self.ip = usize::try_from(*otherwise)
                            .expect("jump address was not a valid usize");
                    }
                    _ => panic!("branch condition was not a boolean {condition:?}"),
                }
            }
            Instruction::ReturnRegister { value } => {
                let value = self.get_register(*value);

                if let Some(call_frame) = self.call_frames.pop() {
                    self.ip = usize::try_from(call_frame.from)
                        .expect("return address was not a valid usize");

                    self.fp = self.call_frames.last().map_or(0, |frame| frame.fp);

                    self.sp = call_frame.fp;

                    self.registers.truncate(
                        self.registers.len() - usize::from(call_frame.max_register_count),
                    );

                    self.function_labels.pop();

                    if !self.call_frames.is_empty() {
                        self.assign(call_frame.to, value);
                    }
                }

                if self.should_gc() {
                    self.gc();
                }
            }
            Instruction::ReturnValue { value } => {
                let value = self.dereference_value(*value, true);

                if let Some(call_frame) = self.call_frames.pop() {
                    self.ip = usize::try_from(call_frame.from)
                        .expect("return address was not a valid usize");

                    self.fp = self.call_frames.last().map_or(0, |frame| frame.fp);

                    self.sp = call_frame.fp;

                    self.registers.truncate(
                        self.registers.len() - usize::from(call_frame.max_register_count),
                    );

                    self.function_labels.pop();

                    if !self.call_frames.is_empty() {
                        self.assign(call_frame.to, value);
                    }
                }

                if self.should_gc() {
                    self.gc();
                }
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
            | Instruction::Register
            | Instruction::Never => {
                panic!("got a value where an instruction was expected")
            }
        }

        Ok(())
    }

    fn assign(&mut self, to: Register, value: CopyableValue) {
        let offset = self.fp + usize::from(u16::from(to));

        self.registers[offset] = value;

        self.sp = self.sp.max(offset + 1);
    }

    #[allow(clippy::inline_always)]
    #[inline(always)]
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
                match self.values.get(usize::from(value_index)) {
                    None => panic!("tried to find a value that doesn't exist"),
                    Some(Value::Compound(values)) if make_constant => {
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
                    Some(Value::TaggedCompound {
                        fields: values,
                        tag,
                    }) if make_constant => {
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
                    Some(Value::String(text)) if make_constant => {
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
    #[must_use]
    fn native_fn_call<O>(
        &mut self,
        call_arguments: &[CopyableValue],
        native_fn: u16,
        out: &mut O,
    ) -> CopyableValue
    where
        O: Write,
    {
        let native_fn =
            NativeFn::try_from(native_fn).expect("tried to call an unknown native function");

        match native_fn {
            NativeFn::PrintU8 => {
                let CopyableValue::U8(value) = &call_arguments[0] else {
                    panic!("({native_fn:?} {:?} @ 0)", call_arguments[0]);
                };

                writeln!(out, "{value}").expect("failed to write to output");

                CopyableValue::Unit
            }
            NativeFn::PrintI8 => {
                let CopyableValue::I8(value) = &call_arguments[0] else {
                    panic!("({native_fn:?} {:?} @ 0)", call_arguments[0]);
                };

                writeln!(out, "{value}").expect("failed to write to output");

                CopyableValue::Unit
            }
            NativeFn::PrintU16 => {
                let CopyableValue::U16(value) = &call_arguments[0] else {
                    panic!("({native_fn:?} {:?} @ 0)", call_arguments[0]);
                };

                writeln!(out, "{value}").expect("failed to write to output");

                CopyableValue::Unit
            }
            NativeFn::PrintI16 => {
                let CopyableValue::I16(value) = &call_arguments[0] else {
                    panic!("({native_fn:?} {:?} @ 0)", call_arguments[0]);
                };

                writeln!(out, "{value}").expect("failed to write to output");

                CopyableValue::Unit
            }
            NativeFn::PrintU32 => {
                let CopyableValue::U32(value) = &call_arguments[0] else {
                    panic!("({native_fn:?} {:?} @ 0)", call_arguments[0]);
                };

                writeln!(out, "{value}").expect("failed to write to output");

                CopyableValue::Unit
            }
            NativeFn::PrintI32 => {
                let CopyableValue::I32(value) = &call_arguments[0] else {
                    panic!("({native_fn:?} {:?} @ 0)", call_arguments[0]);
                };

                writeln!(out, "{value}").expect("failed to write to output");

                CopyableValue::Unit
            }
            NativeFn::PrintU64 => {
                let CopyableValue::U64(value) = &call_arguments[0] else {
                    panic!("({native_fn:?} {:?} @ 0)", call_arguments[0]);
                };

                writeln!(out, "{value}").expect("failed to write to output");

                CopyableValue::Unit
            }
            NativeFn::PrintI64 => {
                let CopyableValue::I64(value) = &call_arguments[0] else {
                    panic!("({native_fn:?} {:?} @ 0)", call_arguments[0]);
                };

                writeln!(out, "{value}").expect("failed to write to output");

                CopyableValue::Unit
            }
            NativeFn::PrintF32 => {
                let CopyableValue::F32(value) = &call_arguments[0] else {
                    panic!("({native_fn:?} {:?} @ 0)", call_arguments[0]);
                };

                writeln!(out, "{value:?}").expect("failed to write to output");

                CopyableValue::Unit
            }
            NativeFn::PrintF64 => {
                let CopyableValue::F64(value) = &call_arguments[0] else {
                    panic!("({native_fn:?} {:?} @ 0)", call_arguments[0]);
                };

                writeln!(out, "{value:?}").expect("failed to write to output");

                CopyableValue::Unit
            }
            NativeFn::PrintBool => {
                let CopyableValue::Boolean(value) = &call_arguments[0] else {
                    panic!("({native_fn:?} {:?} @ 0)", call_arguments[0]);
                };

                writeln!(out, "{value}").expect("failed to write to output");

                CopyableValue::Unit
            }
            NativeFn::PrintUnit => {
                let CopyableValue::Unit = &call_arguments[0] else {
                    panic!("({native_fn:?} {:?} @ 0)", call_arguments[0]);
                };

                writeln!(out, "{{}}").expect("failed to write to output");

                CopyableValue::Unit
            }
            NativeFn::PrintString => {
                let CopyableValue::ValueIndex(value_index) = &call_arguments[0] else {
                    panic!("({native_fn:?} {:?} @ 0)", call_arguments[0]);
                };

                let Value::String(value) = &self.values[usize::from(*value_index)] else {
                    panic!(
                        "({native_fn:?} {:?} @ 0)",
                        self.values[usize::from(*value_index)]
                    );
                };

                writeln!(out, "{value}").expect("failed to write to output");

                CopyableValue::Unit
            }
            NativeFn::ExtendString => {
                let CopyableValue::ValueIndex(value_index) = &call_arguments[1] else {
                    panic!("({native_fn:?} {:?} @ 1)", call_arguments[1]);
                };

                let Value::String(extension) = &self.values[usize::from(*value_index)] else {
                    panic!(
                        "({native_fn:?} {:?} @ 1)",
                        self.values[usize::from(*value_index)]
                    );
                };

                let extension = extension.clone();

                let CopyableValue::ValueIndex(value_index) = &call_arguments[0] else {
                    panic!("({native_fn:?} {:?} @ 0)", call_arguments[0]);
                };

                let Value::String(value) = &mut self.values[usize::from(*value_index)] else {
                    panic!(
                        "({native_fn:?} {:?} @ 0)",
                        self.values[usize::from(*value_index)]
                    );
                };

                value.push_str(extension.as_str());

                CopyableValue::Unit
            }
            NativeFn::SliceString => {
                let CopyableValue::ValueIndex(value_index) = &call_arguments[0] else {
                    panic!("({native_fn:?} {:?} @ 0)", call_arguments[0]);
                };

                let Value::String(value) = &self.values[usize::from(*value_index)] else {
                    panic!(
                        "({native_fn:?} {:?} @ 0)",
                        self.values[usize::from(*value_index)]
                    );
                };

                let CopyableValue::U64(start) = &call_arguments[1] else {
                    panic!("({native_fn:?} {:?} @ 1)", call_arguments[1]);
                };

                let CopyableValue::U64(end) = &call_arguments[2] else {
                    panic!("({native_fn:?} {:?} @ 2)", call_arguments[2]);
                };

                let start = usize::try_from(*start).unwrap_or_else(|_| {
                    panic!("{native_fn:?}: argument 1 is not a valid usize");
                });

                let end = usize::try_from(*end).unwrap_or_else(|_| {
                    panic!("{native_fn:?}: argument 2 is not a valid usize");
                });

                if let Some(value) = value.get(start..end) {
                    self.values.push(Value::String(value.to_string()));

                    let value_index = CopyableValue::ValueIndex(ValueIndex(self.values.len() - 1));

                    self.allocated += self.size_of_value(value_index);

                    self.option_some(value_index)
                } else {
                    self.option_none()
                }
            }
            NativeFn::LengthString => {
                let CopyableValue::ValueIndex(value_index) = &call_arguments[0] else {
                    panic!("({native_fn:?} {:?} @ 0)", call_arguments[0]);
                };

                let Value::String(value) = &self.values[usize::from(*value_index)] else {
                    panic!(
                        "({native_fn:?} {:?} @ 0)",
                        self.values[usize::from(*value_index)]
                    );
                };

                let length =
                    u64::try_from(value.len()).expect("128-bit usize not allowed!  sorry!");

                CopyableValue::U64(length)
            }
            NativeFn::TruncateString => {
                let CopyableValue::ValueIndex(value_index) = &call_arguments[0] else {
                    panic!("({native_fn:?} {:?} @ 0)", call_arguments[0]);
                };

                let Value::String(value) = &mut self.values[usize::from(*value_index)] else {
                    panic!(
                        "({native_fn:?} {:?} @ 0)",
                        self.values[usize::from(*value_index)]
                    );
                };

                let CopyableValue::U64(end) = &call_arguments[1] else {
                    panic!("({native_fn:?} {:?} @ 1)", call_arguments[1]);
                };

                let end = usize::try_from(*end).unwrap_or_else(|_| {
                    panic!("{native_fn:?}: argument 1 is not a valid usize");
                });

                value.truncate(end);

                CopyableValue::Unit
            }
            NativeFn::FloorCharBoundaryString => {
                let CopyableValue::ValueIndex(value_index) = &call_arguments[0] else {
                    panic!("({native_fn:?} {:?} @ 0)", call_arguments[0]);
                };

                let Value::String(value) = &self.values[usize::from(*value_index)] else {
                    panic!(
                        "({native_fn:?} {:?} @ 0)",
                        self.values[usize::from(*value_index)]
                    );
                };

                let CopyableValue::U64(end) = &call_arguments[1] else {
                    panic!("({native_fn:?} {:?} @ 1)", call_arguments[1]);
                };

                let index = usize::try_from(*end).unwrap_or_else(|_| {
                    panic!("{native_fn:?}: argument 1 is not a valid usize");
                });

                let boundary =
                    u64::try_from(value.floor_char_boundary(index)).unwrap_or_else(|_| {
                        panic!("{native_fn:?}: char boundary is not a valid u64");
                    });

                CopyableValue::U64(boundary)
            }
            NativeFn::CeilCharBoundaryString => {
                let CopyableValue::ValueIndex(value_index) = &call_arguments[0] else {
                    panic!("({native_fn:?} {:?} @ 0)", call_arguments[0]);
                };

                let Value::String(value) = &self.values[usize::from(*value_index)] else {
                    panic!(
                        "({native_fn:?} {:?} @ 0)",
                        self.values[usize::from(*value_index)]
                    );
                };

                let CopyableValue::U64(end) = &call_arguments[1] else {
                    panic!("({native_fn:?} {:?} @ 1)", call_arguments[1]);
                };

                let index = usize::try_from(*end).unwrap_or_else(|_| {
                    panic!("{native_fn:?}: argument 1 is not a valid usize");
                });

                let boundary =
                    u64::try_from(value.ceil_char_boundary(index)).unwrap_or_else(|_| {
                        panic!("{native_fn:?}: char boundary is not a valid u64");
                    });

                CopyableValue::U64(boundary)
            }
            NativeFn::Clock => CopyableValue::F64(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("time somehow went backwards")
                    .as_secs_f64()
                    * 1000.0,
            ),
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
                    ) => {
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

        self.next_gc *= 2;
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
                Value::Compound(values) | Value::TaggedCompound { fields: values, .. } => {
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

        self.registers = self
            .registers
            .iter()
            .map(|value| self.retain_value(&mut values, *value, marked))
            .collect::<Vec<_>>();

        values.sort_by_key(|(index, _)| *index);
        values.dedup_by_key(|(index, _)| *index);

        for v in 0..(self.values.len()) {
            if marked.get(v).is_none_or(Option::is_none) {
                self.allocated = self
                    .allocated
                    .checked_sub(self.size_of_value(CopyableValue::ValueIndex(ValueIndex(v))))
                    .unwrap_or_else(|| {
                        panic!("UNDERFLOW");
                    });
            }
        }

        self.values = values
            .into_iter()
            .map(|(_, value)| value)
            .collect::<Vec<_>>();
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
            }
        } else {
            mem::size_of_val(&value)
        }
    }
}
