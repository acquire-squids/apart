use crate::{
    Span,
    basic_blocks::{Instruction, Value},
    parse::{BinaryOp, UnaryOp},
    ssa::{Argument, BlockTerminator, Ssa},
};

use std::{collections::HashMap, io::Write};

struct CallFrame {
    call_arguments: Vec<Value>,
    block_arguments: Vec<Value>,
    from: (usize, usize),
}

#[allow(clippy::too_many_lines)]
pub fn run<O>(ssa: &Ssa, sources: &[(usize, &str)], out: &mut O)
where
    O: Write,
{
    let mut call_frames = vec![CallFrame {
        call_arguments: vec![],
        block_arguments: vec![],
        from: (0, 0),
    }];

    let mut stack = vec![];

    let mut variables = HashMap::new();

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
                Instruction::Unary { op, operand } => {
                    evaluate_unary(
                        &mut variables,
                        call_frames.as_slice(),
                        (call_frames.len(), b, i),
                        (*op, operand),
                    );
                }
                Instruction::Binary { op, lhs, rhs } => {
                    evaluate_binary(
                        &mut variables,
                        call_frames.as_slice(),
                        (call_frames.len(), b, i),
                        (*op, lhs, rhs),
                    );
                }
                Instruction::Assign { value, to } => {
                    variables.insert(
                        (call_frames.len(), usize::from(to.block_index), to.offset),
                        dereference_value(&variables, call_frames.as_slice(), value).clone(),
                    );
                }
                Instruction::Push(value) => {
                    stack
                        .push(dereference_value(&variables, call_frames.as_slice(), value).clone());
                }
                Instruction::Pop => {
                    let Some(call_frame) = call_frames.last_mut() else {
                        unreachable!("cannot pop outside of a call");
                    };

                    call_frame
                        .call_arguments
                        .push(stack.pop().expect("call argument stack underflow"));
                }
                Instruction::Call(callee) => {
                    match dereference_value(&variables, call_frames.as_slice(), callee) {
                        Value::Fn(callee) => {
                            let callee = *callee;

                            call_frames.push(CallFrame {
                                block_arguments: vec![],
                                call_arguments: vec![],
                                from: (b, i),
                            });

                            b = usize::from(callee);
                            i = 0;

                            continue 'block;
                        }
                        Value::NativeFn(span) => {
                            let span = *span;

                            native_fn_call(
                                sources,
                                &mut variables,
                                call_frames.as_slice(),
                                &mut stack,
                                (call_frames.len(), b, i),
                                span,
                                out,
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

                let block_args = jump_to
                    .arguments()
                    .iter()
                    .map(|argument| {
                        match argument {
                            Argument::Address(address) => dereference_address(
                                &variables,
                                call_frames.as_slice(),
                                (
                                    call_frames.len(),
                                    usize::from(address.block_index),
                                    address.offset,
                                ),
                            ),
                            Argument::Passthrough(i) => call_frames
                                .last_mut()
                                .expect("there will always be a call frame")
                                .block_arguments
                                .get(*i)
                                .expect("passthrough block argument didn't exist"),
                        }
                        .clone()
                    })
                    .collect::<Vec<_>>();

                call_frames
                    .last_mut()
                    .expect("there will always be a call frame")
                    .block_arguments = block_args;
            }
            BlockTerminator::Branch {
                condition,
                when_true,
                otherwise,
            } => match dereference_value(&variables, call_frames.as_slice(), condition) {
                Value::Boolean(true) => {
                    b = usize::from(when_true.block());
                    i = 0;

                    let block_args = when_true
                        .arguments()
                        .iter()
                        .map(|argument| {
                            match argument {
                                Argument::Address(address) => dereference_address(
                                    &variables,
                                    call_frames.as_slice(),
                                    (
                                        call_frames.len(),
                                        usize::from(address.block_index),
                                        address.offset,
                                    ),
                                ),
                                Argument::Passthrough(i) => call_frames
                                    .last_mut()
                                    .expect("there will always be a call frame")
                                    .block_arguments
                                    .get(*i)
                                    .expect("passthrough block argument didn't exist"),
                            }
                            .clone()
                        })
                        .collect::<Vec<_>>();

                    call_frames
                        .last_mut()
                        .expect("there will always be a call frame")
                        .block_arguments = block_args;
                }
                Value::Boolean(false) => {
                    b = usize::from(otherwise.block());
                    i = 0;

                    let block_args = otherwise
                        .arguments()
                        .iter()
                        .map(|argument| {
                            match argument {
                                Argument::Address(address) => dereference_address(
                                    &variables,
                                    call_frames.as_slice(),
                                    (
                                        call_frames.len(),
                                        usize::from(address.block_index),
                                        address.offset,
                                    ),
                                ),
                                Argument::Passthrough(i) => call_frames
                                    .last_mut()
                                    .expect("there will always be a call frame")
                                    .block_arguments
                                    .get(*i)
                                    .expect("passthrough block argument didn't exist"),
                            }
                            .clone()
                        })
                        .collect::<Vec<_>>();

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
                let return_value =
                    dereference_value(&variables, call_frames.as_slice(), value).clone();

                if let Some(call_frame) = call_frames.pop() {
                    b = call_frame.from.0;
                    i = call_frame.from.1;

                    variables.insert((call_frames.len(), b, i), return_value);

                    i += 1;
                }

                if call_frames.is_empty() {
                    break 'block;
                }
            }
        }
    }
}

fn native_fn_call<O>(
    sources: &[(usize, &str)],
    variables: &mut HashMap<(usize, usize, usize), Value>,
    call_frames: &[CallFrame],
    stack: &mut Vec<Value>,
    (call_depth, b, i): (usize, usize, usize),
    span: Span,
    out: &mut O,
) where
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
                .map(|value| dereference_value(variables, call_frames, value))
            else {
                unreachable!("type checking would have caught an incorrect argument");
            };

            writeln!(out, "{value}").expect("failed to write to output");

            variables.insert((call_depth, b, i), Value::Unit);
        }
        "print_f64" => {
            let value = stack.pop();

            let Some(Value::Float(value)) = value
                .as_ref()
                .map(|value| dereference_value(variables, call_frames, value))
            else {
                unreachable!("type checking would have caught an incorrect argument");
            };

            writeln!(out, "{value:?}").expect("failed to write to output");

            variables.insert((call_depth, b, i), Value::Unit);
        }
        "print_bool" => {
            let value = stack.pop();

            let Some(Value::Boolean(value)) = value
                .as_ref()
                .map(|value| dereference_value(variables, call_frames, value))
            else {
                unreachable!("type checking would have caught an incorrect argument");
            };

            writeln!(out, "{value}").expect("failed to write to output");

            variables.insert((call_depth, b, i), Value::Unit);
        }
        "print_unit" => {
            let value = stack.pop();

            let Some(Value::Unit) = value
                .as_ref()
                .map(|value| dereference_value(variables, call_frames, value))
            else {
                unreachable!("type checking would have caught an incorrect argument");
            };

            writeln!(out, "{{}}").expect("failed to write to output");

            variables.insert((call_depth, b, i), Value::Unit);
        }
        _ => {
            unreachable!("tried to call an unknown native function");
        }
    }
}

fn dereference_value<'a>(
    variables: &'a HashMap<(usize, usize, usize), Value>,
    call_frames: &'a [CallFrame],
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

            if argument.is_none() {
                println!("{value:?}");
            }

            argument.expect("call argument not found")
        }
        Value::Address(address_block) => dereference_address(
            variables,
            call_frames,
            (
                call_frames.len(),
                usize::from(address_block.block_index),
                address_block.offset,
            ),
        ),
        _ => value,
    }
}

fn dereference_address<'a>(
    variables: &'a HashMap<(usize, usize, usize), Value>,
    call_frames: &'a [CallFrame],
    (call_depth, b, i): (usize, usize, usize),
) -> &'a Value {
    match variables.get(&(call_depth, b, i)) {
        None => call_depth
            .checked_sub(1)
            .map(|call_depth| dereference_address(variables, call_frames, (call_depth, b, i)))
            .expect("value doesn't exist"),
        Some(Value::Argument(offset)) => call_frames
            .last()
            .and_then(|call_frame| call_frame.call_arguments.get(*offset))
            .expect("call argument not found"),
        Some(Value::Address(address_block)) => dereference_address(
            variables,
            call_frames,
            (
                call_depth,
                usize::from(address_block.block_index),
                address_block.offset,
            ),
        ),
        Some(value) => value,
    }
}

fn evaluate_unary(
    variables: &mut HashMap<(usize, usize, usize), Value>,
    call_frames: &[CallFrame],
    (call_depth, b, i): (usize, usize, usize),
    (op, operand): (UnaryOp, &Value),
) {
    match (op, dereference_value(variables, call_frames, operand)) {
        (UnaryOp::Not, Value::Boolean(value)) => {
            variables.insert((call_depth, b, i), Value::Boolean(!value));
        }
        (UnaryOp::Negate, Value::Integer(value)) => {
            variables.insert((call_depth, b, i), Value::Integer(-value));
        }
        (UnaryOp::Negate, Value::Float(value)) => {
            variables.insert((call_depth, b, i), Value::Float(-value));
        }
        (_, _) => unreachable!("type checking would have caught the wrong operand type"),
    }
}

#[allow(clippy::too_many_lines)]
fn evaluate_binary(
    variables: &mut HashMap<(usize, usize, usize), Value>,
    call_frames: &[CallFrame],
    (call_depth, b, i): (usize, usize, usize),
    (op, lhs, rhs): (BinaryOp, &Value, &Value),
) {
    match (
        op,
        dereference_value(variables, call_frames, lhs),
        dereference_value(variables, call_frames, rhs),
    ) {
        (BinaryOp::Multiply, Value::Integer(lhs), Value::Integer(rhs)) => {
            variables.insert((call_depth, b, i), Value::Integer(lhs * rhs));
        }
        (BinaryOp::Divide, Value::Integer(lhs), Value::Integer(rhs)) => {
            variables.insert((call_depth, b, i), Value::Integer(lhs / rhs));
        }
        (BinaryOp::Remainder, Value::Integer(lhs), Value::Integer(rhs)) => {
            variables.insert((call_depth, b, i), Value::Integer(lhs % rhs));
        }
        (BinaryOp::Add, Value::Integer(lhs), Value::Integer(rhs)) => {
            variables.insert((call_depth, b, i), Value::Integer(lhs + rhs));
        }
        (BinaryOp::Subtract, Value::Integer(lhs), Value::Integer(rhs)) => {
            variables.insert((call_depth, b, i), Value::Integer(lhs - rhs));
        }
        (BinaryOp::Less, Value::Integer(lhs), Value::Integer(rhs)) => {
            variables.insert((call_depth, b, i), Value::Boolean(lhs < rhs));
        }
        (BinaryOp::Greater, Value::Integer(lhs), Value::Integer(rhs)) => {
            variables.insert((call_depth, b, i), Value::Boolean(lhs > rhs));
        }
        (BinaryOp::LessOrEqual, Value::Integer(lhs), Value::Integer(rhs)) => {
            variables.insert((call_depth, b, i), Value::Boolean(lhs <= rhs));
        }
        (BinaryOp::GreaterOrEqual, Value::Integer(lhs), Value::Integer(rhs)) => {
            variables.insert((call_depth, b, i), Value::Boolean(lhs >= rhs));
        }
        (BinaryOp::Multiply, Value::Float(lhs), Value::Float(rhs)) => {
            variables.insert((call_depth, b, i), Value::Float(lhs * rhs));
        }
        (BinaryOp::Divide, Value::Float(lhs), Value::Float(rhs)) => {
            variables.insert((call_depth, b, i), Value::Float(lhs / rhs));
        }
        (BinaryOp::Remainder, Value::Float(lhs), Value::Float(rhs)) => {
            variables.insert((call_depth, b, i), Value::Float(lhs % rhs));
        }
        (BinaryOp::Add, Value::Float(lhs), Value::Float(rhs)) => {
            variables.insert((call_depth, b, i), Value::Float(lhs + rhs));
        }
        (BinaryOp::Subtract, Value::Float(lhs), Value::Float(rhs)) => {
            variables.insert((call_depth, b, i), Value::Float(lhs - rhs));
        }
        (BinaryOp::Less, Value::Float(lhs), Value::Float(rhs)) => {
            variables.insert((call_depth, b, i), Value::Boolean(lhs < rhs));
        }
        (BinaryOp::Greater, Value::Float(lhs), Value::Float(rhs)) => {
            variables.insert((call_depth, b, i), Value::Boolean(lhs > rhs));
        }
        (BinaryOp::LessOrEqual, Value::Float(lhs), Value::Float(rhs)) => {
            variables.insert((call_depth, b, i), Value::Boolean(lhs <= rhs));
        }
        (BinaryOp::GreaterOrEqual, Value::Float(lhs), Value::Float(rhs)) => {
            variables.insert((call_depth, b, i), Value::Boolean(lhs >= rhs));
        }
        (BinaryOp::Equal, Value::Integer(lhs), Value::Integer(rhs)) => {
            variables.insert((call_depth, b, i), Value::Boolean(lhs == rhs));
        }
        (BinaryOp::NotEqual, Value::Integer(lhs), Value::Integer(rhs)) => {
            variables.insert((call_depth, b, i), Value::Boolean(lhs != rhs));
        }
        (BinaryOp::Equal, Value::Float(lhs), Value::Float(rhs)) => {
            variables.insert(
                (call_depth, b, i),
                Value::Boolean((lhs - rhs).abs() < f64::EPSILON),
            );
        }
        (BinaryOp::NotEqual, Value::Float(lhs), Value::Float(rhs)) => {
            variables.insert(
                (call_depth, b, i),
                Value::Boolean((lhs - rhs).abs() >= f64::EPSILON),
            );
        }
        (BinaryOp::Equal, Value::Boolean(lhs), Value::Boolean(rhs)) => {
            variables.insert((call_depth, b, i), Value::Boolean(lhs == rhs));
        }
        (BinaryOp::NotEqual, Value::Boolean(lhs), Value::Boolean(rhs)) => {
            variables.insert((call_depth, b, i), Value::Boolean(lhs != rhs));
        }
        (BinaryOp::Equal, Value::Unit, Value::Unit) => {
            variables.insert((call_depth, b, i), Value::Boolean(true));
        }
        (BinaryOp::NotEqual, Value::Unit, Value::Unit) => {
            variables.insert((call_depth, b, i), Value::Boolean(false));
        }
        (BinaryOp::Equal, Value::Fn(lhs), Value::Fn(rhs)) => {
            variables.insert((call_depth, b, i), Value::Boolean(lhs == rhs));
        }
        (BinaryOp::NotEqual, Value::Fn(lhs), Value::Fn(rhs)) => {
            variables.insert((call_depth, b, i), Value::Boolean(lhs != rhs));
        }
        (BinaryOp::Equal, Value::NativeFn(lhs), Value::NativeFn(rhs)) => {
            variables.insert((call_depth, b, i), Value::Boolean(lhs == rhs));
        }
        (BinaryOp::NotEqual, Value::NativeFn(lhs), Value::NativeFn(rhs)) => {
            variables.insert((call_depth, b, i), Value::Boolean(lhs != rhs));
        }
        (_, _, _) => unreachable!("type checking would have caught the wrong operand type"),
    }
}
