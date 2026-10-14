use crate::{
    low_ir::{BinaryOp, Instruction, Location, NativeFn, UnaryOp, ValueOrLocation},
    targets::vm::{CopyableValue, Instructive, Value, ValueIndex},
};

use std::{
    collections::HashSet,
    io::{BufRead, Write},
    mem,
};

struct CallFrame {
    fp: usize,
    from: usize,
    register_count: usize,
}

struct Vm {
    registers: Vec<CopyableValue>,
    values: Vec<Value>,
    allocated: usize,
    next_gc: usize,
    call_frames: Vec<CallFrame>,
    instructions: Vec<Instruction<CopyableValue>>,
    ip: usize,
    sp: usize,
    fp: usize,
}

impl Instructive for Vm {
    fn ip_mut(&mut self) -> &mut usize {
        &mut self.ip
    }

    fn instructions_mut(&mut self) -> &mut Vec<Instruction<CopyableValue>> {
        &mut self.instructions
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
/// - a `CopyableValue::Never` was passed to `Vm::dereference_value`, returning `Interrupt::Panic` (unreachable code was reached)
///
/// # Panics
/// Panics if passed a program that wasn't successfully compiled for the same version
pub fn run<O>(bytes: &[u8], out: &mut O) -> Result<(), Interrupt>
where
    O: Write,
{
    let registers = vec![
        const { CopyableValue::Never };
        bytes[..2]
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
            register_count: 0,
        }],
        ip: 0,
        sp: 0,
        fp: 0,
        instructions: vec![],
    };

    vm.read(bytes);

    vm.run(out)
}

macro_rules! binary_op_arithmetic {
    (
        $op:ident, $lhs:ident, $rhs:ident, $result:ident $(,)?
    ) => {{
        match $op {
            $crate::low_ir::BinaryOp::Multiply => {
                $crate::targets::vm::CopyableValue::$result($lhs * $rhs)
            }
            $crate::low_ir::BinaryOp::Divide => {
                $crate::targets::vm::CopyableValue::$result($lhs / $rhs)
            }
            $crate::low_ir::BinaryOp::Remainder => {
                $crate::targets::vm::CopyableValue::$result($lhs % $rhs)
            }
            $crate::low_ir::BinaryOp::Add => {
                $crate::targets::vm::CopyableValue::$result($lhs + $rhs)
            }
            $crate::low_ir::BinaryOp::Subtract => {
                $crate::targets::vm::CopyableValue::$result($lhs - $rhs)
            }
            $crate::low_ir::BinaryOp::Less => {
                $crate::targets::vm::CopyableValue::Boolean($lhs < $rhs)
            }
            $crate::low_ir::BinaryOp::Greater => {
                $crate::targets::vm::CopyableValue::Boolean($lhs > $rhs)
            }
            $crate::low_ir::BinaryOp::LessOrEqual => {
                $crate::targets::vm::CopyableValue::Boolean($lhs <= $rhs)
            }
            $crate::low_ir::BinaryOp::GreaterOrEqual => {
                $crate::targets::vm::CopyableValue::Boolean($lhs >= $rhs)
            }
            _ => unreachable!("only these opcodes get past the initial match arm"),
        }
    }};
}

impl Vm {
    fn value_to_string(&self, value: CopyableValue, seen: &mut HashSet<ValueIndex>) -> String {
        if let CopyableValue::ValueIndex(value_index) = value
            && let Some(value) = self.values.get(usize::from(value_index))
        {
            if seen.insert(value_index) {
                return format!("{value_index:?}");
            }

            match value {
                Value::MakeCompound(_)
                | Value::MakeTaggedCompound { .. }
                | Value::MakeString(_) => {
                    unreachable!("you shouldn't print constants");
                }
                Value::DebugNever { .. } => {
                    unreachable!("you shouldn't print the never type");
                }
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
                Value::String(_) => {
                    format!("{value:?}")
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

            for value in self.registers.iter().skip(self.fp) {
                buffer.push_str(", ");

                buffer.push_str(self.value_to_string(*value, &mut HashSet::new()).as_str());
            }
        }

        buffer.push(']');

        buffer
    }

    fn run<O>(&mut self, out: &mut O) -> Result<(), Interrupt>
    where
        O: Write,
    {
        while self.ip < self.instructions.len() {
            if cfg!(feature = "step") && !cfg!(test) {
                println!(
                    "@{}: {:?}",
                    self.ip,
                    self.instructions.get(self.ip).expect(
                        "we're within the bounds of the instructions vector, it will exist"
                    )
                );

                println!();

                println!("    {}", self.registers_to_string());

                println!();

                println!("    {:?}", self.sp);

                std::io::stdin().lock().lines().next();
            }

            self.step(out).inspect_err(|_| {
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
    pub fn step<O>(&mut self, out: &mut O) -> Result<(), Interrupt>
    where
        O: Write,
    {
        match self.instructions.get(self.ip) {
            None => return Ok(()),
            Some(Instruction::FnLabel { .. } | Instruction::NoOp) => {}
            Some(Instruction::Unary { op, operand, to }) => {
                let (op, operand, to) = (*op, *operand, *to);

                let operand = self.dereference_value(out, operand)?;

                match op {
                    UnaryOp::Not => match operand {
                        CopyableValue::Boolean(value) => {
                            self.assign(to, CopyableValue::Boolean(!value));
                        }
                        _ => panic!("incorrect argument for logical not ({op:?} {operand:?})"),
                    },
                    UnaryOp::Negate => match operand {
                        CopyableValue::I8(value) => {
                            self.assign(to, CopyableValue::I8(-value));
                        }
                        CopyableValue::I16(value) => {
                            self.assign(to, CopyableValue::I16(-value));
                        }
                        CopyableValue::I32(value) => {
                            self.assign(to, CopyableValue::I32(-value));
                        }
                        CopyableValue::I64(value) => {
                            self.assign(to, CopyableValue::I64(-value));
                        }
                        CopyableValue::F32(value) => {
                            self.assign(to, CopyableValue::F32(-value));
                        }
                        CopyableValue::F64(value) => {
                            self.assign(to, CopyableValue::F64(-value));
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
                        | CopyableValue::Never => {
                            panic!("incorrect argument for negate ({op:?} {operand:?})")
                        }
                    },
                }
            }
            Some(Instruction::Binary { op, lhs, rhs, to }) => {
                let (op, lhs, rhs, to) = (*op, *lhs, *rhs, *to);

                match op {
                    BinaryOp::Multiply
                    | BinaryOp::Divide
                    | BinaryOp::Remainder
                    | BinaryOp::Add
                    | BinaryOp::Subtract
                    | BinaryOp::Less
                    | BinaryOp::Greater
                    | BinaryOp::LessOrEqual
                    | BinaryOp::GreaterOrEqual => {
                        let lhs = self.dereference_value(out, lhs)?;
                        let rhs = self.dereference_value(out, rhs)?;

                        match (lhs, rhs) {
                            (CopyableValue::U8(lhs), CopyableValue::U8(rhs)) => {
                                self.assign(to, binary_op_arithmetic!(op, lhs, rhs, U8));
                            }
                            (CopyableValue::I8(lhs), CopyableValue::I8(rhs)) => {
                                self.assign(to, binary_op_arithmetic!(op, lhs, rhs, I8));
                            }
                            (CopyableValue::U16(lhs), CopyableValue::U16(rhs)) => {
                                self.assign(to, binary_op_arithmetic!(op, lhs, rhs, U16));
                            }
                            (CopyableValue::I16(lhs), CopyableValue::I16(rhs)) => {
                                self.assign(to, binary_op_arithmetic!(op, lhs, rhs, I16));
                            }
                            (CopyableValue::U32(lhs), CopyableValue::U32(rhs)) => {
                                self.assign(to, binary_op_arithmetic!(op, lhs, rhs, U32));
                            }
                            (CopyableValue::I32(lhs), CopyableValue::I32(rhs)) => {
                                self.assign(to, binary_op_arithmetic!(op, lhs, rhs, I32));
                            }
                            (CopyableValue::U64(lhs), CopyableValue::U64(rhs)) => {
                                self.assign(to, binary_op_arithmetic!(op, lhs, rhs, U64));
                            }
                            (CopyableValue::I64(lhs), CopyableValue::I64(rhs)) => {
                                self.assign(to, binary_op_arithmetic!(op, lhs, rhs, I64));
                            }
                            (CopyableValue::F32(lhs), CopyableValue::F32(rhs)) => {
                                self.assign(to, binary_op_arithmetic!(op, lhs, rhs, F32));
                            }
                            (CopyableValue::F64(lhs), CopyableValue::F64(rhs)) => {
                                self.assign(to, binary_op_arithmetic!(op, lhs, rhs, F64));
                            }
                            (
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
                                | CopyableValue::Boolean(_)
                                | CopyableValue::Unit
                                | CopyableValue::Fn { .. }
                                | CopyableValue::NativeFn(_)
                                | CopyableValue::ValueIndex(_)
                                | CopyableValue::Never,
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
                                | CopyableValue::Boolean(_)
                                | CopyableValue::Unit
                                | CopyableValue::Fn { .. }
                                | CopyableValue::NativeFn(_)
                                | CopyableValue::ValueIndex(_)
                                | CopyableValue::Never,
                            ) => {
                                panic!("incorrect argument for arithmetic ({op:?} {lhs:?} {rhs:?})")
                            }
                        }
                    }
                    BinaryOp::And | BinaryOp::Or => {
                        let lhs = self.dereference_value(out, lhs)?;
                        let rhs = self.dereference_value(out, rhs)?;

                        match (lhs, rhs) {
                            (CopyableValue::Boolean(lhs), CopyableValue::Boolean(rhs)) => self
                                .assign(
                                    to,
                                    CopyableValue::Boolean(match op {
                                        BinaryOp::And => lhs && rhs,
                                        BinaryOp::Or => lhs || rhs,
                                        _ => unreachable!(
                                            "only these opcodes get past the initial match arm"
                                        ),
                                    }),
                                ),
                            _ => panic!("incorrect argument for logic ({op:?} {lhs:?} {rhs:?})"),
                        }
                    }
                    BinaryOp::Equal | BinaryOp::NotEqual => {
                        let lhs = self.dereference_value(out, lhs)?;
                        let rhs = self.dereference_value(out, rhs)?;

                        self.assign(
                            to,
                            match op {
                                BinaryOp::Equal => CopyableValue::Boolean(self.values_eq(lhs, rhs)),
                                BinaryOp::NotEqual => {
                                    CopyableValue::Boolean(!self.values_eq(lhs, rhs))
                                }
                                _ => unreachable!(
                                    "only these opcodes get past the initial match arm"
                                ),
                            },
                        );
                    }
                }
            }
            Some(Instruction::Assign { to, value }) => {
                let value = *value;
                let to = *to;

                let value = self.dereference_value(out, value)?;

                self.assign(to, value);
            }
            Some(Instruction::PopN(count)) => {
                self.sp -= *count;
            }
            Some(Instruction::Push(value)) => {
                let value = *value;

                let value = self.dereference_value(out, value)?;

                self.registers[self.sp] = value;

                self.sp += 1;
            }
            Some(Instruction::Access { index, of, to }) => {
                let index = *index;
                let of = *of;
                let to = *to;

                let of = self.dereference_value(out, of)?;

                let value = if let CopyableValue::ValueIndex(value_index) = of {
                    match self.values.get(usize::from(value_index)) {
                        Some(Value::Compound(fields) | Value::TaggedCompound { fields, .. }) => {
                            fields.get(usize::from(index)).map_or_else(
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

                self.assign(to, value);
            }
            Some(Instruction::AccessAssign { index, of, value }) => {
                let index = *index;
                let of = *of;
                let value = *value;

                let value = self.dereference_value(out, value)?;

                let of = self.dereference_value(out, of)?;

                if let CopyableValue::ValueIndex(value_index) = of {
                    match self.values.get_mut(usize::from(value_index)) {
                        Some(Value::Compound(fields) | Value::TaggedCompound { fields, .. }) => {
                            if let Some(field) = fields.get_mut(usize::from(index)) {
                                *field = value;
                            } else {
                                panic!(
                                    "tried to assign to a field that doesn't exist {of:?} @ {index}"
                                )
                            }
                        }
                        _ => panic!(
                            "tried to assign to a field of something other than a compound {of:?} @ {index}"
                        ),
                    }
                } else {
                    panic!(
                        "tried to assign to a field of something other than a compound {of:?} @ {index}"
                    );
                }
            }
            Some(Instruction::GetTag { of, to }) => {
                let of = *of;
                let to = *to;

                let of = self.dereference_value(out, of)?;

                let value = if let CopyableValue::ValueIndex(value_index) = of
                    && let Some(Value::TaggedCompound { tag, .. }) =
                        self.values.get(usize::from(value_index))
                {
                    CopyableValue::U16(*tag)
                } else {
                    panic!("tried to get the tag of something other than a tagged compound {of:?}");
                };

                self.assign(to, value);
            }
            Some(Instruction::Call { callee, arity, to }) => {
                let callee = *callee;
                let arity = *arity;
                let to = *to;

                match self.dereference_value(out, callee)? {
                    CopyableValue::Fn {
                        address,
                        max_register_count,
                    } => {
                        if self.should_gc() {
                            self.gc();
                        }

                        let call_frame = CallFrame {
                            from: self.ip,
                            fp: self.sp - usize::from(arity),
                            register_count: self.registers.len(),
                        };

                        self.ip = address;

                        self.fp = self.sp - usize::from(arity);

                        self.call_frames.push(call_frame);

                        self.registers.resize_with(
                            self.registers.len()
                                + usize::try_from(max_register_count)
                                    .expect("a function's max register count was not a valid u32"),
                            || CopyableValue::Never,
                        );

                        return Ok(());
                    }
                    CopyableValue::NativeFn(index) => {
                        let call_arguments =
                            self.registers[(self.sp - usize::from(arity))..(self.sp)].to_vec();

                        self.sp -= usize::from(arity);

                        let value =
                            self.native_fn_call(call_arguments.as_slice(), u16::from(index), out);

                        self.assign(to, value);
                    }
                    _ => {
                        panic!("tried to call an uncallable");
                    }
                }
            }
            Some(Instruction::Jump(destination)) => {
                let destination = *destination;

                self.ip = usize::from(destination);

                return Ok(());
            }
            Some(Instruction::Branch {
                condition,
                when_true,
                otherwise,
            }) => {
                let (condition, when_true, otherwise) = (*condition, *when_true, *otherwise);

                match self.dereference_value(out, condition)? {
                    CopyableValue::Boolean(true) => {
                        self.ip = usize::from(when_true);
                    }
                    CopyableValue::Boolean(false) => {
                        self.ip = usize::from(otherwise);
                    }
                    value => panic!("branch condition was not a boolean {value:?}"),
                }

                return Ok(());
            }
            Some(Instruction::Return(value)) => {
                let value = *value;

                let value = self.dereference_value(out, value)?;

                if let Some(call_frame) = self.call_frames.pop() {
                    self.ip = call_frame.from;

                    self.fp = self.call_frames.last().map_or(0, |frame| frame.fp);

                    self.sp = call_frame.fp;

                    self.registers.truncate(call_frame.register_count);

                    if !self.call_frames.is_empty()
                        && let Some(Instruction::Call { to, .. }) = self.instructions.get(self.ip)
                    {
                        self.assign(*to, value);
                    }
                }

                if self.should_gc() {
                    self.gc();
                }
            }
        }

        self.ip += 1;

        Ok(())
    }

    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn assign(&mut self, to: Location, value: CopyableValue) {
        match to {
            Location::Register(offset) => {
                let offset = self.fp + usize::from(u16::from(offset));

                self.registers[offset] = value;

                self.sp = self.sp.max(offset + 1);
            }
        }
    }

    fn dereference_value_recursive<O>(
        &mut self,
        out: &mut O,
        value: ValueOrLocation<CopyableValue>,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        self.dereference_value(out, value)
    }

    fn panic<O>(&mut self, out: &mut O, location: Option<(usize, usize)>) -> Result<(), Interrupt>
    where
        O: Write,
    {
        if let Some(Instruction::FnLabel {
            file_name,
            function_label,
        }) = self.instructions[..(self.call_frames.last().map_or(self.ip, |frame| frame.from))]
            .iter()
            .rfind(|instruction| matches!(instruction, Instruction::FnLabel { .. }))
        {
            if let Some((line_number, column)) = location {
                writeln!(
                    out,
                    "VM panicked at {file_name}:{line_number}:{column} in {function_label}"
                )
            } else {
                writeln!(out, "VM panicked at {file_name} in {function_label}")
            }
        } else {
            writeln!(out, "VM panicked")
        }
        .expect("failed to write to output");

        self.ip = self.instructions.len();

        Err(Interrupt::Panic)
    }

    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn dereference_value<O>(
        &mut self,
        out: &mut O,
        value: ValueOrLocation<CopyableValue>,
    ) -> Result<CopyableValue, Interrupt>
    where
        O: Write,
    {
        match value {
            ValueOrLocation::Value(value @ CopyableValue::Never) => {
                self.panic(out, None)?;

                Ok(value)
            }
            ValueOrLocation::At(Location::Register(offset)) => {
                Ok(self.call_frames.last().map_or_else(
                    || self.registers[usize::from(u16::from(offset))],
                    |frame| self.registers[frame.fp + usize::from(u16::from(offset))],
                ))
            }
            ValueOrLocation::Value(value @ CopyableValue::ValueIndex(value_index)) => {
                match self.values.get(usize::from(value_index)) {
                    None => panic!("tried to find a value that doesn't exist"),
                    Some(Value::DebugNever {
                        line_number,
                        column,
                    }) => {
                        self.panic(out, Some((*line_number, *column)))?;

                        Ok(value)
                    }
                    Some(Value::MakeCompound(fields)) => {
                        let fields =
                            fields
                                .clone()
                                .into_iter()
                                .try_fold(vec![], |mut accum, value| {
                                    accum.push(self.dereference_value_recursive(out, value)?);
                                    Ok(accum)
                                })?;

                        self.values.push(Value::Compound(fields));

                        let value_index =
                            CopyableValue::ValueIndex(ValueIndex(self.values.len() - 1));

                        self.allocated += self.size_of_value(value_index);

                        Ok(value_index)
                    }
                    Some(Value::MakeTaggedCompound { fields, tag }) => {
                        let tag = *tag;

                        let fields =
                            fields
                                .clone()
                                .into_iter()
                                .try_fold(vec![], |mut accum, value| {
                                    accum.push(self.dereference_value_recursive(out, value)?);
                                    Ok(accum)
                                })?;

                        self.values.push(Value::TaggedCompound { fields, tag });

                        let value_index =
                            CopyableValue::ValueIndex(ValueIndex(self.values.len() - 1));

                        self.allocated += self.size_of_value(value_index);

                        Ok(value_index)
                    }
                    Some(Value::MakeString(text)) => {
                        self.values.push(Value::String(text.clone()));

                        let value_index =
                            CopyableValue::ValueIndex(ValueIndex(self.values.len() - 1));

                        self.allocated += self.size_of_value(value_index);

                        Ok(value_index)
                    }
                    Some(Value::Compound(_) | Value::TaggedCompound { .. } | Value::String(_)) => {
                        Ok(CopyableValue::ValueIndex(value_index))
                    }
                }
            }
            ValueOrLocation::Value(value) => Ok(value),
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

        #[allow(clippy::branches_sharing_code)]
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
            (CopyableValue::Unit, CopyableValue::Unit) => lhs == rhs,
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

        for v in (0..(self.values.len())).filter(|v| {
            matches!(
                self.values.get(*v),
                Some(
                    Value::MakeCompound(_)
                        | Value::MakeTaggedCompound { .. }
                        | Value::MakeString(_)
                        | Value::DebugNever { .. }
                )
            )
        }) {
            let value = CopyableValue::ValueIndex(ValueIndex(v));

            self.mark_value(&mut marked, value, &mut marked_count);
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
                Value::MakeCompound(_)
                | Value::MakeTaggedCompound { .. }
                | Value::MakeString(_)
                | Value::String(_)
                | Value::DebugNever { .. } => {}
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
                        Value::MakeCompound(_)
                        | Value::MakeTaggedCompound { .. }
                        | Value::MakeString(_)
                        | Value::DebugNever { .. } => {}
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
                Value::MakeCompound(_)
                | Value::MakeTaggedCompound { .. }
                | Value::MakeString(_)
                | Value::DebugNever { .. } => 0,
            }
        } else {
            mem::size_of_val(&value)
        }
    }
}
