use crate::{
    Span,
    basic_blocks::{BlockIndex, Instruction, Value},
    parse::{BinaryOp, UnaryOp},
    ssa::{Argument, BlockTerminator, JumpTo, Ssa},
};

use std::{io::Write, mem};

struct CallFrame<const MAX_REGISTERS: usize> {
    call_arguments: Vec<Value>,
    block_arguments: Vec<Value>,
    from: (usize, usize),
    fp: usize,
    block_index: BlockIndex,
    previous_registers: Vec<(usize, Value)>,
}

struct Evaluator<const MAX_REGISTERS: usize> {
    stack: Vec<Value>,
    registers: [Value; MAX_REGISTERS],
    call_frames: Vec<CallFrame<MAX_REGISTERS>>,
}

pub fn run<const MAX_REGISTERS: usize, O>(ssa: &Ssa, sources: &[(usize, &str)], out: &mut O)
where
    O: Write,
{
    let mut evaluator = Evaluator {
        stack: vec![],
        registers: [const { Value::Runtime }; MAX_REGISTERS],
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
        let mut fp = None;

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
                        let Value::Compound(values) = self.dereference_value(of) else {
                            unreachable!("type checking guarantees an accessee is a compound");
                        };

                        let value = self.dereference_value(&values[*index]).clone();

                        self.assign(to, value);
                    }
                    Instruction::AccessAssign { index, of, value } => {
                        let value = self.dereference_value(value).clone();

                        let Value::Compound(values) = self.dereference_value_mut(of) else {
                            unreachable!("type checking guarantees an accessee is a compound");
                        };

                        values[*index] = value;
                    }
                    Instruction::Assign { value, to } => {
                        let value = self.dereference_value(value).clone();

                        self.assign(to, value);
                    }
                    Instruction::Push(value) => {
                        if fp.is_none() {
                            fp = Some(self.stack.len());
                        }

                        let cloned = self.dereference_value(value).clone();

                        self.stack.push(cloned);
                    }
                    Instruction::Pop => {
                        let value = self.stack.pop().expect("call argument stack underflow");

                        let Some(call_frame) = self.call_frames.last_mut() else {
                            unreachable!("cannot pop outside of a call");
                        };

                        call_frame.call_arguments.push(value);
                    }
                    Instruction::Call {
                        callee,
                        temporary: to,
                    } => {
                        let fp = fp.take().unwrap_or(self.stack.len());

                        match self.dereference_value(callee) {
                            Value::Fn(callee) => {
                                let callee = *callee;

                                self.call_frames.push(CallFrame {
                                    block_arguments: vec![],
                                    call_arguments: vec![],
                                    from: (b, i),
                                    fp,
                                    block_index: callee,
                                    previous_registers: vec![],
                                });

                                b = usize::from(callee);
                                i = 0;

                                continue 'block;
                            }
                            Value::NativeFn(span) => {
                                let span = *span;

                                let value = self.native_fn_call(sources, span, out);

                                self.assign(to, value);
                            }
                            _ => {
                                unreachable!(
                                    "type checking would have caught calling an uncallable"
                                );
                            }
                        }
                    }
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
                } => match self.dereference_value(condition) {
                    Value::Boolean(true) => {
                        b = usize::from(when_true.block());
                        i = 0;

                        let block_args = self.collect_block_arguments(when_true);

                        self.call_frames
                            .last_mut()
                            .expect("there will always be a call frame")
                            .block_arguments = block_args;
                    }
                    Value::Boolean(false) => {
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
                    let value = self.clone_value(value);

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

                    if self.call_frames.is_empty() {
                        break 'block;
                    }
                }
            }
        }
    }

    fn clone_value(&self, value: &Value) -> Value {
        match self.dereference_value(value).clone() {
            Value::Compound(values) => Value::Compound(
                values
                    .iter()
                    .map(|value| self.clone_value(value))
                    .collect::<Vec<_>>(),
            ),
            value => value,
        }
    }

    fn assign(&mut self, to: &Value, value: Value) {
        match to {
            Value::Register(index) => {
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
            Value::Address(address) => {
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
    fn native_fn_call<O>(&mut self, sources: &[(usize, &str)], span: Span, out: &mut O) -> Value
    where
        O: Write,
    {
        match sources
            .iter()
            .find(|(source_id, _)| *source_id == span.source_id())
            .and_then(|(_, source)| span.lexeme(source.as_ref()))
            .expect("this span should be from a matching source")
        {
            "print_i64" => {
                let value = self.stack.pop();

                let Some(Value::Integer(value)) =
                    value.as_ref().map(|value| self.dereference_value(value))
                else {
                    unreachable!("type checking would have caught an incorrect argument");
                };

                writeln!(out, "{value}").expect("failed to write to output");

                Value::Unit
            }
            "print_f64" => {
                let value = self.stack.pop();

                let Some(Value::Float(value)) =
                    value.as_ref().map(|value| self.dereference_value(value))
                else {
                    unreachable!("type checking would have caught an incorrect argument");
                };

                writeln!(out, "{value:?}").expect("failed to write to output");

                Value::Unit
            }
            "print_bool" => {
                let value = self.stack.pop();

                let Some(Value::Boolean(value)) =
                    value.as_ref().map(|value| self.dereference_value(value))
                else {
                    unreachable!("type checking would have caught an incorrect argument");
                };

                writeln!(out, "{value}").expect("failed to write to output");

                Value::Unit
            }
            "print_unit" => {
                let value = self.stack.pop();

                let Some(Value::Unit) = value.as_ref().map(|value| self.dereference_value(value))
                else {
                    unreachable!("type checking would have caught an incorrect argument");
                };

                writeln!(out, "{{}}").expect("failed to write to output");

                Value::Unit
            }
            _ => {
                unreachable!("tried to call an unknown native function");
            }
        }
    }

    #[must_use]
    fn collect_block_arguments(&self, jump_to: &JumpTo) -> Vec<Value> {
        jump_to
            .arguments()
            .iter()
            .map(|argument| {
                match argument {
                    Argument::Address(Value::Address(address)) => self.dereference_address(
                        self.call_frames
                            .iter()
                            .rfind(|frame| frame.block_index == address.block_index)
                            .map(|frame| frame.fp + address.offset)
                            .expect("couldn't calculate offset from fp"),
                    ),
                    Argument::Address(Value::Register(index)) => self.dereference_register(*index),
                    Argument::Address(_) => {
                        unreachable!("only registers and stack slots can be block arguments")
                    }
                    Argument::Passthrough(i) => {
                        &self
                            .call_frames
                            .last()
                            .expect("there will always be a call frame")
                            .block_arguments[*i]
                    }
                }
                .clone()
            })
            .collect::<Vec<_>>()
    }

    #[must_use]
    fn dereference_value<'a>(&'a self, value: &'a Value) -> &'a Value {
        match value {
            Value::BlockArgument(index) => self
                .call_frames
                .last()
                .map(|call_frame| &call_frame.block_arguments[*index])
                .expect("the block argument should exist"),
            Value::CallArgument(index) => self
                .call_frames
                .last()
                .map(|call_frame| &call_frame.call_arguments[*index])
                .expect("the call argument should exist"),
            Value::Address(address) => self.dereference_address(
                self.call_frames
                    .iter()
                    .rfind(|frame| frame.block_index == address.block_index)
                    .map(|frame| frame.fp + address.offset)
                    .expect("couldn't calculate offset from fp"),
            ),
            Value::Register(index) => self.dereference_register(*index),
            _ => value,
        }
    }

    #[must_use]
    fn dereference_address(&self, offset: usize) -> &Value {
        match &self.stack[offset] {
            Value::BlockArgument(index) => self
                .call_frames
                .last()
                .map(|call_frame| &call_frame.block_arguments[*index])
                .expect("the block argument should exist"),
            Value::CallArgument(index) => self
                .call_frames
                .last()
                .map(|call_frame| &call_frame.call_arguments[*index])
                .expect("the call argument should exist"),
            Value::Address(address) => self.dereference_address(
                self.call_frames
                    .iter()
                    .rfind(|frame| frame.block_index == address.block_index)
                    .map(|frame| frame.fp + address.offset)
                    .expect("couldn't calculate offset from fp"),
            ),
            Value::Register(index) => self.dereference_register(*index),
            value => value,
        }
    }

    #[must_use]
    fn dereference_register(&self, index: usize) -> &Value {
        match &self.registers[index] {
            Value::BlockArgument(index) => self
                .call_frames
                .last()
                .map(|call_frame| &call_frame.block_arguments[*index])
                .expect("the block argument should exist"),
            Value::CallArgument(index) => self
                .call_frames
                .last()
                .map(|call_frame| &call_frame.call_arguments[*index])
                .expect("the call argument should exist"),
            Value::Address(address) => self.dereference_address(
                self.call_frames
                    .iter()
                    .rfind(|frame| frame.block_index == address.block_index)
                    .map(|frame| frame.fp + address.offset)
                    .expect("couldn't calculate offset from fp"),
            ),
            Value::Register(index) => self.dereference_register(*index),
            value => value,
        }
    }

    #[must_use]
    fn dereference_value_mut<'a>(&'a mut self, value: &Value) -> &'a mut Value {
        match value {
            Value::BlockArgument(index) => self
                .call_frames
                .last_mut()
                .map(|call_frame| &mut call_frame.block_arguments[*index])
                .expect("the block argument should exist"),
            Value::CallArgument(index) => self
                .call_frames
                .last_mut()
                .map(|call_frame| &mut call_frame.call_arguments[*index])
                .expect("the call argument should exist"),
            Value::Address(address) => {
                let offset = self
                    .call_frames
                    .iter()
                    .rfind(|frame| frame.block_index == address.block_index)
                    .map(|frame| frame.fp + address.offset)
                    .expect("couldn't calculate offset from fp");

                self.dereference_address_mut(offset)
            }
            Value::Register(index) => self.dereference_register_mut(*index),
            _ => {
                unreachable!("dereference_value_mut should never be used with a normal value");
            }
        }
    }

    #[must_use]
    fn dereference_address_mut(&mut self, offset: usize) -> &mut Value {
        match &mut self.stack[offset] {
            Value::BlockArgument(index) => self
                .call_frames
                .last_mut()
                .map(|call_frame| &mut call_frame.block_arguments[*index])
                .expect("the block argument should exist"),
            Value::CallArgument(index) => self
                .call_frames
                .last_mut()
                .map(|call_frame| &mut call_frame.call_arguments[*index])
                .expect("the call argument should exist"),
            Value::Address(address) => {
                let address = *address;

                self.dereference_address_mut(
                    self.call_frames
                        .iter()
                        .rfind(|frame| frame.block_index == address.block_index)
                        .map(|frame| frame.fp + address.offset)
                        .expect("couldn't calculate offset from fp"),
                )
            }
            Value::Register(index) => {
                let index = *index;

                self.dereference_register_mut(index)
            }
            _ => &mut self.stack[offset],
        }
    }

    #[must_use]
    fn dereference_register_mut(&mut self, index: usize) -> &mut Value {
        match &mut self.registers[index] {
            Value::BlockArgument(index) => self
                .call_frames
                .last_mut()
                .map(|call_frame| &mut call_frame.block_arguments[*index])
                .expect("the block argument should exist"),
            Value::CallArgument(index) => self
                .call_frames
                .last_mut()
                .map(|call_frame| &mut call_frame.call_arguments[*index])
                .expect("the call argument should exist"),
            Value::Address(address) => {
                let address = *address;

                self.dereference_address_mut(
                    self.call_frames
                        .iter()
                        .rfind(|frame| frame.block_index == address.block_index)
                        .map(|frame| frame.fp + address.offset)
                        .expect("couldn't calculate offset from fp"),
                )
            }
            Value::Register(index) => {
                let index = *index;

                self.dereference_register_mut(index)
            }
            _ => &mut self.registers[index],
        }
    }

    #[must_use]
    fn evaluate_unary(&self, (op, operand): (UnaryOp, &Value)) -> Value {
        match (op, self.dereference_value(operand)) {
            (UnaryOp::Not, Value::Boolean(value)) => Value::Boolean(!value),
            (UnaryOp::Negate, Value::Integer(value)) => Value::Integer(-value),
            (UnaryOp::Negate, Value::Float(value)) => Value::Float(-value),
            (_, _) => unreachable!("type checking would have caught the wrong operand type"),
        }
    }

    #[allow(clippy::too_many_lines)]
    #[must_use]
    fn evaluate_binary(&self, (op, lhs, rhs): (BinaryOp, &Value, &Value)) -> Value {
        match (op, self.dereference_value(lhs), self.dereference_value(rhs)) {
            (BinaryOp::Multiply, Value::Integer(lhs), Value::Integer(rhs)) => {
                Value::Integer(lhs * rhs)
            }
            (BinaryOp::Divide, Value::Integer(lhs), Value::Integer(rhs)) => {
                Value::Integer(lhs / rhs)
            }
            (BinaryOp::Remainder, Value::Integer(lhs), Value::Integer(rhs)) => {
                Value::Integer(lhs % rhs)
            }
            (BinaryOp::Add, Value::Integer(lhs), Value::Integer(rhs)) => Value::Integer(lhs + rhs),
            (BinaryOp::Subtract, Value::Integer(lhs), Value::Integer(rhs)) => {
                Value::Integer(lhs - rhs)
            }
            (BinaryOp::Less, Value::Integer(lhs), Value::Integer(rhs)) => Value::Boolean(lhs < rhs),
            (BinaryOp::Greater, Value::Integer(lhs), Value::Integer(rhs)) => {
                Value::Boolean(lhs > rhs)
            }
            (BinaryOp::LessOrEqual, Value::Integer(lhs), Value::Integer(rhs)) => {
                Value::Boolean(lhs <= rhs)
            }
            (BinaryOp::GreaterOrEqual, Value::Integer(lhs), Value::Integer(rhs)) => {
                Value::Boolean(lhs >= rhs)
            }
            (BinaryOp::Multiply, Value::Float(lhs), Value::Float(rhs)) => Value::Float(lhs * rhs),
            (BinaryOp::Divide, Value::Float(lhs), Value::Float(rhs)) => Value::Float(lhs / rhs),
            (BinaryOp::Remainder, Value::Float(lhs), Value::Float(rhs)) => Value::Float(lhs % rhs),
            (BinaryOp::Add, Value::Float(lhs), Value::Float(rhs)) => Value::Float(lhs + rhs),
            (BinaryOp::Subtract, Value::Float(lhs), Value::Float(rhs)) => Value::Float(lhs - rhs),
            (BinaryOp::Less, Value::Float(lhs), Value::Float(rhs)) => Value::Boolean(lhs < rhs),
            (BinaryOp::Greater, Value::Float(lhs), Value::Float(rhs)) => Value::Boolean(lhs > rhs),
            (BinaryOp::LessOrEqual, Value::Float(lhs), Value::Float(rhs)) => {
                Value::Boolean(lhs <= rhs)
            }
            (BinaryOp::GreaterOrEqual, Value::Float(lhs), Value::Float(rhs)) => {
                Value::Boolean(lhs >= rhs)
            }
            (BinaryOp::Equal, Value::Float(lhs), Value::Float(rhs)) => {
                Value::Boolean((lhs - rhs).abs() < f64::EPSILON)
            }
            (BinaryOp::NotEqual, Value::Float(lhs), Value::Float(rhs)) => {
                Value::Boolean((lhs - rhs).abs() >= f64::EPSILON)
            }
            (BinaryOp::Equal, lhs, rhs) => Value::Boolean(lhs == rhs),
            (BinaryOp::NotEqual, lhs, rhs) => Value::Boolean(lhs != rhs),
            (_, _, _) => unreachable!("type checking would have caught the wrong operand type"),
        }
    }
}
