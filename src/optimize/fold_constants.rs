use crate::{
    basic_blocks::{Address, BlockIndex, Instruction, Value},
    parse::{BinaryOp, UnaryOp},
    ssa::Ssa,
};

pub fn optimize(ssa: &mut Ssa) -> bool {
    let mut changed = false;

    ssa.for_live_blocks(|ssa, block_index| {
        if let Some(block) = ssa.get_block_mut(block_index) {
            for (i, instruction) in block.instructions_mut().iter_mut().enumerate() {
                match instruction {
                    Instruction::FnLabel { .. }
                    | Instruction::Panic
                    | Instruction::NoOp
                    | Instruction::Assign { .. }
                    | Instruction::Push(_)
                    | Instruction::Call { .. }
                    | Instruction::AccessAssign { .. } => {}
                    Instruction::Unary { .. } => {
                        fold_unary(block_index, i, instruction, &mut changed);
                    }
                    Instruction::Binary { .. } => {
                        fold_binary(block_index, i, instruction, &mut changed);
                    }
                    Instruction::GetTag {
                        of: value,
                        temporary: to,
                    } => {
                        if let Value::TaggedCompound { tag, .. } = value {
                            *instruction = Instruction::Assign {
                                value: Value::U16(*tag),
                                to: to.clone(),
                            };

                            changed = true;
                        }
                    }
                    Instruction::Access {
                        of: value,
                        index,
                        temporary: to,
                    } => {
                        if let Value::TaggedCompound { fields, .. } = value {
                            *instruction = Instruction::Assign {
                                value: fields[usize::from(*index)].clone(),
                                to: to.clone(),
                            };

                            changed = true;
                        } else if let Value::Compound(fields) = value {
                            *instruction = Instruction::Assign {
                                value: fields[usize::from(*index)].clone(),
                                to: to.clone(),
                            };

                            changed = true;
                        }
                    }
                }
            }
        }
    });

    changed
}

fn fold_unary(
    block_index: BlockIndex,
    i: usize,
    instruction: &mut Instruction,
    changed: &mut bool,
) {
    if let Instruction::Unary { op, operand, .. } = instruction {
        match (op, operand) {
            (UnaryOp::Not, Value::Boolean(value)) => {
                *instruction = Instruction::Assign {
                    value: Value::Boolean(!*value),
                    to: Value::Address(Address {
                        block_index,
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (UnaryOp::Negate, Value::I8(value)) => {
                *instruction = Instruction::Assign {
                    value: Value::I8(-*value),
                    to: Value::Address(Address {
                        block_index,
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (UnaryOp::Negate, Value::I16(value)) => {
                *instruction = Instruction::Assign {
                    value: Value::I16(-*value),
                    to: Value::Address(Address {
                        block_index,
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (UnaryOp::Negate, Value::I32(value)) => {
                *instruction = Instruction::Assign {
                    value: Value::I32(-*value),
                    to: Value::Address(Address {
                        block_index,
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
                        block_index,
                        offset: i,
                        version: 0,
                    }),
                };

                *changed = true;
            }
            (UnaryOp::Negate, Value::F32(value)) => {
                *instruction = Instruction::Assign {
                    value: Value::F32(-*value),
                    to: Value::Address(Address {
                        block_index,
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
                        block_index,
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

macro_rules! fold_binary_op {
    (
        $instruction:ident, $block_index:ident, $i:ident ;
        (* $lhs:ident $rhs:ident) => $result:ident
    ) => {{
        *$instruction = $crate::basic_blocks::Instruction::Assign {
            value: $crate::basic_blocks::Value::$result(*$lhs * *$rhs),
            to: $crate::basic_blocks::Value::Address($crate::basic_blocks::Address {
                block_index: $block_index,
                offset: $i,
                version: 0,
            }),
        };

        true
    }};

    (
        $instruction:ident, $block_index:ident, $i:ident ;
        (/ $lhs:ident $rhs:ident) => $result:ident
    ) => {{
        *$instruction = $crate::basic_blocks::Instruction::Assign {
            value: $crate::basic_blocks::Value::$result(*$lhs / *$rhs),
            to: $crate::basic_blocks::Value::Address($crate::basic_blocks::Address {
                block_index: $block_index,
                offset: $i,
                version: 0,
            }),
        };

        true
    }};

    (
        $instruction:ident, $block_index:ident, $i:ident ;
        (% $lhs:ident $rhs:ident) => $result:ident
    ) => {{
        *$instruction = $crate::basic_blocks::Instruction::Assign {
            value: $crate::basic_blocks::Value::$result(*$lhs % *$rhs),
            to: $crate::basic_blocks::Value::Address($crate::basic_blocks::Address {
                block_index: $block_index,
                offset: $i,
                version: 0,
            }),
        };

        true
    }};

    (
        $instruction:ident, $block_index:ident, $i:ident ;
        (+ $lhs:ident $rhs:ident) => $result:ident
    ) => {{
        *$instruction = $crate::basic_blocks::Instruction::Assign {
            value: $crate::basic_blocks::Value::$result(*$lhs + *$rhs),
            to: $crate::basic_blocks::Value::Address($crate::basic_blocks::Address {
                block_index: $block_index,
                offset: $i,
                version: 0,
            }),
        };

        true
    }};

    (
        $instruction:ident, $block_index:ident, $i:ident ;
        (- $lhs:ident $rhs:ident) => $result:ident
    ) => {{
        *$instruction = $crate::basic_blocks::Instruction::Assign {
            value: $crate::basic_blocks::Value::$result(*$lhs - *$rhs),
            to: $crate::basic_blocks::Value::Address($crate::basic_blocks::Address {
                block_index: $block_index,
                offset: $i,
                version: 0,
            }),
        };

        true
    }};

    (
        $instruction:ident, $block_index:ident, $i:ident ;
        (< $lhs:ident $rhs:ident)
    ) => {{
        *$instruction = $crate::basic_blocks::Instruction::Assign {
            value: $crate::basic_blocks::Value::Boolean($lhs < $rhs),
            to: $crate::basic_blocks::Value::Address($crate::basic_blocks::Address {
                block_index: $block_index,
                offset: $i,
                version: 0,
            }),
        };

        true
    }};

    (
        $instruction:ident, $block_index:ident, $i:ident ;
        (> $lhs:ident $rhs:ident)
    ) => {{
        *$instruction = $crate::basic_blocks::Instruction::Assign {
            value: $crate::basic_blocks::Value::Boolean($lhs > $rhs),
            to: $crate::basic_blocks::Value::Address($crate::basic_blocks::Address {
                block_index: $block_index,
                offset: $i,
                version: 0,
            }),
        };

        true
    }};

    (
        $instruction:ident, $block_index:ident, $i:ident ;
        (<= $lhs:ident $rhs:ident)
    ) => {{
        *$instruction = $crate::basic_blocks::Instruction::Assign {
            value: $crate::basic_blocks::Value::Boolean($lhs <= $rhs),
            to: $crate::basic_blocks::Value::Address($crate::basic_blocks::Address {
                block_index: $block_index,
                offset: $i,
                version: 0,
            }),
        };

        true
    }};

    (
        $instruction:ident, $block_index:ident, $i:ident ;
        (>= $lhs:ident $rhs:ident)
    ) => {{
        *$instruction = $crate::basic_blocks::Instruction::Assign {
            value: $crate::basic_blocks::Value::Boolean($lhs >= $rhs),
            to: $crate::basic_blocks::Value::Address($crate::basic_blocks::Address {
                block_index: $block_index,
                offset: $i,
                version: 0,
            }),
        };

        true
    }};

    (
        $instruction:ident, $block_index:ident, $i:ident ;
        (== $lhs:ident $rhs:ident)
    ) => {{
        *$instruction = $crate::basic_blocks::Instruction::Assign {
            value: $crate::basic_blocks::Value::Boolean($lhs == $rhs),
            to: $crate::basic_blocks::Value::Address($crate::basic_blocks::Address {
                block_index: $block_index,
                offset: $i,
                version: 0,
            }),
        };

        true
    }};

    (
        $instruction:ident, $block_index:ident, $i:ident ;
        (!= $lhs:ident $rhs:ident)
    ) => {{
        *$instruction = $crate::basic_blocks::Instruction::Assign {
            value: $crate::basic_blocks::Value::Boolean($lhs != $rhs),
            to: $crate::basic_blocks::Value::Address($crate::basic_blocks::Address {
                block_index: $block_index,
                offset: $i,
                version: 0,
            }),
        };

        true
    }};

    (
        $instruction:ident, $block_index:ident, $i:ident ;
        ($method:ident $lhs:ident $rhs:ident) => $result:ident
    ) => {{
        if let Some(value) = $lhs.$method(*$rhs) {
            *$instruction = $crate::basic_blocks::Instruction::Assign {
                value: $crate::basic_blocks::Value::$result(value),
                to: $crate::basic_blocks::Value::Address($crate::basic_blocks::Address {
                    block_index: $block_index,
                    offset: $i,
                    version: 0,
                }),
            };

            true
        } else {
            false
        }
    }};
}

#[allow(clippy::too_many_lines)]
fn fold_binary(
    block_index: BlockIndex,
    i: usize,
    instruction: &mut Instruction,
    changed: &mut bool,
) {
    if let Instruction::Binary { op, lhs, rhs, .. } = instruction {
        match (op, lhs, rhs) {
            (BinaryOp::Multiply, Value::U8(lhs), Value::U8(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_mul lhs rhs) => U8);
            }
            (BinaryOp::Multiply, Value::I8(lhs), Value::I8(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_mul lhs rhs) => I8);
            }
            (BinaryOp::Multiply, Value::U16(lhs), Value::U16(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_mul lhs rhs) => U16);
            }
            (BinaryOp::Multiply, Value::I16(lhs), Value::I16(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_mul lhs rhs) => I16);
            }
            (BinaryOp::Multiply, Value::U32(lhs), Value::U32(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_mul lhs rhs) => U32);
            }
            (BinaryOp::Multiply, Value::I32(lhs), Value::I32(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_mul lhs rhs) => I32);
            }
            (BinaryOp::Multiply, Value::U64(lhs), Value::U64(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_mul lhs rhs) => U64);
            }
            (BinaryOp::Multiply, Value::I64(lhs), Value::I64(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_mul lhs rhs) => I64);
            }
            (BinaryOp::Divide | BinaryOp::IntegerDivide, Value::U8(lhs), Value::U8(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_div lhs rhs) => U8);
            }
            (BinaryOp::Divide | BinaryOp::IntegerDivide, Value::I8(lhs), Value::I8(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_div lhs rhs) => I8);
            }
            (BinaryOp::Divide | BinaryOp::IntegerDivide, Value::U16(lhs), Value::U16(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_div lhs rhs) => U16);
            }
            (BinaryOp::Divide | BinaryOp::IntegerDivide, Value::I16(lhs), Value::I16(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_div lhs rhs) => I16);
            }
            (BinaryOp::Divide | BinaryOp::IntegerDivide, Value::U32(lhs), Value::U32(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_div lhs rhs) => U32);
            }
            (BinaryOp::Divide | BinaryOp::IntegerDivide, Value::I32(lhs), Value::I32(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_div lhs rhs) => I32);
            }
            (BinaryOp::Divide | BinaryOp::IntegerDivide, Value::U64(lhs), Value::U64(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_div lhs rhs) => U64);
            }
            (BinaryOp::Divide | BinaryOp::IntegerDivide, Value::I64(lhs), Value::I64(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_div lhs rhs) => I64);
            }
            (BinaryOp::Remainder, Value::U8(lhs), Value::U8(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_rem lhs rhs) => U8);
            }
            (BinaryOp::Remainder, Value::I8(lhs), Value::I8(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_rem lhs rhs) => I8);
            }
            (BinaryOp::Remainder, Value::U16(lhs), Value::U16(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_rem lhs rhs) => U16);
            }
            (BinaryOp::Remainder, Value::I16(lhs), Value::I16(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_rem lhs rhs) => I16);
            }
            (BinaryOp::Remainder, Value::U32(lhs), Value::U32(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_rem lhs rhs) => U32);
            }
            (BinaryOp::Remainder, Value::I32(lhs), Value::I32(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_rem lhs rhs) => I32);
            }
            (BinaryOp::Remainder, Value::U64(lhs), Value::U64(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_rem lhs rhs) => U64);
            }
            (BinaryOp::Remainder, Value::I64(lhs), Value::I64(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_rem lhs rhs) => I64);
            }
            (BinaryOp::Add, Value::U8(lhs), Value::U8(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_add lhs rhs) => U8);
            }
            (BinaryOp::Add, Value::I8(lhs), Value::I8(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_add lhs rhs) => I8);
            }
            (BinaryOp::Add, Value::U16(lhs), Value::U16(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_add lhs rhs) => U16);
            }
            (BinaryOp::Add, Value::I16(lhs), Value::I16(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_add lhs rhs) => I16);
            }
            (BinaryOp::Add, Value::U32(lhs), Value::U32(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_add lhs rhs) => U32);
            }
            (BinaryOp::Add, Value::I32(lhs), Value::I32(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_add lhs rhs) => I32);
            }
            (BinaryOp::Add, Value::U64(lhs), Value::U64(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_add lhs rhs) => U64);
            }
            (BinaryOp::Add, Value::I64(lhs), Value::I64(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_add lhs rhs) => I64);
            }
            (BinaryOp::Subtract, Value::U8(lhs), Value::U8(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_sub lhs rhs) => U8);
            }
            (BinaryOp::Subtract, Value::I8(lhs), Value::I8(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_sub lhs rhs) => I8);
            }
            (BinaryOp::Subtract, Value::U16(lhs), Value::U16(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_sub lhs rhs) => U16);
            }
            (BinaryOp::Subtract, Value::I16(lhs), Value::I16(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_sub lhs rhs) => I16);
            }
            (BinaryOp::Subtract, Value::U32(lhs), Value::U32(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_sub lhs rhs) => U32);
            }
            (BinaryOp::Subtract, Value::I32(lhs), Value::I32(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_sub lhs rhs) => I32);
            }
            (BinaryOp::Subtract, Value::U64(lhs), Value::U64(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_sub lhs rhs) => U64);
            }
            (BinaryOp::Subtract, Value::I64(lhs), Value::I64(rhs)) => {
                *changed |=
                    fold_binary_op!(instruction, block_index, i ; (checked_sub lhs rhs) => I64);
            }
            (BinaryOp::Less, Value::U8(lhs), Value::U8(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (< lhs rhs));
            }
            (BinaryOp::Less, Value::I8(lhs), Value::I8(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (< lhs rhs));
            }
            (BinaryOp::Less, Value::U16(lhs), Value::U16(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (< lhs rhs));
            }
            (BinaryOp::Less, Value::I16(lhs), Value::I16(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (< lhs rhs));
            }
            (BinaryOp::Less, Value::U32(lhs), Value::U32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (< lhs rhs));
            }
            (BinaryOp::Less, Value::I32(lhs), Value::I32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (< lhs rhs));
            }
            (BinaryOp::Less, Value::U64(lhs), Value::U64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (< lhs rhs));
            }
            (BinaryOp::Less, Value::I64(lhs), Value::I64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (< lhs rhs));
            }
            (BinaryOp::Less, Value::F32(lhs), Value::F32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (< lhs rhs));
            }
            (BinaryOp::Less, Value::F64(lhs), Value::F64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (< lhs rhs));
            }
            (BinaryOp::Greater, Value::U8(lhs), Value::U8(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (> lhs rhs));
            }
            (BinaryOp::Greater, Value::I8(lhs), Value::I8(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (> lhs rhs));
            }
            (BinaryOp::Greater, Value::U16(lhs), Value::U16(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (> lhs rhs));
            }
            (BinaryOp::Greater, Value::I16(lhs), Value::I16(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (> lhs rhs));
            }
            (BinaryOp::Greater, Value::U32(lhs), Value::U32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (> lhs rhs));
            }
            (BinaryOp::Greater, Value::I32(lhs), Value::I32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (> lhs rhs));
            }
            (BinaryOp::Greater, Value::U64(lhs), Value::U64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (> lhs rhs));
            }
            (BinaryOp::Greater, Value::I64(lhs), Value::I64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (> lhs rhs));
            }
            (BinaryOp::Greater, Value::F32(lhs), Value::F32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (> lhs rhs));
            }
            (BinaryOp::Greater, Value::F64(lhs), Value::F64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (> lhs rhs));
            }
            (BinaryOp::LessOrEqual, Value::U8(lhs), Value::U8(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (<= lhs rhs));
            }
            (BinaryOp::LessOrEqual, Value::I8(lhs), Value::I8(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (<= lhs rhs));
            }
            (BinaryOp::LessOrEqual, Value::U16(lhs), Value::U16(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (<= lhs rhs));
            }
            (BinaryOp::LessOrEqual, Value::I16(lhs), Value::I16(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (<= lhs rhs));
            }
            (BinaryOp::LessOrEqual, Value::U32(lhs), Value::U32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (<= lhs rhs));
            }
            (BinaryOp::LessOrEqual, Value::I32(lhs), Value::I32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (<= lhs rhs));
            }
            (BinaryOp::LessOrEqual, Value::U64(lhs), Value::U64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (<= lhs rhs));
            }
            (BinaryOp::LessOrEqual, Value::I64(lhs), Value::I64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (<= lhs rhs));
            }
            (BinaryOp::LessOrEqual, Value::F32(lhs), Value::F32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (<= lhs rhs));
            }
            (BinaryOp::LessOrEqual, Value::F64(lhs), Value::F64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (<= lhs rhs));
            }
            (BinaryOp::GreaterOrEqual, Value::U8(lhs), Value::U8(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (>= lhs rhs));
            }
            (BinaryOp::GreaterOrEqual, Value::I8(lhs), Value::I8(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (>= lhs rhs));
            }
            (BinaryOp::GreaterOrEqual, Value::U16(lhs), Value::U16(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (>= lhs rhs));
            }
            (BinaryOp::GreaterOrEqual, Value::I16(lhs), Value::I16(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (>= lhs rhs));
            }
            (BinaryOp::GreaterOrEqual, Value::U32(lhs), Value::U32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (>= lhs rhs));
            }
            (BinaryOp::GreaterOrEqual, Value::I32(lhs), Value::I32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (>= lhs rhs));
            }
            (BinaryOp::GreaterOrEqual, Value::U64(lhs), Value::U64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (>= lhs rhs));
            }
            (BinaryOp::GreaterOrEqual, Value::I64(lhs), Value::I64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (>= lhs rhs));
            }
            (BinaryOp::GreaterOrEqual, Value::F32(lhs), Value::F32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (>= lhs rhs));
            }
            (BinaryOp::GreaterOrEqual, Value::F64(lhs), Value::F64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (>= lhs rhs));
            }
            (BinaryOp::Multiply, Value::F32(lhs), Value::F32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (* lhs rhs) => F32);
            }
            (BinaryOp::Multiply, Value::F64(lhs), Value::F64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (* lhs rhs) => F64);
            }
            (BinaryOp::Divide, Value::F32(lhs), Value::F32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (/ lhs rhs) => F32);
            }
            (BinaryOp::Divide, Value::F64(lhs), Value::F64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (/ lhs rhs) => F64);
            }
            (BinaryOp::Remainder, Value::F32(lhs), Value::F32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (% lhs rhs) => F32);
            }
            (BinaryOp::Remainder, Value::F64(lhs), Value::F64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (% lhs rhs) => F64);
            }
            (BinaryOp::Add, Value::F32(lhs), Value::F32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (+ lhs rhs) => F32);
            }
            (BinaryOp::Add, Value::F64(lhs), Value::F64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (+ lhs rhs) => F64);
            }
            (BinaryOp::Subtract, Value::F32(lhs), Value::F32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (- lhs rhs) => F32);
            }
            (BinaryOp::Subtract, Value::F64(lhs), Value::F64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (- lhs rhs) => F64);
            }
            (BinaryOp::Equal, Value::U8(lhs), Value::U8(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (== lhs rhs));
            }
            (BinaryOp::Equal, Value::I8(lhs), Value::I8(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (== lhs rhs));
            }
            (BinaryOp::Equal, Value::U16(lhs), Value::U16(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (== lhs rhs));
            }
            (BinaryOp::Equal, Value::I16(lhs), Value::I16(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (== lhs rhs));
            }
            (BinaryOp::Equal, Value::U32(lhs), Value::U32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (== lhs rhs));
            }
            (BinaryOp::Equal, Value::I32(lhs), Value::I32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (== lhs rhs));
            }
            (BinaryOp::Equal, Value::U64(lhs), Value::U64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (== lhs rhs));
            }
            (BinaryOp::Equal, Value::I64(lhs), Value::I64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (== lhs rhs));
            }
            (BinaryOp::NotEqual, Value::U8(lhs), Value::U8(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (!= lhs rhs));
            }
            (BinaryOp::NotEqual, Value::I8(lhs), Value::I8(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (!= lhs rhs));
            }
            (BinaryOp::NotEqual, Value::U16(lhs), Value::U16(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (!= lhs rhs));
            }
            (BinaryOp::NotEqual, Value::I16(lhs), Value::I16(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (!= lhs rhs));
            }
            (BinaryOp::NotEqual, Value::U32(lhs), Value::U32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (!= lhs rhs));
            }
            (BinaryOp::NotEqual, Value::I32(lhs), Value::I32(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (!= lhs rhs));
            }
            (BinaryOp::NotEqual, Value::U64(lhs), Value::U64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (!= lhs rhs));
            }
            (BinaryOp::NotEqual, Value::I64(lhs), Value::I64(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (!= lhs rhs));
            }
            (BinaryOp::Equal, Value::F32(lhs), Value::F32(rhs)) => {
                *changed |= {
                    *instruction = Instruction::Assign {
                        value: Value::Boolean((*lhs - *rhs).abs() < f32::EPSILON),
                        to: Value::Address(Address {
                            block_index,
                            offset: i,
                            version: 0,
                        }),
                    };

                    true
                };
            }
            (BinaryOp::Equal, Value::F64(lhs), Value::F64(rhs)) => {
                *changed |= {
                    *instruction = Instruction::Assign {
                        value: Value::Boolean((*lhs - *rhs).abs() < f64::EPSILON),
                        to: Value::Address(Address {
                            block_index,
                            offset: i,
                            version: 0,
                        }),
                    };

                    true
                };
            }
            (BinaryOp::NotEqual, Value::F32(lhs), Value::F32(rhs)) => {
                *changed |= {
                    *instruction = Instruction::Assign {
                        value: Value::Boolean((*lhs - *rhs).abs() >= f32::EPSILON),
                        to: Value::Address(Address {
                            block_index,
                            offset: i,
                            version: 0,
                        }),
                    };

                    true
                };
            }
            (BinaryOp::NotEqual, Value::F64(lhs), Value::F64(rhs)) => {
                *changed |= {
                    *instruction = Instruction::Assign {
                        value: Value::Boolean((*lhs - *rhs).abs() >= f64::EPSILON),
                        to: Value::Address(Address {
                            block_index,
                            offset: i,
                            version: 0,
                        }),
                    };

                    true
                };
            }
            (BinaryOp::Equal, Value::Boolean(lhs), Value::Boolean(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (== lhs rhs));
            }
            (BinaryOp::NotEqual, Value::Boolean(lhs), Value::Boolean(rhs)) => {
                *changed |= fold_binary_op!(instruction, block_index, i ; (!= lhs rhs));
            }
            (BinaryOp::Equal, Value::Unit, Value::Unit) => {
                *changed |= {
                    *instruction = Instruction::Assign {
                        value: Value::Boolean(true),
                        to: Value::Address(Address {
                            block_index,
                            offset: i,
                            version: 0,
                        }),
                    };

                    true
                };
            }
            (BinaryOp::NotEqual, Value::Unit, Value::Unit) => {
                *changed |= {
                    *instruction = Instruction::Assign {
                        value: Value::Boolean(false),
                        to: Value::Address(Address {
                            block_index,
                            offset: i,
                            version: 0,
                        }),
                    };

                    true
                };
            }
            (BinaryOp::And, Value::Boolean(lhs), Value::Boolean(rhs)) => {
                *changed |= {
                    *instruction = Instruction::Assign {
                        value: Value::Boolean(*lhs && *rhs),
                        to: Value::Address(Address {
                            block_index,
                            offset: i,
                            version: 0,
                        }),
                    };

                    true
                };
            }
            (BinaryOp::Or, Value::Boolean(lhs), Value::Boolean(rhs)) => {
                *changed |= {
                    *instruction = Instruction::Assign {
                        value: Value::Boolean(*lhs || *rhs),
                        to: Value::Address(Address {
                            block_index,
                            offset: i,
                            version: 0,
                        }),
                    };

                    true
                };
            }
            (
                BinaryOp::PathAccess
                | BinaryOp::Access
                | BinaryOp::Multiply
                | BinaryOp::IntegerDivide
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
