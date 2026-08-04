use crate::{
    Span,
    basic_blocks::{BlockIndex, Instruction, Value},
    parse::{BinaryOp, UnaryOp},
    ssa::{Argument, BlockTerminator, JumpTo, Ssa},
};

use std::io::Write;

struct CallFrame {
    call_arguments: Vec<Value>,
    block_arguments: Vec<Value>,
    from: (usize, usize),
    fp: usize,
    block_index: BlockIndex,
    previous_registers: Vec<(usize, Value)>,
}

#[allow(clippy::too_many_lines)]
pub fn run<const MAX_REGISTERS: usize, O>(ssa: &Ssa, sources: &[(usize, &str)], out: &mut O)
where
    O: Write,
{
    let mut call_frames = vec![CallFrame {
        call_arguments: vec![],
        block_arguments: vec![],
        from: (0, 0),
        fp: 0,
        block_index: BlockIndex(0),
        previous_registers: vec![],
    }];

    let mut stack = vec![];

    let mut registers = [const { Value::Runtime }; MAX_REGISTERS];

    let mut fp = 0;

    let mut b = 0;
    let mut i = 0;

    'block: while let Some(block) = ssa.blocks().get(b) {
        assert_eq!(
            block.parameters().len(),
            call_frames
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
                    let value = evaluate_unary(
                        stack.as_slice(),
                        call_frames.as_slice(),
                        registers.as_slice(),
                        (*op, operand),
                    );

                    assign(
                        &mut stack,
                        call_frames.as_mut_slice(),
                        registers.as_mut_slice(),
                        to,
                        value,
                    );
                }
                Instruction::Binary {
                    op,
                    lhs,
                    rhs,
                    temporary: to,
                } => {
                    let value = evaluate_binary(
                        stack.as_slice(),
                        call_frames.as_slice(),
                        registers.as_slice(),
                        (*op, lhs, rhs),
                    );

                    assign(
                        &mut stack,
                        call_frames.as_mut_slice(),
                        registers.as_mut_slice(),
                        to,
                        value,
                    );
                }
                Instruction::Assign { value, to } => {
                    let value = dereference_value(
                        stack.as_slice(),
                        call_frames.as_slice(),
                        registers.as_slice(),
                        value,
                    )
                    .clone();

                    assign(
                        &mut stack,
                        call_frames.as_mut_slice(),
                        registers.as_mut_slice(),
                        to,
                        value,
                    );
                }
                Instruction::Push(value) => {
                    fp = stack.len();

                    stack.push(
                        dereference_value(
                            stack.as_slice(),
                            call_frames.as_slice(),
                            registers.as_slice(),
                            value,
                        )
                        .clone(),
                    );
                }
                Instruction::Pop => {
                    let Some(call_frame) = call_frames.last_mut() else {
                        unreachable!("cannot pop outside of a call");
                    };

                    call_frame
                        .call_arguments
                        .push(stack.pop().expect("call argument stack underflow"));
                }
                Instruction::Call {
                    callee,
                    temporary: to,
                } => {
                    match dereference_value(
                        stack.as_slice(),
                        call_frames.as_slice(),
                        registers.as_slice(),
                        callee,
                    ) {
                        Value::Fn(callee) => {
                            let callee = *callee;

                            call_frames.push(CallFrame {
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

                            let value = native_fn_call(
                                sources,
                                &mut stack,
                                call_frames.as_slice(),
                                registers.as_slice(),
                                span,
                                out,
                            );

                            assign(
                                &mut stack,
                                call_frames.as_mut_slice(),
                                registers.as_mut_slice(),
                                to,
                                value,
                            );
                        }
                        _ => {
                            unreachable!("type checking would have caught calling an uncallable");
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

                let block_args = collect_block_arguments(
                    stack.as_slice(),
                    call_frames.as_slice(),
                    registers.as_slice(),
                    jump_to,
                );

                call_frames
                    .last_mut()
                    .expect("there will always be a call frame")
                    .block_arguments = block_args;
            }
            BlockTerminator::Branch {
                condition,
                when_true,
                otherwise,
            } => match dereference_value(
                stack.as_slice(),
                call_frames.as_slice(),
                registers.as_slice(),
                condition,
            ) {
                Value::Boolean(true) => {
                    b = usize::from(when_true.block());
                    i = 0;

                    let block_args = collect_block_arguments(
                        stack.as_slice(),
                        call_frames.as_slice(),
                        registers.as_slice(),
                        when_true,
                    );

                    call_frames
                        .last_mut()
                        .expect("there will always be a call frame")
                        .block_arguments = block_args;
                }
                Value::Boolean(false) => {
                    b = usize::from(otherwise.block());
                    i = 0;

                    let block_args = collect_block_arguments(
                        stack.as_slice(),
                        call_frames.as_slice(),
                        registers.as_slice(),
                        otherwise,
                    );

                    call_frames
                        .last_mut()
                        .expect("there will always be a call frame")
                        .block_arguments = block_args;
                }
                _ => {
                    unreachable!("type checking guarantees conditions are boolean");
                }
            },
            BlockTerminator::Return(value) => {
                let value = dereference_value(
                    stack.as_slice(),
                    call_frames.as_slice(),
                    registers.as_slice(),
                    value,
                )
                .clone();

                if let Some(call_frame) = call_frames.pop() {
                    b = call_frame.from.0;
                    i = call_frame.from.1;

                    stack.truncate(call_frame.fp);

                    for (register_index, value) in call_frame.previous_registers {
                        registers[register_index] = value;
                    }

                    if !call_frames.is_empty() {
                        let Some(Instruction::Call { temporary: to, .. }) = ssa
                            .blocks()
                            .get(b)
                            .and_then(|block| block.instructions().get(i))
                        else {
                            unreachable!("call frames only come from calls");
                        };

                        assign(
                            &mut stack,
                            call_frames.as_mut_slice(),
                            registers.as_mut_slice(),
                            to,
                            value,
                        );
                    }

                    i += 1;
                }

                if call_frames.is_empty() {
                    break 'block;
                }
            }
        }
    }
}

fn assign(
    stack: &mut Vec<Value>,
    call_frames: &mut [CallFrame],
    registers: &mut [Value],
    to: &Value,
    value: Value,
) {
    match to {
        Value::Register(index) => {
            let Some(previous_registers) = call_frames
                .last_mut()
                .map(|frame| &mut frame.previous_registers)
            else {
                unreachable!("assignments only happen in calls");
            };

            if !previous_registers.iter().any(|(to, _)| to == index) {
                previous_registers.push((*index, registers[*index].clone()));
            }

            registers[*index] = value;
        }
        Value::Address(address) => {
            if let Some(stack_value) = call_frames
                .iter()
                .rfind(|frame| frame.block_index == address.block_index)
                .map(|frame| frame.fp + address.offset)
                .and_then(|offset| stack.get_mut(offset))
            {
                *stack_value = value;
            } else {
                stack.push(value);
            }
        }
        _ => unreachable!("only registers and stack slots can be assigned to"),
    }
}

#[must_use]
fn native_fn_call<O>(
    sources: &[(usize, &str)],
    stack: &mut Vec<Value>,
    call_frames: &[CallFrame],
    registers: &[Value],
    span: Span,
    out: &mut O,
) -> Value
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
            let value = stack.pop();

            let Some(Value::Integer(value)) = value
                .as_ref()
                .map(|value| dereference_value(stack, call_frames, registers, value))
            else {
                unreachable!("type checking would have caught an incorrect argument");
            };

            writeln!(out, "{value}").expect("failed to write to output");

            Value::Unit
        }
        "print_f64" => {
            let value = stack.pop();

            let Some(Value::Float(value)) = value
                .as_ref()
                .map(|value| dereference_value(stack, call_frames, registers, value))
            else {
                unreachable!("type checking would have caught an incorrect argument");
            };

            writeln!(out, "{value:?}").expect("failed to write to output");

            Value::Unit
        }
        "print_bool" => {
            let value = stack.pop();

            let Some(Value::Boolean(value)) = value
                .as_ref()
                .map(|value| dereference_value(stack, call_frames, registers, value))
            else {
                unreachable!("type checking would have caught an incorrect argument");
            };

            writeln!(out, "{value}").expect("failed to write to output");

            Value::Unit
        }
        "print_unit" => {
            let value = stack.pop();

            let Some(Value::Unit) = value
                .as_ref()
                .map(|value| dereference_value(stack, call_frames, registers, value))
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
fn collect_block_arguments(
    stack: &[Value],
    call_frames: &[CallFrame],
    registers: &[Value],
    jump_to: &JumpTo,
) -> Vec<Value> {
    jump_to
        .arguments()
        .iter()
        .map(|argument| {
            match argument {
                Argument::Address(Value::Address(address)) => dereference_address(
                    stack,
                    call_frames,
                    registers,
                    call_frames
                        .iter()
                        .rfind(|frame| frame.block_index == address.block_index)
                        .map(|frame| frame.fp + address.offset)
                        .expect("couldn't calculate offset from fp"),
                ),
                Argument::Address(Value::Register(index)) => {
                    dereference_register(stack, call_frames, registers, *index)
                }
                Argument::Address(_) => {
                    unreachable!("only registers and stack slots can be block arguments")
                }
                Argument::Passthrough(i) => call_frames
                    .last()
                    .expect("there will always be a call frame")
                    .block_arguments
                    .get(*i)
                    .expect("passthrough block argument didn't exist"),
            }
            .clone()
        })
        .collect::<Vec<_>>()
}

#[must_use]
fn dereference_value<'a>(
    stack: &'a [Value],
    call_frames: &'a [CallFrame],
    registers: &'a [Value],
    value: &'a Value,
) -> &'a Value {
    match value {
        Value::Argument(offset) => {
            let argument = call_frames.last().and_then(|call_frame| {
                if *offset >= call_frame.block_arguments.len() {
                    call_frame
                        .call_arguments
                        .get(*offset - call_frame.block_arguments.len())
                } else {
                    call_frame.block_arguments.get(*offset)
                }
            });

            argument.expect("call argument not found")
        }
        Value::Address(address) => dereference_address(
            stack,
            call_frames,
            registers,
            call_frames
                .iter()
                .rfind(|frame| frame.block_index == address.block_index)
                .map(|frame| frame.fp + address.offset)
                .expect("couldn't calculate offset from fp"),
        ),
        Value::Register(index) => dereference_register(stack, call_frames, registers, *index),
        _ => value,
    }
}

#[must_use]
fn dereference_address<'a>(
    stack: &'a [Value],
    call_frames: &'a [CallFrame],
    registers: &'a [Value],
    offset: usize,
) -> &'a Value {
    match stack.get(offset).expect("value doesn't exist") {
        Value::Argument(offset) => call_frames
            .last()
            .and_then(|call_frame| call_frame.call_arguments.get(*offset))
            .expect("call argument not found"),
        Value::Address(address) => dereference_address(
            stack,
            call_frames,
            registers,
            call_frames
                .iter()
                .rfind(|frame| frame.block_index == address.block_index)
                .map(|frame| frame.fp + address.offset)
                .expect("couldn't calculate offset from fp"),
        ),
        Value::Register(index) => dereference_register(stack, call_frames, registers, *index),
        value => value,
    }
}

#[must_use]
fn dereference_register<'a>(
    stack: &'a [Value],
    call_frames: &'a [CallFrame],
    registers: &'a [Value],
    index: usize,
) -> &'a Value {
    match registers.get(index).expect("value doesn't exist") {
        Value::Argument(offset) => call_frames
            .last()
            .and_then(|call_frame| call_frame.call_arguments.get(*offset))
            .expect("call argument not found"),
        Value::Address(address) => dereference_address(
            stack,
            call_frames,
            registers,
            call_frames
                .iter()
                .rfind(|frame| frame.block_index == address.block_index)
                .map(|frame| frame.fp + address.offset)
                .expect("couldn't calculate offset from fp"),
        ),
        Value::Register(index) => dereference_register(stack, call_frames, registers, *index),
        value => value,
    }
}

#[must_use]
fn evaluate_unary(
    stack: &[Value],
    call_frames: &[CallFrame],
    registers: &[Value],
    (op, operand): (UnaryOp, &Value),
) -> Value {
    match (
        op,
        dereference_value(stack, call_frames, registers, operand),
    ) {
        (UnaryOp::Not, Value::Boolean(value)) => Value::Boolean(!value),
        (UnaryOp::Negate, Value::Integer(value)) => Value::Integer(-value),
        (UnaryOp::Negate, Value::Float(value)) => Value::Float(-value),
        (_, _) => unreachable!("type checking would have caught the wrong operand type"),
    }
}

#[allow(clippy::too_many_lines)]
#[must_use]
fn evaluate_binary(
    stack: &[Value],
    call_frames: &[CallFrame],
    registers: &[Value],
    (op, lhs, rhs): (BinaryOp, &Value, &Value),
) -> Value {
    match (
        op,
        dereference_value(stack, call_frames, registers, lhs),
        dereference_value(stack, call_frames, registers, rhs),
    ) {
        (BinaryOp::Multiply, Value::Integer(lhs), Value::Integer(rhs)) => Value::Integer(lhs * rhs),
        (BinaryOp::Divide, Value::Integer(lhs), Value::Integer(rhs)) => Value::Integer(lhs / rhs),
        (BinaryOp::Remainder, Value::Integer(lhs), Value::Integer(rhs)) => {
            Value::Integer(lhs % rhs)
        }
        (BinaryOp::Add, Value::Integer(lhs), Value::Integer(rhs)) => Value::Integer(lhs + rhs),
        (BinaryOp::Subtract, Value::Integer(lhs), Value::Integer(rhs)) => Value::Integer(lhs - rhs),
        (BinaryOp::Less, Value::Integer(lhs), Value::Integer(rhs)) => Value::Boolean(lhs < rhs),
        (BinaryOp::Greater, Value::Integer(lhs), Value::Integer(rhs)) => Value::Boolean(lhs > rhs),
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
        (BinaryOp::LessOrEqual, Value::Float(lhs), Value::Float(rhs)) => Value::Boolean(lhs <= rhs),
        (BinaryOp::GreaterOrEqual, Value::Float(lhs), Value::Float(rhs)) => {
            Value::Boolean(lhs >= rhs)
        }
        (BinaryOp::Equal, Value::Integer(lhs), Value::Integer(rhs)) => Value::Boolean(lhs == rhs),
        (BinaryOp::NotEqual, Value::Integer(lhs), Value::Integer(rhs)) => {
            Value::Boolean(lhs != rhs)
        }
        (BinaryOp::Equal, Value::Float(lhs), Value::Float(rhs)) => {
            Value::Boolean((lhs - rhs).abs() < f64::EPSILON)
        }
        (BinaryOp::NotEqual, Value::Float(lhs), Value::Float(rhs)) => {
            Value::Boolean((lhs - rhs).abs() >= f64::EPSILON)
        }
        (BinaryOp::Equal, Value::Boolean(lhs), Value::Boolean(rhs)) => Value::Boolean(lhs == rhs),
        (BinaryOp::NotEqual, Value::Boolean(lhs), Value::Boolean(rhs)) => {
            Value::Boolean(lhs != rhs)
        }
        (BinaryOp::Equal, Value::Unit, Value::Unit) => Value::Boolean(true),
        (BinaryOp::NotEqual, Value::Unit, Value::Unit) => Value::Boolean(false),
        (BinaryOp::Equal, Value::Fn(lhs), Value::Fn(rhs)) => Value::Boolean(lhs == rhs),
        (BinaryOp::NotEqual, Value::Fn(lhs), Value::Fn(rhs)) => Value::Boolean(lhs != rhs),
        (BinaryOp::Equal, Value::NativeFn(lhs), Value::NativeFn(rhs)) => Value::Boolean(lhs == rhs),
        (BinaryOp::NotEqual, Value::NativeFn(lhs), Value::NativeFn(rhs)) => {
            Value::Boolean(lhs != rhs)
        }
        (_, _, _) => unreachable!("type checking would have caught the wrong operand type"),
    }
}
