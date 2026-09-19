use crate::targets::vm::{NativeFn, OpCode, TypeId};

use std::{io::Write, mem};

struct CallFrame {
    fp: usize,
    from: usize,
    previous_registers: Vec<CopyableValue>,
}

struct Vm {
    stack: Vec<CopyableValue>,
    values: Vec<Value>,
    allocated: usize,
    next_gc: usize,
    max_registers: usize,
    registers: Vec<CopyableValue>,
    call_frames: Vec<CallFrame>,
    ip: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CopyableValue {
    I64(i64),
    F64(f64),
    Boolean(bool),
    Unit,
    Fn(usize),
    NativeFn(u16),
    Register(usize),
    StackOffset(usize),
    ValueIndex(ValueIndex),
}

#[derive(Debug)]
enum Value {
    Compound(Vec<CopyableValue>),
    TaggedCompound {
        fields: Vec<CopyableValue>,
        tag: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct ValueIndex(usize);

impl From<ValueIndex> for usize {
    fn from(value: ValueIndex) -> Self {
        value.0
    }
}

trait Deserialize {
    fn from_bytes(bytes: &[u8], vm: &mut Vm) -> Self;
}

/// # Panics
/// Panics if passed a program that wasn't successfully compiled for the same version
pub fn run<O>(bytes: &[u8], out: &mut O)
where
    O: Write,
{
    let max_registers = bytes[..8]
        .as_array::<8>()
        .map(|array| u64::from_le_bytes(*array))
        .and_then(|max_registers| usize::try_from(max_registers).ok())
        .expect("maximum registers is unknown");

    let mut evaluator = Vm {
        stack: vec![],
        values: vec![],
        allocated: 0,
        next_gc: 1_000_000,
        max_registers,
        registers: vec![const { CopyableValue::Unit }; max_registers],
        call_frames: vec![CallFrame {
            fp: 0,
            from: 0,
            previous_registers: vec![],
        }],
        ip: bytes[8..16]
            .as_array::<8>()
            .map(|array| u64::from_le_bytes(*array))
            .and_then(|ip| usize::try_from(ip).ok())
            .expect("entrypoint is unknown"),
    };

    evaluator.run(bytes, out);
}

impl Vm {
    #[allow(clippy::too_many_lines)]
    fn run<O>(&mut self, bytes: &[u8], out: &mut O)
    where
        O: Write,
    {
        while self.ip < bytes.len() {
            let op_code = <OpCode as Deserialize>::from_bytes(bytes, self);

            match op_code {
                OpCode::Not => {
                    let to = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let operand = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    match self.dereference_value(operand) {
                        CopyableValue::Boolean(value) => {
                            self.assign(to, CopyableValue::Boolean(!value));
                        }
                        _ => panic!("incorrect argument for logical not"),
                    }
                }
                OpCode::Negate => {
                    let to = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let operand = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    match self.dereference_value(operand) {
                        CopyableValue::I64(value) => self.assign(to, CopyableValue::I64(-value)),
                        CopyableValue::F64(value) => self.assign(to, CopyableValue::F64(-value)),
                        _ => panic!("incorrect argument for negate"),
                    }
                }
                OpCode::Multiply
                | OpCode::Divide
                | OpCode::Remainder
                | OpCode::Add
                | OpCode::Subtract
                | OpCode::Less
                | OpCode::Greater
                | OpCode::LessOrEqual
                | OpCode::GreaterOrEqual => {
                    let to = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let lhs = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let rhs = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let lhs = self.dereference_value(lhs);
                    let rhs = self.dereference_value(rhs);

                    match (lhs, rhs) {
                        (CopyableValue::I64(lhs), CopyableValue::I64(rhs)) => self.assign(
                            to,
                            match op_code {
                                OpCode::Multiply => CopyableValue::I64(lhs * rhs),
                                OpCode::Divide => CopyableValue::I64(lhs / rhs),
                                OpCode::Remainder => CopyableValue::I64(lhs % rhs),
                                OpCode::Add => CopyableValue::I64(lhs + rhs),
                                OpCode::Subtract => CopyableValue::I64(lhs - rhs),
                                OpCode::Less => CopyableValue::Boolean(lhs < rhs),
                                OpCode::Greater => CopyableValue::Boolean(lhs > rhs),
                                OpCode::LessOrEqual => CopyableValue::Boolean(lhs <= rhs),
                                OpCode::GreaterOrEqual => CopyableValue::Boolean(lhs >= rhs),
                                _ => unreachable!(
                                    "only these opcodes get past the initial match arm"
                                ),
                            },
                        ),
                        (CopyableValue::F64(lhs), CopyableValue::F64(rhs)) => self.assign(
                            to,
                            match op_code {
                                OpCode::Multiply => CopyableValue::F64(lhs * rhs),
                                OpCode::Divide => CopyableValue::F64(lhs / rhs),
                                OpCode::Remainder => CopyableValue::F64(lhs % rhs),
                                OpCode::Add => CopyableValue::F64(lhs + rhs),
                                OpCode::Subtract => CopyableValue::F64(lhs - rhs),
                                OpCode::Less => CopyableValue::Boolean(lhs < rhs),
                                OpCode::Greater => CopyableValue::Boolean(lhs > rhs),
                                OpCode::LessOrEqual => CopyableValue::Boolean(lhs <= rhs),
                                OpCode::GreaterOrEqual => CopyableValue::Boolean(lhs >= rhs),
                                _ => unreachable!(
                                    "only these opcodes get past the initial match arm"
                                ),
                            },
                        ),
                        _ => panic!("incorrect argument for arithmetic"),
                    }
                }
                OpCode::And | OpCode::Or => {
                    let to = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let lhs = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let rhs = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let lhs = self.dereference_value(lhs);
                    let rhs = self.dereference_value(rhs);

                    match (lhs, rhs) {
                        (CopyableValue::Boolean(lhs), CopyableValue::Boolean(rhs)) => self.assign(
                            to,
                            CopyableValue::Boolean(match op_code {
                                OpCode::And => lhs && rhs,
                                OpCode::Or => lhs || rhs,
                                _ => unreachable!(
                                    "only these opcodes get past the initial match arm"
                                ),
                            }),
                        ),
                        _ => panic!("incorrect argument for logic"),
                    }
                }
                OpCode::Equal | OpCode::NotEqual => {
                    let to = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let lhs = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let rhs = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let lhs = self.dereference_value(lhs);
                    let rhs = self.dereference_value(rhs);

                    self.assign(
                        to,
                        match op_code {
                            OpCode::Equal => CopyableValue::Boolean(self.values_eq(lhs, rhs)),
                            OpCode::NotEqual => CopyableValue::Boolean(!self.values_eq(lhs, rhs)),
                            _ => unreachable!("only these opcodes get past the initial match arm"),
                        },
                    );
                }
                OpCode::Assign => {
                    let to = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let value = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let value = self.dereference_value(value);

                    self.assign(to, value);
                }
                OpCode::Push => {
                    let value = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let value = self.dereference_value(value);

                    self.stack.push(value);
                }
                OpCode::Access => {
                    let to = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let index = bytes[(self.ip)..(self.ip + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|index| usize::try_from(index).ok())
                        .expect("access index is not a valid usize");

                    self.ip += 8;

                    let of = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let value = if let CopyableValue::ValueIndex(value_index) =
                        self.dereference_value(of)
                        && let Some(Value::Compound(fields)) =
                            self.values.get(usize::from(value_index))
                    {
                        fields[index]
                    } else {
                        panic!("tried to access something other than a compound");
                    };

                    let value = self.dereference_value(value);

                    self.assign(to, value);
                }
                OpCode::AccessAssign => {
                    let index = bytes[(self.ip)..(self.ip + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|index| usize::try_from(index).ok())
                        .expect("access index is not a valid usize");

                    self.ip += 8;

                    let of = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let value = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let value = self.dereference_value(value);

                    if let CopyableValue::ValueIndex(value_index) = self.dereference_value(of)
                        && let Some(Value::Compound(fields)) =
                            self.values.get_mut(usize::from(value_index))
                        && let Some(field) = fields.get_mut(index)
                    {
                        *field = value;
                    } else {
                        panic!("tried to assign to an invalid compound field");
                    }
                }
                OpCode::Call => {
                    let callee = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let arity = bytes[(self.ip)..(self.ip + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|arity| usize::try_from(arity).ok())
                        .expect("call arity is not a valid usize");

                    self.ip += 8;

                    match self.dereference_value(callee) {
                        CopyableValue::Fn(callee) => {
                            if self.should_gc() {
                                self.gc();
                            }

                            let call_frame = CallFrame {
                                from: self.ip,
                                fp: self.stack.len() - arity,
                                previous_registers: Vec::with_capacity(self.max_registers),
                            };

                            self.ip = callee;

                            self.call_frames.push(call_frame);
                        }
                        CopyableValue::NativeFn(index) => {
                            let call_arguments = self.stack.split_off(self.stack.len() - arity);

                            let value = Self::native_fn_call(call_arguments.as_slice(), index, out);

                            let to = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                            let value = self.dereference_value(value);

                            self.assign(to, value);
                        }
                        _ => {
                            panic!("tried to call an uncallable");
                        }
                    }
                }
                OpCode::Jump => {
                    let to = bytes[(self.ip)..(self.ip + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|to| usize::try_from(to).ok())
                        .expect("jump address is not a valid usize");

                    self.ip += 8;

                    let argument_count = bytes[(self.ip)..(self.ip + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|argument_count| usize::try_from(argument_count).ok())
                        .expect("jump address is not a valid usize");

                    self.ip += 8;

                    let mut arguments = vec![];

                    for _ in 0..argument_count {
                        let to = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                        let value = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                        arguments.push((to, self.dereference_value(value)));
                    }

                    for (to, argument) in arguments {
                        self.assign(to, argument);
                    }

                    self.ip = bytes[to..(to + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|ip| usize::try_from(ip).ok())
                        .expect("branch address is not a valid usize");
                }
                OpCode::Branch => {
                    let condition = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let when_true = bytes[(self.ip)..(self.ip + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|to| usize::try_from(to).ok())
                        .expect("jump address is not a valid usize");

                    self.ip += 8;

                    let otherwise = bytes[(self.ip)..(self.ip + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|to| usize::try_from(to).ok())
                        .expect("jump address is not a valid usize");

                    self.ip += 8;

                    match self.dereference_value(condition) {
                        CopyableValue::Boolean(true) => {
                            let argument_count = bytes[(self.ip)..(self.ip + 8)]
                                .as_array::<8>()
                                .map(|array| u64::from_le_bytes(*array))
                                .and_then(|argument_count| usize::try_from(argument_count).ok())
                                .expect("jump address is not a valid usize");

                            self.ip += 8;

                            let mut arguments = vec![];

                            for _ in 0..argument_count {
                                let to = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                                let value = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                                arguments.push((to, self.dereference_value(value)));
                            }

                            for (to, argument) in arguments {
                                self.assign(to, argument);
                            }

                            self.ip = bytes[when_true..(when_true + 8)]
                                .as_array::<8>()
                                .map(|array| u64::from_le_bytes(*array))
                                .and_then(|ip| usize::try_from(ip).ok())
                                .expect("branch address is not a valid usize");
                        }
                        CopyableValue::Boolean(false) => {
                            let argument_count = bytes[(self.ip)..(self.ip + 8)]
                                .as_array::<8>()
                                .map(|array| u64::from_le_bytes(*array))
                                .and_then(|argument_count| usize::try_from(argument_count).ok())
                                .expect("jump address is not a valid usize");

                            self.ip += 8;

                            for _ in 0..argument_count {
                                let _ = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                                let _ = <CopyableValue as Deserialize>::from_bytes(bytes, self);
                            }

                            let argument_count = bytes[(self.ip)..(self.ip + 8)]
                                .as_array::<8>()
                                .map(|array| u64::from_le_bytes(*array))
                                .and_then(|argument_count| usize::try_from(argument_count).ok())
                                .expect("jump address is not a valid usize");

                            self.ip += 8;

                            let mut arguments = vec![];

                            for _ in 0..argument_count {
                                let to = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                                let value = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                                arguments.push((to, self.dereference_value(value)));
                            }

                            for (to, argument) in arguments {
                                self.assign(to, argument);
                            }

                            self.ip = bytes[otherwise..(otherwise + 8)]
                                .as_array::<8>()
                                .map(|array| u64::from_le_bytes(*array))
                                .and_then(|ip| usize::try_from(ip).ok())
                                .expect("branch address is not a valid usize");
                        }
                        _ => panic!("branch condition was not a boolean"),
                    }
                }
                OpCode::Return => {
                    let value = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                    let value = self.dereference_value(value);

                    if let Some(call_frame) = self.call_frames.pop() {
                        self.ip = call_frame.from;

                        self.stack.truncate(call_frame.fp);

                        self.registers[..(call_frame.previous_registers.len())]
                            .copy_from_slice(call_frame.previous_registers.as_slice());

                        if !self.call_frames.is_empty() {
                            let to = <CopyableValue as Deserialize>::from_bytes(bytes, self);

                            self.assign(to, value);
                        }
                    }

                    if self.should_gc() {
                        self.gc();
                    }

                    if self.call_frames.is_empty() {
                        break;
                    }
                }
            }
        }

        self.gc();

        assert_eq!(self.allocated, 0, "{} BYTES LEAKED", self.allocated);
    }

    fn assign(&mut self, to: CopyableValue, value: CopyableValue) {
        let value = self.dereference_value(value);

        match to {
            CopyableValue::Register(index) => {
                if let Some(call_frame) = self.call_frames.last_mut()
                    && index >= call_frame.previous_registers.len()
                {
                    let old_value = self.registers[index];

                    call_frame.previous_registers.push(old_value);
                }

                self.registers[index] = value;
            }
            CopyableValue::StackOffset(offset) => {
                if let Some(stack_value) = self
                    .call_frames
                    .last()
                    .map(|frame| frame.fp + offset)
                    .and_then(|offset| self.stack.get_mut(offset))
                {
                    *stack_value = value;
                } else {
                    self.stack.push(value);
                }
            }
            _ => panic!("only registers and stack slots can be assigned to"),
        }
    }

    fn dereference_value(&self, value: CopyableValue) -> CopyableValue {
        match value {
            CopyableValue::Register(index) => self.dereference_value(self.registers[index]),
            CopyableValue::StackOffset(offset) => self
                .call_frames
                .last()
                .map(|frame| frame.fp + offset)
                .and_then(|offset| self.stack.get(offset))
                .map_or_else(
                    || panic!("the stack is too short"),
                    |stack_value| self.dereference_value(*stack_value),
                ),
            _ => value,
        }
    }

    #[must_use]
    fn native_fn_call<O>(
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
            NativeFn::PrintI64 => {
                let CopyableValue::I64(value) = &call_arguments[0] else {
                    panic!("({native_fn:?} {:?} @ 0)", call_arguments[0]);
                };

                writeln!(out, "{value}").expect("failed to write to output");

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
        }
    }

    fn values_eq(&self, lhs: CopyableValue, rhs: CopyableValue) -> bool {
        match (lhs, rhs) {
            (CopyableValue::I64(lhs), CopyableValue::I64(rhs)) => lhs == rhs,
            (CopyableValue::F64(lhs), CopyableValue::F64(rhs)) => lhs == rhs,
            (CopyableValue::Boolean(lhs), CopyableValue::Boolean(rhs)) => lhs == rhs,
            (CopyableValue::Unit, CopyableValue::Unit) => lhs == rhs,
            (CopyableValue::Fn(lhs), CopyableValue::Fn(rhs)) => lhs == rhs,
            (CopyableValue::NativeFn(lhs), CopyableValue::NativeFn(rhs)) => lhs == rhs,
            (CopyableValue::ValueIndex(lhs), CopyableValue::ValueIndex(rhs)) => {
                match (
                    &self.values[usize::from(lhs)],
                    &self.values[usize::from(rhs)],
                ) {
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
                    (_, _) => false,
                }
            }
            (_, _) => false,
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

        for value in &self.stack {
            self.mark_value(&mut marked, *value, &mut marked_count);
        }

        for value in &self.registers {
            self.mark_value(&mut marked, *value, &mut marked_count);
        }

        for call_frame in &self.call_frames {
            for previous_register in &call_frame.previous_registers {
                self.mark_value(&mut marked, *previous_register, &mut marked_count);
            }
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

        self.stack = self
            .stack
            .iter()
            .map(|value| self.retain_value(&mut values, *value, marked))
            .collect::<Vec<_>>();

        self.registers = self
            .registers
            .iter()
            .map(|value| self.retain_value(&mut values, *value, marked))
            .collect::<Vec<_>>();

        for c in 0..(self.call_frames.len()) {
            let Some(call_frame) = self.call_frames.get(c) else {
                unreachable!("the call frame will exist");
            };

            let previous_registers = call_frame
                .previous_registers
                .iter()
                .map(|previous_register| self.retain_value(&mut values, *previous_register, marked))
                .collect::<Vec<_>>();

            let Some(call_frame) = self.call_frames.get_mut(c) else {
                unreachable!("the call frame will exist");
            };

            call_frame.previous_registers = previous_registers;
        }

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
            }
        } else {
            mem::size_of_val(&value)
        }
    }

    fn push_value(&mut self, value: Value) -> CopyableValue {
        let value_index = CopyableValue::ValueIndex(ValueIndex(self.values.len()));

        self.values.push(value);

        value_index
    }
}

impl Deserialize for OpCode {
    fn from_bytes(bytes: &[u8], vm: &mut Vm) -> Self {
        match Self::try_from(
            bytes[{
                let ip = vm.ip;
                vm.ip += 1;
                ip
            }],
        ) {
            Ok(op_code) => op_code,
            Err(error) => panic!("unknown opcode {error}"),
        }
    }
}

impl Deserialize for CopyableValue {
    #[allow(clippy::too_many_lines)]
    fn from_bytes(bytes: &[u8], vm: &mut Vm) -> Self {
        match TypeId::try_from(
            bytes[{
                let ip = vm.ip;
                vm.ip += 1;
                ip
            }],
        ) {
            Ok(TypeId::I64) => Self::I64(i64::from_le_bytes(
                *bytes[(vm.ip)..{
                    vm.ip += 8;
                    vm.ip
                }]
                    .as_array::<8>()
                    .expect("the range is the same length as the expected array"),
            )),
            Ok(TypeId::F64) => Self::F64(f64::from_le_bytes(
                *bytes[(vm.ip)..{
                    vm.ip += 8;
                    vm.ip
                }]
                    .as_array::<8>()
                    .expect("the range is the same length as the expected array"),
            )),
            Ok(TypeId::Boolean) => Self::Boolean(
                bool::try_from(
                    bytes[{
                        let ip = vm.ip;
                        vm.ip += 1;
                        ip
                    }],
                )
                .expect("a boolean could not be created from its byte"),
            ),
            Ok(TypeId::Unit) => Self::Unit,
            Ok(TypeId::Fn) => {
                let jump_address = bytes[(vm.ip)..{
                    vm.ip += 8;
                    vm.ip
                }]
                    .as_array::<8>()
                    .map(|array| u64::from_le_bytes(*array))
                    .and_then(|address| usize::try_from(address).ok())
                    .expect("a function was not a valid usize");

                Self::Fn(
                    bytes[jump_address..(jump_address + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|address| usize::try_from(address).ok())
                        .expect("a function was not a valid usize"),
                )
            }
            Ok(TypeId::NativeFn) => Self::NativeFn(
                bytes[(vm.ip)..{
                    vm.ip += 2;
                    vm.ip
                }]
                    .as_array::<2>()
                    .map(|array| u16::from_le_bytes(*array))
                    .expect("a native function was not valid"),
            ),
            Ok(TypeId::StackOffset) => Self::StackOffset(
                bytes[(vm.ip)..{
                    vm.ip += 8;
                    vm.ip
                }]
                    .as_array::<8>()
                    .map(|array| u64::from_le_bytes(*array))
                    .and_then(|address| usize::try_from(address).ok())
                    .expect("a stack offset was not a valid usize"),
            ),
            Ok(TypeId::Register) => Self::Register(
                bytes[(vm.ip)..{
                    vm.ip += 8;
                    vm.ip
                }]
                    .as_array::<8>()
                    .map(|array| u64::from_le_bytes(*array))
                    .and_then(|address| usize::try_from(address).ok())
                    .expect("a register was not a valid usize"),
            ),
            Ok(TypeId::Compound) => {
                let field_count = bytes[(vm.ip)..{
                    vm.ip += 8;
                    vm.ip
                }]
                    .as_array::<8>()
                    .map(|array| u64::from_le_bytes(*array))
                    .and_then(|field_count| usize::try_from(field_count).ok())
                    .expect("a compound's field count was not a valid usize");

                let mut fields = vec![];

                for _ in 0..field_count {
                    let field = <Self as Deserialize>::from_bytes(bytes, vm);

                    let field = vm.dereference_value(field);

                    vm.allocated += mem::size_of_val(&field);

                    fields.push(field);
                }

                let value_index = vm.push_value(Value::Compound(fields));

                vm.allocated +=
                    mem::size_of_val(vm.values.last().expect("the value was just pushed"));

                value_index
            }
            Ok(TypeId::TaggedCompound) => {
                let tag = bytes[(vm.ip)..{
                    vm.ip += 2;
                    vm.ip
                }]
                    .as_array::<2>()
                    .map(|array| u16::from_le_bytes(*array))
                    .expect("a tagged compound's tag was not a valid u16");

                let field_count = bytes[(vm.ip)..{
                    vm.ip += 8;
                    vm.ip
                }]
                    .as_array::<8>()
                    .map(|array| u64::from_le_bytes(*array))
                    .and_then(|field_count| usize::try_from(field_count).ok())
                    .expect("a tagged compound's field count was not a valid usize");

                let mut fields = vec![];

                for _ in 0..field_count {
                    let field = <Self as Deserialize>::from_bytes(bytes, vm);

                    let field = vm.dereference_value(field);

                    vm.allocated += mem::size_of_val(&field);

                    fields.push(field);
                }

                let value_index = vm.push_value(Value::TaggedCompound { tag, fields });

                vm.allocated +=
                    mem::size_of_val(vm.values.last().expect("the value was just pushed"));

                value_index
            }
            Err(error) => panic!("unknown type id {error}"),
        }
    }
}
