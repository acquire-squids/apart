use crate::targets::vm::{
    CopyableValue, Instruction, Instructive, NativeFn, OpCode, Value, ValueIndex,
};

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
    instructions: Vec<Instruction>,
    ip: usize,
}

impl Instructive for Vm {
    fn ip_mut(&mut self) -> &mut usize {
        &mut self.ip
    }

    fn instructions_mut(&mut self) -> &mut Vec<Instruction> {
        &mut self.instructions
    }

    fn values_mut(&mut self) -> &mut Vec<Value> {
        &mut self.values
    }
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

    let mut vm = Vm {
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
        ip: 0,
        instructions: vec![],
    };

    vm.read(bytes);

    vm.run(out);
}

macro_rules! dereference_value {
    (
        $self:ident, $value:expr $(,)?
    ) => {
        match $value {
            $crate::targets::vm::CopyableValue::Register(index) => $self.registers[index],
            $crate::targets::vm::CopyableValue::StackOffset(offset) => $self
                .call_frames
                .last()
                .map_or($self.stack[offset], |frame| $self.stack[frame.fp + offset]),
            $crate::targets::vm::CopyableValue::ValueIndex(value_index) => {
                match $self.values.get(usize::from(value_index)) {
                    None => panic!("tried to find a value that doesn't exist"),
                    Some($crate::targets::vm::Value::MakeCompound(fields)) => {
                        let fields = fields
                            .clone()
                            .into_iter()
                            .map(|value| $self.dereference_value(value))
                            .collect::<Vec<_>>();

                        $self.values.push(Value::Compound(fields));

                        let value_index = $crate::targets::vm::CopyableValue::ValueIndex(
                            ValueIndex($self.values.len() - 1),
                        );

                        $self.allocated += $self.size_of_value(value_index);

                        value_index
                    }
                    Some($crate::targets::vm::Value::MakeTaggedCompound { fields, tag }) => {
                        let tag = *tag;

                        let fields = fields
                            .clone()
                            .into_iter()
                            .map(|value| $self.dereference_value(value))
                            .collect::<Vec<_>>();

                        $self.values.push(Value::TaggedCompound { fields, tag });

                        let value_index = $crate::targets::vm::CopyableValue::ValueIndex(
                            ValueIndex($self.values.len() - 1),
                        );

                        $self.allocated += $self.size_of_value(value_index);

                        value_index
                    }
                    Some(Value::Compound(_) | Value::TaggedCompound { .. }) => $value,
                }
            }
            _ => $value,
        }
    };
}

impl Vm {
    #[allow(clippy::too_many_lines)]
    fn run<O>(&mut self, out: &mut O)
    where
        O: Write,
    {
        let instructions = mem::take(&mut self.instructions);

        while self.ip < instructions.len() {
            match instructions.get(self.ip) {
                None => break,
                Some(Instruction::Unary {
                    op_code,
                    operand,
                    to,
                }) => {
                    let operand = *operand;
                    let to = *to;

                    match op_code {
                        OpCode::Not => match dereference_value!(self, operand) {
                            CopyableValue::Boolean(value) => {
                                self.assign(to, CopyableValue::Boolean(!value));
                            }
                            _ => panic!("incorrect argument for logical not"),
                        },
                        OpCode::Negate => match dereference_value!(self, operand) {
                            CopyableValue::I64(value) => {
                                self.assign(to, CopyableValue::I64(-value));
                            }
                            CopyableValue::F64(value) => {
                                self.assign(to, CopyableValue::F64(-value));
                            }
                            _ => panic!("incorrect argument for negate"),
                        },
                        _ => unreachable!("there are no other unary operators"),
                    }
                }
                Some(Instruction::Binary {
                    op_code,
                    lhs,
                    rhs,
                    to,
                }) => {
                    let lhs = *lhs;
                    let rhs = *rhs;
                    let to = *to;

                    match op_code {
                        OpCode::Multiply
                        | OpCode::Divide
                        | OpCode::Remainder
                        | OpCode::Add
                        | OpCode::Subtract
                        | OpCode::Less
                        | OpCode::Greater
                        | OpCode::LessOrEqual
                        | OpCode::GreaterOrEqual => {
                            let lhs = dereference_value!(self, lhs);
                            let rhs = dereference_value!(self, rhs);

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
                                        OpCode::GreaterOrEqual => {
                                            CopyableValue::Boolean(lhs >= rhs)
                                        }
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
                                        OpCode::GreaterOrEqual => {
                                            CopyableValue::Boolean(lhs >= rhs)
                                        }
                                        _ => unreachable!(
                                            "only these opcodes get past the initial match arm"
                                        ),
                                    },
                                ),
                                _ => panic!("incorrect argument for arithmetic"),
                            }
                        }
                        OpCode::And | OpCode::Or => {
                            let lhs = dereference_value!(self, lhs);
                            let rhs = dereference_value!(self, rhs);

                            match (lhs, rhs) {
                                (CopyableValue::Boolean(lhs), CopyableValue::Boolean(rhs)) => self
                                    .assign(
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
                            let lhs = dereference_value!(self, lhs);
                            let rhs = dereference_value!(self, rhs);

                            self.assign(
                                to,
                                match op_code {
                                    OpCode::Equal => {
                                        CopyableValue::Boolean(self.values_eq(lhs, rhs))
                                    }
                                    OpCode::NotEqual => {
                                        CopyableValue::Boolean(!self.values_eq(lhs, rhs))
                                    }
                                    _ => unreachable!(
                                        "only these opcodes get past the initial match arm"
                                    ),
                                },
                            );
                        }
                        _ => unreachable!("there are no other binary operators"),
                    }
                }
                Some(Instruction::Assign { to, value }) => {
                    let value = *value;
                    let to = *to;

                    let value = dereference_value!(self, value);

                    self.assign(to, value);
                }
                Some(Instruction::Push(value)) => {
                    let value = *value;

                    let value = dereference_value!(self, value);

                    self.stack.push(value);
                }
                Some(Instruction::Access { index, of, to }) => {
                    let index = *index;
                    let of = *of;
                    let to = *to;

                    let value = if let CopyableValue::ValueIndex(value_index) =
                        dereference_value!(self, of)
                        && let Some(Value::Compound(fields)) =
                            self.values.get(usize::from(value_index))
                    {
                        fields[index]
                    } else {
                        panic!("tried to access something other than a compound");
                    };

                    let value = dereference_value!(self, value);

                    self.assign(to, value);
                }
                Some(Instruction::AccessAssign { index, of, value }) => {
                    let index = *index;
                    let of = *of;
                    let value = *value;

                    let value = dereference_value!(self, value);

                    if let CopyableValue::ValueIndex(value_index) = dereference_value!(self, of)
                        && let Some(Value::Compound(fields)) =
                            self.values.get_mut(usize::from(value_index))
                        && let Some(field) = fields.get_mut(index)
                    {
                        *field = value;
                    } else {
                        panic!("tried to assign to an invalid compound field");
                    }
                }
                Some(Instruction::Call { callee, arity, to }) => {
                    let callee = *callee;
                    let arity = *arity;
                    let to = *to;

                    match dereference_value!(self, callee) {
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

                            continue;
                        }
                        CopyableValue::NativeFn(index) => {
                            let call_arguments = self.stack.split_off(self.stack.len() - arity);

                            let value = Self::native_fn_call(call_arguments.as_slice(), index, out);

                            let value = dereference_value!(self, value);

                            self.assign(to, value);
                        }
                        _ => {
                            panic!("tried to call an uncallable");
                        }
                    }
                }
                Some(Instruction::Jump(destination, arguments)) => {
                    let arguments = arguments
                        .iter()
                        .map(|(to, argument)| (*to, dereference_value!(self, *argument)))
                        .collect::<Vec<_>>();

                    for (to, argument) in arguments {
                        self.assign(to, argument);
                    }

                    self.ip = *destination;

                    continue;
                }
                Some(Instruction::Branch {
                    condition,
                    when_true,
                    otherwise,
                }) => {
                    let condition = *condition;

                    match dereference_value!(self, condition) {
                        CopyableValue::Boolean(true) => {
                            let arguments = when_true
                                .1
                                .iter()
                                .map(|(to, argument)| (*to, dereference_value!(self, *argument)))
                                .collect::<Vec<_>>();

                            for (to, argument) in arguments {
                                self.assign(to, argument);
                            }

                            self.ip = when_true.0;
                        }
                        CopyableValue::Boolean(false) => {
                            let arguments = otherwise
                                .1
                                .iter()
                                .map(|(to, argument)| (*to, dereference_value!(self, *argument)))
                                .collect::<Vec<_>>();

                            for (to, argument) in arguments {
                                self.assign(to, argument);
                            }

                            self.ip = otherwise.0;
                        }
                        _ => panic!("branch condition was not a boolean"),
                    }

                    continue;
                }
                Some(Instruction::Return(value)) => {
                    let value = *value;

                    let value = dereference_value!(self, value);

                    if let Some(call_frame) = self.call_frames.pop() {
                        self.ip = call_frame.from;

                        self.stack.truncate(call_frame.fp);

                        self.registers[..(call_frame.previous_registers.len())]
                            .copy_from_slice(call_frame.previous_registers.as_slice());

                        if !self.call_frames.is_empty()
                            && let Some(Instruction::Call { to, .. }) = instructions.get(self.ip)
                        {
                            self.assign(*to, value);
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

            self.ip += 1;
        }

        self.gc();

        assert_eq!(self.allocated, 0, "{} BYTES LEAKED", self.allocated);
    }

    fn assign(&mut self, to: CopyableValue, value: CopyableValue) {
        let value = dereference_value!(self, value);

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

    fn dereference_value(&mut self, value: CopyableValue) -> CopyableValue {
        match value {
            CopyableValue::Register(index) => self.registers[index],
            CopyableValue::StackOffset(offset) => self
                .call_frames
                .last()
                .map_or(self.stack[offset], |frame| self.stack[frame.fp + offset]),
            CopyableValue::ValueIndex(value_index) => {
                match self.values.get(usize::from(value_index)) {
                    None => panic!("tried to find a value that doesn't exist"),
                    Some(Value::MakeCompound(fields)) => {
                        let fields = fields
                            .clone()
                            .into_iter()
                            .map(|value| self.dereference_value(value))
                            .collect::<Vec<_>>();

                        self.values.push(Value::Compound(fields));

                        let value_index =
                            CopyableValue::ValueIndex(ValueIndex(self.values.len() - 1));

                        self.allocated += self.size_of_value(value_index);

                        value_index
                    }
                    Some(Value::MakeTaggedCompound { fields, tag }) => {
                        let tag = *tag;

                        let fields = fields
                            .clone()
                            .into_iter()
                            .map(|value| self.dereference_value(value))
                            .collect::<Vec<_>>();

                        self.values.push(Value::TaggedCompound { fields, tag });

                        let value_index =
                            CopyableValue::ValueIndex(ValueIndex(self.values.len() - 1));

                        self.allocated += self.size_of_value(value_index);

                        value_index
                    }
                    Some(Value::Compound(_) | Value::TaggedCompound { .. }) => value,
                }
            }
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

        for v in (0..(self.values.len())).filter(|v| {
            matches!(
                self.values.get(*v),
                Some(Value::MakeCompound(_) | Value::MakeTaggedCompound { .. })
            )
        }) {
            let value = CopyableValue::ValueIndex(ValueIndex(v));

            self.mark_value(&mut marked, value, &mut marked_count);
        }

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
                Value::MakeCompound(values)
                | Value::MakeTaggedCompound { fields: values, .. }
                | Value::Compound(values)
                | Value::TaggedCompound { fields: values, .. } => {
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
                        Value::MakeCompound(values) => {
                            let values = values
                                .iter()
                                .map(|value| self.retain_value(replacement_values, *value, marked))
                                .collect::<Vec<_>>();

                            replacement_values.push((replacement_index, Value::Compound(values)));
                        }
                        Value::MakeTaggedCompound {
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
                Value::Compound(values) | Value::MakeCompound(values) => {
                    values
                        .iter()
                        .fold(0, |accum, value| accum + mem::size_of_val(value))
                        + mem::size_of_val(&self.values[usize::from(index)])
                }
                Value::TaggedCompound { fields: values, .. }
                | Value::MakeTaggedCompound { fields: values, .. } => {
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
