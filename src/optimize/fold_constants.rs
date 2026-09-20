use crate::{
    basic_blocks::{Address, BlockIndex, Instruction, Value},
    parse::{BinaryOp, UnaryOp},
    ssa::Ssa,
};

pub fn optimize(ssa: &mut Ssa) -> bool {
    let mut changed = false;

    for (b, block) in ssa.blocks_mut().iter_mut().enumerate() {
        for (i, instruction) in block.instructions_mut().iter_mut().enumerate() {
            match instruction {
                Instruction::NoOp
                | Instruction::Assign { .. }
                | Instruction::Push(_)
                | Instruction::Call { .. }
                | Instruction::Access { .. }
                | Instruction::AccessAssign { .. } => {}
                Instruction::Unary { .. } => {
                    fold_unary(b, i, instruction, &mut changed);
                }
                Instruction::Binary { .. } => {
                    fold_binary(b, i, instruction, &mut changed);
                }
            }
        }
    }

    changed
}

fn fold_unary(b: usize, i: usize, instruction: &mut Instruction, changed: &mut bool) {
    if let Instruction::Unary { op, operand, .. } = instruction {
        match (op, operand) {
            (UnaryOp::Not, Value::Boolean(value)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(!*value),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (UnaryOp::Negate, Value::I64(value)) => {
                *instruction = Instruction::Assign {
                    value: Value::I64(-*value),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (UnaryOp::Negate, Value::F64(value)) => {
                *instruction = Instruction::Assign {
                    value: Value::F64(-*value),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (UnaryOp::Not | UnaryOp::Negate, _) => {}
        }
    }
}

#[allow(clippy::too_many_lines)]
fn fold_binary(b: usize, i: usize, instruction: &mut Instruction, changed: &mut bool) {
    if let Instruction::Binary { op, lhs, rhs, .. } = instruction {
        match (op, lhs, rhs) {
            (BinaryOp::Multiply, Value::I64(lhs), Value::I64(rhs))
                if let Some(value) = lhs.checked_mul(*rhs) =>
            {
                *instruction = Instruction::Assign {
                    value: Value::I64(value),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::Multiply, Value::F64(lhs), Value::F64(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::F64(*lhs * *rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::Divide, Value::I64(lhs), Value::I64(rhs))
                if let Some(value) = lhs.checked_div(*rhs) =>
            {
                *instruction = Instruction::Assign {
                    value: Value::I64(value),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::Divide, Value::F64(lhs), Value::F64(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::F64(*lhs / *rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::Remainder, Value::I64(lhs), Value::I64(rhs))
                if let Some(value) = lhs.checked_rem(*rhs) =>
            {
                *instruction = Instruction::Assign {
                    value: Value::I64(value),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::Remainder, Value::F64(lhs), Value::F64(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::F64(*lhs % *rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::Add, Value::I64(lhs), Value::I64(rhs))
                if let Some(value) = lhs.checked_add(*rhs) =>
            {
                *instruction = Instruction::Assign {
                    value: Value::I64(value),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::Add, Value::F64(lhs), Value::F64(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::F64(*lhs + *rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::Subtract, Value::I64(lhs), Value::I64(rhs))
                if let Some(value) = lhs.checked_sub(*rhs) =>
            {
                *instruction = Instruction::Assign {
                    value: Value::I64(value),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::Subtract, Value::F64(lhs), Value::F64(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::F64(*lhs - *rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::Less, Value::I64(lhs), Value::I64(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(*lhs < *rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::Less, Value::F64(lhs), Value::F64(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(*lhs < *rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::Greater, Value::I64(lhs), Value::I64(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(*lhs > *rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::Greater, Value::F64(lhs), Value::F64(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(*lhs > *rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::LessOrEqual, Value::I64(lhs), Value::I64(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(*lhs <= *rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::LessOrEqual, Value::F64(lhs), Value::F64(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(*lhs <= *rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::GreaterOrEqual, Value::I64(lhs), Value::I64(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(*lhs >= *rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::GreaterOrEqual, Value::F64(lhs), Value::F64(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(*lhs >= *rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::Equal, Value::I64(lhs), Value::I64(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(lhs == rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::Equal, Value::F64(lhs), Value::F64(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean((*lhs - *rhs).abs() < f64::EPSILON),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::Equal, Value::Boolean(lhs), Value::Boolean(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(lhs == rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::Equal, Value::Unit, Value::Unit) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(true),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::Equal, Value::Fn(lhs), Value::Fn(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(lhs == rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::Equal, Value::NativeFn(lhs), Value::NativeFn(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(lhs == rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::NotEqual, Value::I64(lhs), Value::I64(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(lhs != rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::NotEqual, Value::F64(lhs), Value::F64(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean((*lhs - *rhs).abs() >= f64::EPSILON),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::NotEqual, Value::Boolean(lhs), Value::Boolean(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(lhs != rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::NotEqual, Value::Unit, Value::Unit) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(false),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::NotEqual, Value::Fn(lhs), Value::Fn(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(lhs != rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::NotEqual, Value::NativeFn(lhs), Value::NativeFn(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(lhs != rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::And, Value::Boolean(lhs), Value::Boolean(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(*lhs && *rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (BinaryOp::Or, Value::Boolean(lhs), Value::Boolean(rhs)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(*lhs || *rhs),
                    to: Value::Address(Address {
                        block_index: BlockIndex(b),
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (
                BinaryOp::PathAccess
                | BinaryOp::Access
                | BinaryOp::Multiply
                | BinaryOp::Divide
                | BinaryOp::Remainder
                | BinaryOp::Add
                | BinaryOp::Subtract
                | BinaryOp::Less
                | BinaryOp::Greater
                | BinaryOp::LessOrEqual
                | BinaryOp::GreaterOrEqual
                | BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::And
                | BinaryOp::Or
                | BinaryOp::Assign,
                _,
                _,
            ) => {}
        }
    }
}
