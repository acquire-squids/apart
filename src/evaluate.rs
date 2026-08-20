use crate::{
    Span,
    basic_blocks::{BlockIndex, Instruction, Value as IrValue},
    parse::{BinaryOp, UnaryOp},
    ssa::{Argument, BlockTerminator, JumpTo, Ssa},
};

use std::{io::Write, mem};

struct CallFrame<const MAX_REGISTERS: usize> {
    call_arguments: Vec<CopyableValue>,
    block_arguments: Vec<CopyableValue>,
    from: (usize, usize),
    fp: usize,
    block_index: BlockIndex,
    previous_registers: Vec<(usize, CopyableValue)>,
}

struct Evaluator<const MAX_REGISTERS: usize> {
    stack: Vec<CopyableValue>,
    values: Vec<Value>,
    allocated: usize,
    next_gc: usize,
    registers: [CopyableValue; MAX_REGISTERS],
    call_frames: Vec<CallFrame<MAX_REGISTERS>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CopyableValue {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Unit,
    Fn(BlockIndex),
    Runtime,
    ValueIndex(ValueIndex),
}

#[derive(Debug)]
enum Value {
    NativeFn(Span),
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

pub fn run<const MAX_REGISTERS: usize, O>(ssa: &Ssa, sources: &[(usize, &str)], out: &mut O)
where
    O: Write,
{
    let mut evaluator = Evaluator {
        stack: vec![],
        values: vec![],
        allocated: 0,
        next_gc: 1_000_000,
        registers: [const { CopyableValue::Runtime }; MAX_REGISTERS],
        call_frames: vec![CallFrame {
            call_arguments: vec![],
            block_arguments: vec![],
            from: (0, 0),
            fp: 0,
            block_index: BlockIndex(0),
            previous_registers: vec![],
        }],
    };

    evaluator.run(ssa, sources, out);
}

impl<const MAX_REGISTERS: usize> Evaluator<MAX_REGISTERS> {
    #[allow(clippy::too_many_lines)]
    fn run<O>(&mut self, ssa: &Ssa, sources: &[(usize, &str)], out: &mut O)
    where
        O: Write,
    {
        let mut b = 0;
        let mut i = 0;

        'block: while let Some(block) = ssa.blocks().get(b) {
            assert_eq!(
                block.parameters().len(),
                self.call_frames
                    .last()
                    .map_or(0, |call_frame| call_frame.block_arguments.len()),
                "BLOCK ARGUMENT MISMATCH IN BLOCK INDEX {b}"
            );

            while let Some(instruction) = block.instructions().get(i) {
                match instruction {
                    Instruction::NoOp => {}
                    Instruction::Unary {
                        op,
                        operand,
                        temporary: to,
                    } => {
                        let value = self.evaluate_unary((*op, operand));

                        self.assign(to, value);
                    }
                    Instruction::Binary {
                        op,
                        lhs,
                        rhs,
                        temporary: to,
                    } => {
                        let value = self.evaluate_binary((*op, lhs, rhs));

                        self.assign(to, value);
                    }
                    Instruction::Access {
                        index,
                        of,
                        temporary: to,
                    } => {
                        let CopyableValue::ValueIndex(value_index) = self.convert_ir_value(of)
                        else {
                            unreachable!("type checking guarantees an accessee is a compound");
                        };

                        let Value::Compound(values) = &self.values[usize::from(value_index)] else {
                            unreachable!("type checking guarantees an accessee is a compound");
                        };

                        self.assign(to, values[*index]);
                    }
                    Instruction::AccessAssign { index, of, value } => {
                        let value = self.convert_ir_value(value);

                        let CopyableValue::ValueIndex(value_index) = self.convert_ir_value(of)
                        else {
                            unreachable!("type checking guarantees an accessee is a compound");
                        };

                        let Value::Compound(values) = &mut self.values[usize::from(value_index)]
                        else {
                            unreachable!("type checking guarantees an accessee is a compound");
                        };

                        values[*index] = value;
                    }
                    Instruction::Assign { value, to } => {
                        let value = self.convert_ir_value(value);

                        self.assign(to, value);
                    }
                    Instruction::Push(value) => {
                        let cloned = self.convert_ir_value(value);

                        self.stack.push(cloned);
                    }
                    Instruction::Call {
                        callee,
                        arity,
                        temporary: to,
                    } => match self.convert_ir_value(callee) {
                        CopyableValue::Fn(callee) => {
                            if self.should_gc() {
                                self.gc();
                            }

                            let call_frame = CallFrame {
                                block_arguments: vec![],
                                call_arguments: self
                                    .stack
                                    .drain((self.stack.len() - *arity)..)
                                    .collect::<Vec<_>>(),
                                from: (b, i),
                                fp: self.stack.len(),
                                block_index: callee,
                                previous_registers: vec![],
                            };

                            self.call_frames.push(call_frame);

                            b = usize::from(callee);
                            i = 0;

                            continue 'block;
                        }
                        CopyableValue::ValueIndex(index) => {
                            let Value::NativeFn(span) = self.values[usize::from(index)] else {
                                unreachable!("type checking guarantees callees are functions");
                            };

                            let call_arguments = self
                                .stack
                                .drain((self.stack.len() - *arity)..)
                                .collect::<Vec<_>>();

                            let value =
                                Self::native_fn_call(call_arguments.as_slice(), sources, span, out);

                            self.assign(to, value);
                        }
                        _ => {
                            unreachable!("type checking would have caught calling an uncallable");
                        }
                    },
                }

                i += 1;
            }

            match block.terminator() {
                BlockTerminator::Jump(jump_to) => {
                    b = usize::from(jump_to.block());
                    i = 0;

                    let block_args = self.collect_block_arguments(jump_to);

                    self.call_frames
                        .last_mut()
                        .expect("there will always be a call frame")
                        .block_arguments = block_args;
                }
                BlockTerminator::Branch {
                    condition,
                    when_true,
                    otherwise,
                } => match self.convert_ir_value(condition) {
                    CopyableValue::Boolean(true) => {
                        b = usize::from(when_true.block());
                        i = 0;

                        let block_args = self.collect_block_arguments(when_true);

                        self.call_frames
                            .last_mut()
                            .expect("there will always be a call frame")
                            .block_arguments = block_args;
                    }
                    CopyableValue::Boolean(false) => {
                        b = usize::from(otherwise.block());
                        i = 0;

                        let block_args = self.collect_block_arguments(otherwise);

                        self.call_frames
                            .last_mut()
                            .expect("there will always be a call frame")
                            .block_arguments = block_args;
                    }
                    _ => {
                        unreachable!("type checking guarantees conditions are boolean");
                    }
                },
                BlockTerminator::Return(value) => {
                    let value = self.convert_ir_value(value);

                    if let Some(call_frame) = self.call_frames.pop() {
                        b = call_frame.from.0;
                        i = call_frame.from.1;

                        self.stack.truncate(call_frame.fp);

                        for (register_index, value) in call_frame.previous_registers {
                            self.registers[register_index] = value;
                        }

                        if !self.call_frames.is_empty() {
                            let Some(Instruction::Call { temporary: to, .. }) = ssa
                                .blocks()
                                .get(b)
                                .and_then(|block| block.instructions().get(i))
                            else {
                                unreachable!("call frames only come from calls");
                            };

                            self.assign(to, value);
                        }

                        i += 1;
                    }

                    if self.should_gc() {
                        self.gc();
                    }

                    if self.call_frames.is_empty() {
                        break 'block;
                    }
                }
            }
        }

        self.gc();

        assert_eq!(self.allocated, 0, "{} BYTES LEAKED", self.allocated);
    }

    fn assign(&mut self, to: &IrValue, value: CopyableValue) {
        match to {
            IrValue::Register(index) => {
                if let Some(call_frame) = self.call_frames.last_mut()
                    && !call_frame
                        .previous_registers
                        .iter()
                        .any(|(register_index, _)| register_index == index)
                {
                    let old_value = mem::replace(&mut self.registers[*index], value);

                    call_frame.previous_registers.push((*index, old_value));
                } else {
                    self.registers[*index] = value;
                }
            }
            IrValue::Address(address) => {
                if let Some(stack_value) = self
                    .call_frames
                    .iter()
                    .rfind(|frame| frame.block_index == address.block_index)
                    .map(|frame| frame.fp + address.offset)
                    .and_then(|offset| self.stack.get_mut(offset))
                {
                    *stack_value = value;
                } else {
                    self.stack.push(value);
                }
            }
            _ => unreachable!("only registers and stack slots can be assigned to"),
        }
    }

    #[must_use]
    fn native_fn_call<O>(
        call_arguments: &[CopyableValue],
        sources: &[(usize, &str)],
        span: Span,
        out: &mut O,
    ) -> CopyableValue
    where
        O: Write,
    {
        match sources
            .iter()
            .find(|(source_id, _)| *source_id == span.source_id())
            .and_then(|(_, source)| span.lexeme(source.as_ref()))
            .expect("this span should be from a matching source")
        {
            lexeme @ "print_i64" => {
                let CopyableValue::Integer(value) = &call_arguments[0] else {
                    unreachable!("({lexeme:?} {:?} @ 0)", call_arguments[0]);
                };

                writeln!(out, "{value}").expect("failed to write to output");

                CopyableValue::Unit
            }
            lexeme @ "print_f64" => {
                let CopyableValue::Float(value) = &call_arguments[0] else {
                    unreachable!("({lexeme:?} {:?} @ 0)", call_arguments[0]);
                };

                writeln!(out, "{value:?}").expect("failed to write to output");

                CopyableValue::Unit
            }
            lexeme @ "print_bool" => {
                let CopyableValue::Boolean(value) = &call_arguments[0] else {
                    unreachable!("({lexeme:?} {:?} @ 0)", call_arguments[0]);
                };

                writeln!(out, "{value}").expect("failed to write to output");

                CopyableValue::Unit
            }
            lexeme @ "print_unit" => {
                let CopyableValue::Unit = &call_arguments[0] else {
                    unreachable!("({lexeme:?} {:?} @ 0)", call_arguments[0]);
                };

                writeln!(out, "{{}}").expect("failed to write to output");

                CopyableValue::Unit
            }
            _ => {
                unreachable!("tried to call an unknown native function");
            }
        }
    }

    #[must_use]
    fn collect_block_arguments(&mut self, jump_to: &JumpTo) -> Vec<CopyableValue> {
        jump_to
            .arguments()
            .iter()
            .map(|argument| match argument {
                Argument::Address(ir_value) => self.convert_ir_value(ir_value),
                Argument::Passthrough(i) => {
                    self.call_frames
                        .last()
                        .expect("there will always be a call frame")
                        .block_arguments[*i]
                }
            })
            .collect::<Vec<_>>()
    }

    #[must_use]
    fn evaluate_unary(&mut self, (op, operand): (UnaryOp, &IrValue)) -> CopyableValue {
        match (op, self.convert_ir_value(operand)) {
            (UnaryOp::Not, CopyableValue::Boolean(value)) => CopyableValue::Boolean(!value),
            (UnaryOp::Negate, CopyableValue::Integer(value)) => CopyableValue::Integer(-value),
            (UnaryOp::Negate, CopyableValue::Float(value)) => CopyableValue::Float(-value),
            (op, operand) => unreachable!("({op:?} {operand:?})"),
        }
    }

    #[allow(clippy::too_many_lines)]
    #[must_use]
    fn evaluate_binary(&mut self, (op, lhs, rhs): (BinaryOp, &IrValue, &IrValue)) -> CopyableValue {
        match (op, self.convert_ir_value(lhs), self.convert_ir_value(rhs)) {
            (BinaryOp::Multiply, CopyableValue::Integer(lhs), CopyableValue::Integer(rhs)) => {
                CopyableValue::Integer(lhs * rhs)
            }
            (BinaryOp::Divide, CopyableValue::Integer(lhs), CopyableValue::Integer(rhs)) => {
                CopyableValue::Integer(lhs / rhs)
            }
            (BinaryOp::Remainder, CopyableValue::Integer(lhs), CopyableValue::Integer(rhs)) => {
                CopyableValue::Integer(lhs % rhs)
            }
            (BinaryOp::Add, CopyableValue::Integer(lhs), CopyableValue::Integer(rhs)) => {
                CopyableValue::Integer(lhs + rhs)
            }
            (BinaryOp::Subtract, CopyableValue::Integer(lhs), CopyableValue::Integer(rhs)) => {
                CopyableValue::Integer(lhs - rhs)
            }
            (BinaryOp::Less, CopyableValue::Integer(lhs), CopyableValue::Integer(rhs)) => {
                CopyableValue::Boolean(lhs < rhs)
            }
            (BinaryOp::Greater, CopyableValue::Integer(lhs), CopyableValue::Integer(rhs)) => {
                CopyableValue::Boolean(lhs > rhs)
            }
            (BinaryOp::LessOrEqual, CopyableValue::Integer(lhs), CopyableValue::Integer(rhs)) => {
                CopyableValue::Boolean(lhs <= rhs)
            }
            (
                BinaryOp::GreaterOrEqual,
                CopyableValue::Integer(lhs),
                CopyableValue::Integer(rhs),
            ) => CopyableValue::Boolean(lhs >= rhs),
            (BinaryOp::Multiply, CopyableValue::Float(lhs), CopyableValue::Float(rhs)) => {
                CopyableValue::Float(lhs * rhs)
            }
            (BinaryOp::Divide, CopyableValue::Float(lhs), CopyableValue::Float(rhs)) => {
                CopyableValue::Float(lhs / rhs)
            }
            (BinaryOp::Remainder, CopyableValue::Float(lhs), CopyableValue::Float(rhs)) => {
                CopyableValue::Float(lhs % rhs)
            }
            (BinaryOp::Add, CopyableValue::Float(lhs), CopyableValue::Float(rhs)) => {
                CopyableValue::Float(lhs + rhs)
            }
            (BinaryOp::Subtract, CopyableValue::Float(lhs), CopyableValue::Float(rhs)) => {
                CopyableValue::Float(lhs - rhs)
            }
            (BinaryOp::Less, CopyableValue::Float(lhs), CopyableValue::Float(rhs)) => {
                CopyableValue::Boolean(lhs < rhs)
            }
            (BinaryOp::Greater, CopyableValue::Float(lhs), CopyableValue::Float(rhs)) => {
                CopyableValue::Boolean(lhs > rhs)
            }
            (BinaryOp::LessOrEqual, CopyableValue::Float(lhs), CopyableValue::Float(rhs)) => {
                CopyableValue::Boolean(lhs <= rhs)
            }
            (BinaryOp::GreaterOrEqual, CopyableValue::Float(lhs), CopyableValue::Float(rhs)) => {
                CopyableValue::Boolean(lhs >= rhs)
            }
            (BinaryOp::Equal, CopyableValue::Float(lhs), CopyableValue::Float(rhs)) => {
                CopyableValue::Boolean((lhs - rhs).abs() < f64::EPSILON)
            }
            (BinaryOp::NotEqual, CopyableValue::Float(lhs), CopyableValue::Float(rhs)) => {
                CopyableValue::Boolean((lhs - rhs).abs() >= f64::EPSILON)
            }
            (BinaryOp::Equal, lhs, rhs) => CopyableValue::Boolean(self.values_eq(lhs, rhs)),
            (BinaryOp::NotEqual, lhs, rhs) => CopyableValue::Boolean(!self.values_eq(lhs, rhs)),
            (op, lhs, rhs) => unreachable!("({op:?} {lhs:?} {rhs:?})"),
        }
    }

    fn values_eq(&self, lhs: CopyableValue, rhs: CopyableValue) -> bool {
        match (lhs, rhs) {
            (CopyableValue::Integer(lhs), CopyableValue::Integer(rhs)) => lhs == rhs,
            (CopyableValue::Float(lhs), CopyableValue::Float(rhs)) => lhs == rhs,
            (CopyableValue::Boolean(lhs), CopyableValue::Boolean(rhs)) => lhs == rhs,
            (CopyableValue::Unit, CopyableValue::Unit) => lhs == rhs,
            (CopyableValue::Fn(lhs), CopyableValue::Fn(rhs)) => lhs == rhs,
            (CopyableValue::ValueIndex(lhs), CopyableValue::ValueIndex(rhs)) => {
                match (
                    &self.values[usize::from(lhs)],
                    &self.values[usize::from(rhs)],
                ) {
                    (Value::NativeFn(lhs), Value::NativeFn(rhs)) => lhs == rhs,
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

    fn convert_ir_value(&mut self, ir_value: &IrValue) -> CopyableValue {
        match ir_value {
            IrValue::Integer(value) => CopyableValue::Integer(*value),
            IrValue::Float(value) => CopyableValue::Float(*value),
            IrValue::Boolean(value) => CopyableValue::Boolean(*value),
            IrValue::Unit => CopyableValue::Unit,
            IrValue::Fn(value) => CopyableValue::Fn(*value),
            IrValue::Runtime => CopyableValue::Runtime,
            IrValue::BlockArgument(value) => self
                .call_frames
                .last()
                .map(|call_frame| call_frame.block_arguments[*value])
                .expect("the call frame will exist"),
            IrValue::CallArgument(value) => self
                .call_frames
                .last()
                .map(|call_frame| call_frame.call_arguments[*value])
                .expect("the call frame will exist"),
            IrValue::Register(value) => self.registers[*value],
            IrValue::Address(address) => self
                .call_frames
                .iter()
                .rfind(|frame| frame.block_index == address.block_index)
                .map(|frame| frame.fp + address.offset)
                .map(|offset| self.stack[offset])
                .expect("the stack value will exist"),
            IrValue::NativeFn(span) => {
                self.values.push(Value::NativeFn(*span));

                self.allocated +=
                    mem::size_of_val(self.values.last().expect("the value was just pushed"));

                CopyableValue::ValueIndex(ValueIndex(self.values.len() - 1))
            }
            IrValue::Compound(values) => {
                let values = values
                    .iter()
                    .map(|value| {
                        let element = self.convert_ir_value(value);

                        self.allocated += mem::size_of_val(&element);

                        element
                    })
                    .collect::<Vec<_>>();

                self.values.push(Value::Compound(values));

                self.allocated +=
                    mem::size_of_val(self.values.last().expect("the value was just pushed"));

                CopyableValue::ValueIndex(ValueIndex(self.values.len() - 1))
            }
            IrValue::TaggedCompound {
                fields: values,
                tag,
            } => {
                let tag = *tag;

                let values = values
                    .iter()
                    .map(|value| {
                        let element = self.convert_ir_value(value);

                        self.allocated += mem::size_of_val(&element);

                        element
                    })
                    .collect::<Vec<_>>();

                self.values.push(Value::TaggedCompound {
                    fields: values,
                    tag,
                });

                self.allocated +=
                    mem::size_of_val(self.values.last().expect("the value was just pushed"));

                CopyableValue::ValueIndex(ValueIndex(self.values.len() - 1))
            }
        }
    }

    const fn should_gc(&self) -> bool {
        self.allocated >= self.next_gc
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
            for call_argument in &call_frame.call_arguments {
                self.mark_value(&mut marked, *call_argument, &mut marked_count);
            }

            for block_argument in &call_frame.block_arguments {
                self.mark_value(&mut marked, *block_argument, &mut marked_count);
            }

            for (_, previous_register) in &call_frame.previous_registers {
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
                Value::NativeFn(_) => {}
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
            .collect::<Vec<_>>()
            .try_into()
            .expect("the replacement registers are made from the original registers");

        for c in 0..(self.call_frames.len()) {
            let Some(call_frame) = self.call_frames.get(c) else {
                unreachable!("the call frame will exist");
            };

            let call_arguments = call_frame
                .call_arguments
                .iter()
                .map(|call_argument| self.retain_value(&mut values, *call_argument, marked))
                .collect::<Vec<_>>();

            let block_arguments = call_frame
                .block_arguments
                .iter()
                .map(|block_argument| self.retain_value(&mut values, *block_argument, marked))
                .collect::<Vec<_>>();

            let previous_registers = call_frame
                .previous_registers
                .iter()
                .map(|(i, previous_register)| {
                    (
                        *i,
                        self.retain_value(&mut values, *previous_register, marked),
                    )
                })
                .collect::<Vec<_>>();

            let Some(call_frame) = self.call_frames.get_mut(c) else {
                unreachable!("the call frame will exist");
            };

            call_frame.call_arguments = call_arguments;
            call_frame.block_arguments = block_arguments;
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
                .map_or(CopyableValue::Runtime, |replacement_index| {
                    match &self.values[usize::from(index)] {
                        Value::NativeFn(span) => {
                            replacement_values.push((replacement_index, Value::NativeFn(*span)));
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
                Value::NativeFn(_) => mem::size_of_val(&self.values[usize::from(index)]),
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
}
