use crate::{
    Compiled,
    basic_blocks::BlockIndex,
    low_ir::{
        BinaryOp, Instruction as IrInstruction, Ir, Location, NativeFn, Register, UnaryOp,
        Value as IrValue, ValueOrLocation,
    },
};

use std::collections::HashMap;

/// # Panics
/// Panics if `usize` doesn't fit in a u64
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn compile(compiled: &Compiled<'_, Ir<IrValue>>) -> Vec<u8> {
    let ssa = compiled.result();

    let mut bytecode = vec![];

    bytecode.extend_from_slice(
        u64::try_from(compiled.result().main_max_register_count())
            .expect("128-bit usize not allowed!  sorry!")
            .to_le_bytes()
            .as_slice(),
    );

    let mut block_addresses = vec![0usize; ssa.blocks().len()];
    let mut address_to_label = HashMap::new();

    for (b, block) in ssa.blocks().iter().enumerate() {
        block_addresses[b] = bytecode.len();

        address_to_label.insert(b, b);

        bytecode.push(u8::from(OpCode::Label));

        for instruction in block.instructions() {
            bytecode.append(&mut instruction.to_bytes(compiled));
        }
    }

    loop {
        if !backpatch(
            &mut bytecode,
            block_addresses.as_slice(),
            &mut address_to_label,
        ) {
            break;
        }

        find_labels(
            &mut bytecode,
            &mut block_addresses,
            &mut address_to_label,
            false,
        );
    }

    find_labels(
        &mut bytecode,
        &mut block_addresses,
        &mut address_to_label,
        true,
    );

    bytecode
}

macro_rules! backpatch_jump {
    (
        $bytecode:ident, $ip:ident, $block_addresses:ident, $address_to_label:ident ;
        $int_type:ident $(,)?
    ) => {{
        const SIZE: usize = std::mem::size_of::<$int_type>();

        $ip += 1;

        let current_destination = $int_type::from_le_bytes(
            *$bytecode[$ip..($ip + SIZE)]
                .as_array::<SIZE>()
                .expect("128-bit jump destination not allowed!  sorry!"),
        );

        let current_destination = usize::try_from(current_destination).unwrap_or_else(|_| {
            panic!("jump destination was not a valid usize {current_destination}");
        });

        let label_index = $address_to_label
            .get(&current_destination)
            .copied()
            .unwrap_or_else(|| {
                println!("{:?}", $address_to_label);
                panic!("destination was not a label {current_destination}");
            });

        let destination = $block_addresses
            .get(label_index)
            .copied()
            .unwrap_or_else(|| {
                panic!("block address not found for block {current_destination}");
            });

        if let Ok(destination) = u8::try_from(destination).map(u8::to_le_bytes) {
            $bytecode[$ip - 1] = u8::from($crate::targets::vm::OpCode::JumpU8);

            $bytecode.splice($ip..($ip + SIZE), destination);

            $ip += 1;
        } else if let Ok(destination) = u16::try_from(destination).map(u16::to_le_bytes) {
            $bytecode[$ip - 1] = u8::from($crate::targets::vm::OpCode::JumpU16);

            $bytecode.splice($ip..($ip + SIZE), destination);

            $ip += 2;
        } else if let Ok(destination) = u32::try_from(destination).map(u32::to_le_bytes) {
            $bytecode[$ip - 1] = u8::from($crate::targets::vm::OpCode::JumpU32);

            $bytecode.splice($ip..($ip + SIZE), destination);

            $ip += 4;
        } else {
            $bytecode[$ip - 1] = u8::from($crate::targets::vm::OpCode::JumpU64);

            $bytecode.splice(
                $ip..($ip + SIZE),
                u64::try_from(destination)
                    .expect("128-bit usize not allowed!  sorry!")
                    .to_le_bytes(),
            );

            $ip += 8;
        }

        destination != current_destination
    }};
}

macro_rules! backpatch_branch {
    (
        $bytecode:ident, $ip:ident, $block_addresses:ident, $address_to_label:ident ;
        $int_type:ident, $changed:ident $(,)?
    ) => {{
        const SIZE: usize = std::mem::size_of::<$int_type>();

        $ip += 1;

        let current_when_true = $int_type::from_le_bytes(
            *$bytecode[$ip..($ip + SIZE)]
                .as_array::<SIZE>()
                .expect("128-bit jump destination not allowed!  sorry!"),
        );

        let current_when_true = usize::try_from(current_when_true).unwrap_or_else(|_| {
            panic!("jump destination was not a valid usize {current_when_true}");
        });

        let label_index = $address_to_label
            .get(&current_when_true)
            .copied()
            .unwrap_or_else(|| {
                println!("{:?}", $address_to_label);
                panic!("destination was not a label {current_when_true}");
            });

        let when_true = $block_addresses
            .get(label_index)
            .copied()
            .unwrap_or_else(|| {
                panic!("block address not found for block {current_when_true}");
            });

        let current_otherwise = $int_type::from_le_bytes(
            *$bytecode[($ip + SIZE)..($ip + SIZE * 2)]
                .as_array::<SIZE>()
                .expect("128-bit jump destination not allowed!  sorry!"),
        );

        let current_otherwise = usize::try_from(current_otherwise).unwrap_or_else(|_| {
            panic!("jump otherwise was not a valid usize {current_otherwise}");
        });

        let label_index = $address_to_label
            .get(&usize::from(current_otherwise))
            .copied()
            .unwrap_or_else(|| {
                println!("{:?}", $address_to_label);
                panic!("destination was not a label {current_otherwise}");
            });

        let otherwise = $block_addresses
            .get(label_index)
            .copied()
            .unwrap_or_else(|| {
                panic!("block address not found for block {current_otherwise}");
            });

        if let Ok(when_true) = u8::try_from(when_true).map(u8::to_le_bytes)
            && let Ok(otherwise) = u8::try_from(otherwise).map(u8::to_le_bytes)
        {
            $bytecode[$ip - 1] = u8::from($crate::targets::vm::OpCode::BranchU8);

            $bytecode.splice(($ip + SIZE)..($ip + SIZE * 2), otherwise);

            $bytecode.splice($ip..($ip + SIZE), when_true);

            $ip += 2;
        } else if let Ok(when_true) = u16::try_from(when_true).map(u16::to_le_bytes)
            && let Ok(otherwise) = u16::try_from(otherwise).map(u16::to_le_bytes)
        {
            $bytecode[$ip - 1] = u8::from($crate::targets::vm::OpCode::BranchU16);

            $bytecode.splice(($ip + SIZE)..($ip + SIZE * 2), otherwise);

            $bytecode.splice($ip..($ip + SIZE), when_true);

            $ip += 4;
        } else if let Ok(when_true) = u32::try_from(when_true).map(u32::to_le_bytes)
            && let Ok(otherwise) = u32::try_from(otherwise).map(u32::to_le_bytes)
        {
            $bytecode[$ip - 1] = u8::from($crate::targets::vm::OpCode::BranchU32);

            $bytecode.splice(($ip + SIZE)..($ip + SIZE * 2), otherwise);

            $bytecode.splice($ip..($ip + SIZE), when_true);

            $ip += 8;
        } else {
            $bytecode[$ip - 1] = u8::from($crate::targets::vm::OpCode::BranchU64);

            $bytecode.splice(
                ($ip + SIZE)..($ip + SIZE * 2),
                u64::try_from(otherwise)
                    .expect("128-bit usize not allowed!  sorry!")
                    .to_le_bytes(),
            );

            $bytecode.splice(
                $ip..($ip + SIZE),
                u64::try_from(when_true)
                    .expect("128-bit usize not allowed!  sorry!")
                    .to_le_bytes(),
            );

            $ip += 16;
        }

        $crate::targets::vm::backpatch_value(
            $bytecode,
            &mut $ip,
            $block_addresses,
            $address_to_label,
            &mut $changed,
        );

        when_true != current_when_true || otherwise != current_otherwise
    }};
}

#[allow(clippy::too_many_lines)]
fn backpatch(
    bytecode: &mut Vec<u8>,
    block_addresses: &[usize],
    address_to_label: &mut HashMap<usize, usize>,
) -> bool {
    let mut changed = false;

    let mut ip = 8;

    while let Some(byte) = bytecode.get(ip).copied() {
        match OpCode::try_from(byte) {
            Err(error) => {
                panic!("unknown op {error:#X}");
            }
            Ok(OpCode::Label) => {
                ip += 1;
            }
            Ok(OpCode::Unary | OpCode::CallU8 | OpCode::AccessU8) => {
                ip += 2;

                move_through_location(bytecode, &mut ip);

                backpatch_value(
                    bytecode,
                    &mut ip,
                    block_addresses,
                    address_to_label,
                    &mut changed,
                );
            }
            Ok(OpCode::Binary) => {
                ip += 2;

                move_through_location(bytecode, &mut ip);

                backpatch_value(
                    bytecode,
                    &mut ip,
                    block_addresses,
                    address_to_label,
                    &mut changed,
                );

                backpatch_value(
                    bytecode,
                    &mut ip,
                    block_addresses,
                    address_to_label,
                    &mut changed,
                );
            }
            Ok(OpCode::Assign | OpCode::GetTag) => {
                ip += 1;

                move_through_location(bytecode, &mut ip);

                backpatch_value(
                    bytecode,
                    &mut ip,
                    block_addresses,
                    address_to_label,
                    &mut changed,
                );
            }
            Ok(OpCode::PopNU8) => {
                ip += 2;
            }
            Ok(OpCode::PopNU16) => {
                ip += 3;
            }
            Ok(OpCode::PopNU32) => {
                ip += 5;
            }
            Ok(OpCode::PopNU64) => {
                ip += 9;
            }
            Ok(OpCode::Push | OpCode::Return) => {
                ip += 1;

                backpatch_value(
                    bytecode,
                    &mut ip,
                    block_addresses,
                    address_to_label,
                    &mut changed,
                );
            }
            Ok(OpCode::CallU16 | OpCode::AccessU16) => {
                ip += 3;

                move_through_location(bytecode, &mut ip);

                backpatch_value(
                    bytecode,
                    &mut ip,
                    block_addresses,
                    address_to_label,
                    &mut changed,
                );
            }
            Ok(OpCode::CallU32 | OpCode::AccessU32) => {
                ip += 5;

                move_through_location(bytecode, &mut ip);

                backpatch_value(
                    bytecode,
                    &mut ip,
                    block_addresses,
                    address_to_label,
                    &mut changed,
                );
            }
            Ok(OpCode::CallU64 | OpCode::AccessU64) => {
                ip += 9;

                move_through_location(bytecode, &mut ip);

                backpatch_value(
                    bytecode,
                    &mut ip,
                    block_addresses,
                    address_to_label,
                    &mut changed,
                );
            }
            Ok(OpCode::AccessAssignU8) => {
                ip += 2;

                backpatch_value(
                    bytecode,
                    &mut ip,
                    block_addresses,
                    address_to_label,
                    &mut changed,
                );

                backpatch_value(
                    bytecode,
                    &mut ip,
                    block_addresses,
                    address_to_label,
                    &mut changed,
                );
            }
            Ok(OpCode::AccessAssignU16) => {
                ip += 3;

                backpatch_value(
                    bytecode,
                    &mut ip,
                    block_addresses,
                    address_to_label,
                    &mut changed,
                );

                backpatch_value(
                    bytecode,
                    &mut ip,
                    block_addresses,
                    address_to_label,
                    &mut changed,
                );
            }
            Ok(OpCode::AccessAssignU32) => {
                ip += 5;

                backpatch_value(
                    bytecode,
                    &mut ip,
                    block_addresses,
                    address_to_label,
                    &mut changed,
                );

                backpatch_value(
                    bytecode,
                    &mut ip,
                    block_addresses,
                    address_to_label,
                    &mut changed,
                );
            }
            Ok(OpCode::AccessAssignU64) => {
                ip += 9;

                backpatch_value(
                    bytecode,
                    &mut ip,
                    block_addresses,
                    address_to_label,
                    &mut changed,
                );

                backpatch_value(
                    bytecode,
                    &mut ip,
                    block_addresses,
                    address_to_label,
                    &mut changed,
                );
            }
            Ok(OpCode::JumpU8) => {
                changed |= backpatch_jump!(
                    bytecode, ip, block_addresses, address_to_label ;
                    u8,
                );
            }
            Ok(OpCode::JumpU16) => {
                changed |= backpatch_jump!(
                    bytecode, ip, block_addresses, address_to_label ;
                    u16,
                );
            }
            Ok(OpCode::JumpU32) => {
                changed |= backpatch_jump!(
                    bytecode, ip, block_addresses, address_to_label ;
                    u32,
                );
            }
            Ok(OpCode::JumpU64) => {
                changed |= backpatch_jump!(
                    bytecode, ip, block_addresses, address_to_label ;
                    u64,
                );
            }
            Ok(OpCode::BranchU8) => {
                changed |= backpatch_branch!(
                    bytecode, ip, block_addresses, address_to_label ;
                    u8, changed,
                );
            }
            Ok(OpCode::BranchU16) => {
                changed |= backpatch_branch!(
                    bytecode, ip, block_addresses, address_to_label ;
                    u16, changed,
                );
            }
            Ok(OpCode::BranchU32) => {
                changed |= backpatch_branch!(
                    bytecode, ip, block_addresses, address_to_label ;
                    u32, changed,
                );
            }
            Ok(OpCode::BranchU64) => {
                changed |= backpatch_branch!(
                    bytecode, ip, block_addresses, address_to_label ;
                    u64, changed,
                );
            }
        }
    }

    changed
}

fn move_through_location(bytecode: &[u8], ip: &mut usize) {
    match ValueAt::try_from(bytecode[*ip]) {
        Ok(ValueAt::RegisterU8) => {
            *ip += 2;
        }
        Ok(ValueAt::RegisterU16) => {
            *ip += 3;
        }
        Ok(ValueAt::RegisterU32) => {
            *ip += 5;
        }
        Ok(ValueAt::RegisterU64) => {
            *ip += 9;
        }
        Ok(ValueAt::Value) => {
            panic!("tried to move through a value as a location");
        }
        Err(error) => {
            panic!("tried to move through an invalid location {error:#X}");
        }
    }
}

macro_rules! backpatch_fn {
    (
        $bytecode:ident, $ip:ident, $block_addresses:ident, $address_to_label:ident ;
        $int_type:ident $(,)?
    ) => {{
        const SIZE: usize = std::mem::size_of::<$int_type>();

        *$ip += 1;

        let current_destination = $int_type::from_le_bytes(
            *$bytecode[(*$ip)..(*$ip + SIZE)]
                .as_array::<SIZE>()
                .expect("128-bit jump destination not allowed!  sorry!"),
        );

        let current_destination = usize::try_from(current_destination).unwrap_or_else(|_| {
            panic!("jump destination was not a valid usize {current_destination}");
        });

        let label_index = $address_to_label
            .get(&current_destination)
            .copied()
            .unwrap_or_else(|| {
                println!("{:?}", $address_to_label);
                panic!("destination was not a label {current_destination}");
            });

        let destination = $block_addresses
            .get(label_index)
            .copied()
            .unwrap_or_else(|| {
                panic!("block address not found for block {current_destination}");
            });

        if let Ok(destination) = u8::try_from(destination).map(u8::to_le_bytes) {
            $bytecode[*$ip - 1] = u8::from($crate::targets::vm::TypeId::FnU8);

            $bytecode.splice((*$ip)..(*$ip + SIZE), destination);

            *$ip += 1;
        } else if let Ok(destination) = u16::try_from(destination).map(u16::to_le_bytes) {
            $bytecode[*$ip - 1] = u8::from($crate::targets::vm::TypeId::FnU16);

            $bytecode.splice((*$ip)..(*$ip + SIZE), destination);

            *$ip += 2;
        } else if let Ok(destination) = u32::try_from(destination).map(u32::to_le_bytes) {
            $bytecode[*$ip - 1] = u8::from($crate::targets::vm::TypeId::FnU32);

            $bytecode.splice((*$ip)..(*$ip + SIZE), destination);

            *$ip += 4;
        } else {
            $bytecode[*$ip - 1] = u8::from($crate::targets::vm::TypeId::FnU64);

            $bytecode.splice(
                (*$ip)..(*$ip + SIZE),
                u64::try_from(destination)
                    .expect("128-bit usize not allowed!  sorry!")
                    .to_le_bytes(),
            );

            *$ip += 8;
        }

        *$ip += 4;

        destination != current_destination
    }};
}

#[allow(clippy::too_many_lines)]
fn backpatch_value(
    bytecode: &mut Vec<u8>,
    ip: &mut usize,
    block_addresses: &[usize],
    address_to_label: &mut HashMap<usize, usize>,
    changed: &mut bool,
) {
    match ValueAt::try_from(bytecode[*ip]) {
        Ok(ValueAt::Value) => {
            *ip += 1;

            match TypeId::try_from(bytecode[*ip]) {
                Err(error) => {
                    panic!("tried to backpatch unknown value type {error:#X}");
                }
                Ok(TypeId::Unit) => {
                    *ip += 1;
                }
                Ok(TypeId::U8 | TypeId::I8 | TypeId::Boolean) => {
                    *ip += 2;
                }
                Ok(TypeId::U16 | TypeId::I16 | TypeId::NativeFn) => {
                    *ip += 3;
                }
                Ok(TypeId::U32 | TypeId::I32) => {
                    *ip += 5;
                }
                Ok(TypeId::U64 | TypeId::I64 | TypeId::F64) => {
                    *ip += 9;
                }
                Ok(TypeId::String) => {
                    *ip += 1;

                    let length = bytecode
                        .get((*ip)..(*ip + 8))
                        .and_then(|bytes| bytes.as_array::<8>())
                        .map(|bytes| u64::from_le_bytes(*bytes))
                        .and_then(|length| usize::try_from(length).ok())
                        .expect("string length was not a valid usize");

                    *ip += 8 + length;
                }
                Ok(TypeId::FnU8) => {
                    *changed |= backpatch_fn!(
                        bytecode, ip, block_addresses, address_to_label ;
                        u8,
                    );
                }
                Ok(TypeId::FnU16) => {
                    *changed |= backpatch_fn!(
                        bytecode, ip, block_addresses, address_to_label ;
                        u16,
                    );
                }
                Ok(TypeId::FnU32) => {
                    *changed |= backpatch_fn!(
                        bytecode, ip, block_addresses, address_to_label ;
                        u32,
                    );
                }
                Ok(TypeId::FnU64) => {
                    *changed |= backpatch_fn!(
                        bytecode, ip, block_addresses, address_to_label ;
                        u64,
                    );
                }
                Ok(TypeId::Compound) => {
                    *ip += 1;

                    let field_count = bytecode
                        .get((*ip)..(*ip + 8))
                        .and_then(|bytes| bytes.as_array::<8>())
                        .map(|bytes| u64::from_le_bytes(*bytes))
                        .and_then(|field_count| usize::try_from(field_count).ok())
                        .expect("tagged compound field count was not a valid usize");

                    *ip += 8;

                    for _ in 0..field_count {
                        backpatch_value(bytecode, ip, block_addresses, address_to_label, changed);
                    }
                }
                Ok(TypeId::TaggedCompound) => {
                    *ip += 3;

                    let field_count = bytecode
                        .get((*ip)..(*ip + 8))
                        .and_then(|bytes| bytes.as_array::<8>())
                        .map(|bytes| u64::from_le_bytes(*bytes))
                        .and_then(|field_count| usize::try_from(field_count).ok())
                        .expect("tagged compound field count was not a valid usize");

                    *ip += 8;

                    for _ in 0..field_count {
                        backpatch_value(bytecode, ip, block_addresses, address_to_label, changed);
                    }
                }
            }
        }
        Ok(
            ValueAt::RegisterU8
            | ValueAt::RegisterU16
            | ValueAt::RegisterU32
            | ValueAt::RegisterU64,
        ) => {
            move_through_location(bytecode, ip);
        }
        Err(error) => {
            panic!("tried to backpatch an invalid value {error:#X}");
        }
    }
}

#[allow(clippy::too_many_lines)]
fn find_labels(
    bytecode: &mut Vec<u8>,
    block_addresses: &mut [usize],
    address_to_label: &mut HashMap<usize, usize>,
    remove_labels: bool,
) {
    address_to_label.clear();

    let mut label_index = 0;
    let mut ip = 8;

    while let Some(byte) = bytecode.get(ip).copied() {
        match OpCode::try_from(byte) {
            Err(error) => {
                panic!("unknown op {error:#X}");
            }
            Ok(OpCode::Label) => {
                if remove_labels {
                    bytecode.splice(ip..=ip, []);
                } else {
                    address_to_label.insert(block_addresses[label_index], label_index);

                    block_addresses[label_index] = ip - label_index;

                    label_index += 1;

                    ip += 1;
                }
            }
            Ok(OpCode::Unary | OpCode::CallU8 | OpCode::AccessU8) => {
                ip += 2;

                move_through_location(bytecode, &mut ip);

                move_through_value(bytecode, &mut ip);
            }
            Ok(OpCode::Binary) => {
                ip += 2;

                move_through_location(bytecode, &mut ip);

                move_through_value(bytecode, &mut ip);

                move_through_value(bytecode, &mut ip);
            }
            Ok(OpCode::Assign | OpCode::GetTag) => {
                ip += 1;

                move_through_location(bytecode, &mut ip);

                move_through_value(bytecode, &mut ip);
            }
            Ok(OpCode::PopNU8 | OpCode::JumpU8) => {
                ip += 2;
            }
            Ok(OpCode::PopNU16 | OpCode::JumpU16) => {
                ip += 3;
            }
            Ok(OpCode::PopNU32 | OpCode::JumpU32) => {
                ip += 5;
            }
            Ok(OpCode::PopNU64 | OpCode::JumpU64) => {
                ip += 9;
            }
            Ok(OpCode::Push | OpCode::Return) => {
                ip += 1;

                move_through_value(bytecode, &mut ip);
            }
            Ok(OpCode::CallU16 | OpCode::AccessU16) => {
                ip += 3;

                move_through_location(bytecode, &mut ip);

                move_through_value(bytecode, &mut ip);
            }
            Ok(OpCode::CallU32 | OpCode::AccessU32) => {
                ip += 5;

                move_through_location(bytecode, &mut ip);

                move_through_value(bytecode, &mut ip);
            }
            Ok(OpCode::CallU64 | OpCode::AccessU64) => {
                ip += 9;

                move_through_location(bytecode, &mut ip);

                move_through_value(bytecode, &mut ip);
            }
            Ok(OpCode::AccessAssignU8) => {
                ip += 2;

                move_through_value(bytecode, &mut ip);

                move_through_value(bytecode, &mut ip);
            }
            Ok(OpCode::AccessAssignU16) => {
                ip += 3;

                move_through_value(bytecode, &mut ip);

                move_through_value(bytecode, &mut ip);
            }
            Ok(OpCode::AccessAssignU32) => {
                ip += 5;

                move_through_value(bytecode, &mut ip);

                move_through_value(bytecode, &mut ip);
            }
            Ok(OpCode::AccessAssignU64) => {
                ip += 9;

                move_through_value(bytecode, &mut ip);

                move_through_value(bytecode, &mut ip);
            }
            Ok(OpCode::BranchU8) => {
                ip += 3;

                move_through_value(bytecode, &mut ip);
            }
            Ok(OpCode::BranchU16) => {
                ip += 5;

                move_through_value(bytecode, &mut ip);
            }
            Ok(OpCode::BranchU32) => {
                ip += 9;

                move_through_value(bytecode, &mut ip);
            }
            Ok(OpCode::BranchU64) => {
                ip += 17;

                move_through_value(bytecode, &mut ip);
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
fn move_through_value(bytecode: &mut Vec<u8>, ip: &mut usize) {
    match ValueAt::try_from(bytecode[*ip]) {
        Ok(ValueAt::Value) => {
            *ip += 1;

            match TypeId::try_from(bytecode[*ip]) {
                Err(error) => {
                    panic!("tried to move through unknown value type {error:#X}");
                }
                Ok(TypeId::Unit) => {
                    *ip += 1;
                }
                Ok(TypeId::U8 | TypeId::I8 | TypeId::Boolean) => {
                    *ip += 2;
                }
                Ok(TypeId::U16 | TypeId::I16 | TypeId::NativeFn) => {
                    *ip += 3;
                }
                Ok(TypeId::U32 | TypeId::I32) => {
                    *ip += 5;
                }
                Ok(TypeId::U64 | TypeId::I64 | TypeId::F64 | TypeId::FnU32) => {
                    *ip += 9;
                }
                Ok(TypeId::String) => {
                    *ip += 1;

                    let length = bytecode
                        .get((*ip)..(*ip + 8))
                        .and_then(|bytes| bytes.as_array::<8>())
                        .map(|bytes| u64::from_le_bytes(*bytes))
                        .and_then(|length| usize::try_from(length).ok())
                        .expect("string length was not a valid usize");

                    *ip += 8 + length;
                }
                Ok(TypeId::FnU8) => {
                    *ip += 6;
                }
                Ok(TypeId::FnU16) => {
                    *ip += 7;
                }
                Ok(TypeId::FnU64) => {
                    *ip += 13;
                }
                Ok(TypeId::Compound) => {
                    *ip += 1;

                    let field_count = bytecode
                        .get((*ip)..(*ip + 8))
                        .and_then(|bytes| bytes.as_array::<8>())
                        .map(|bytes| u64::from_le_bytes(*bytes))
                        .and_then(|field_count| usize::try_from(field_count).ok())
                        .expect("tagged compound field count was not a valid usize");

                    *ip += 8;

                    for _ in 0..field_count {
                        move_through_value(bytecode, ip);
                    }
                }
                Ok(TypeId::TaggedCompound) => {
                    *ip += 3;

                    let field_count = bytecode
                        .get((*ip)..(*ip + 8))
                        .and_then(|bytes| bytes.as_array::<8>())
                        .map(|bytes| u64::from_le_bytes(*bytes))
                        .and_then(|field_count| usize::try_from(field_count).ok())
                        .expect("tagged compound field count was not a valid usize");

                    *ip += 8;

                    for _ in 0..field_count {
                        move_through_value(bytecode, ip);
                    }
                }
            }
        }
        Ok(
            ValueAt::RegisterU8
            | ValueAt::RegisterU16
            | ValueAt::RegisterU32
            | ValueAt::RegisterU64,
        ) => {
            move_through_location(bytecode, ip);
        }
        Err(error) => {
            panic!("tried to move_through an invalid value {error:#X}");
        }
    }
}

crate::int_enum! {
    pub OpCode as u8 ;
    Label => 0x10,
    Unary => 0x20,
    Binary => 0x40,
    Assign => 0x60,
    Push => 0x80,
    PopNU8 => 0x90,
    PopNU16 => 0x91,
    PopNU32 => 0x92,
    PopNU64 => 0x93,
    CallU8 => 0xA0,
    CallU16 => 0xA1,
    CallU32 => 0xA2,
    CallU64 => 0xA3,
    AccessU8 => 0xB0,
    AccessAssignU8 => 0xB1,
    AccessU16 => 0xB2,
    AccessAssignU16 => 0xB3,
    AccessU32 => 0xB4,
    AccessAssignU32 => 0xB5,
    AccessU64 => 0xB6,
    AccessAssignU64 => 0xB7,
    GetTag => 0xB8,
    JumpU8 => 0xC0,
    BranchU8 => 0xC1,
    JumpU16 => 0xC2,
    BranchU16 => 0xC3,
    JumpU32 => 0xC4,
    BranchU32 => 0xC5,
    JumpU64 => 0xC6,
    BranchU64 => 0xC7,
    Return => 0xC9,
}

crate::int_enum! {
    pub ValueAt as u8 ;
    Value => 0x00,
    RegisterU8 => 0x01,
    RegisterU16 => 0x02,
    RegisterU32 => 0x03,
    RegisterU64 => 0x04,
}

crate::int_enum! {
    pub TypeId as u8 ;
    I64 => 0x00,
    F64 => 0x01,
    Boolean => 0x02,
    Unit => 0x03,
    FnU8 => 0x04,
    FnU16 => 0x05,
    FnU32 => 0x06,
    FnU64 => 0x07,
    NativeFn => 0x08,
    Compound => 0x09,
    TaggedCompound => 0x0A,
    U8 => 0x0B,
    I8 => 0x0C,
    U16 => 0x0D,
    I16 => 0x0E,
    U32 => 0x0F,
    I32 => 0x10,
    U64 => 0x11,
    String => 0x12,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CopyableValue {
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    U64(u64),
    I64(i64),
    F64(f64),
    Boolean(bool),
    Unit,
    Fn {
        address: usize,
        max_register_count: u32,
    },
    NativeFn(NativeFn),
    ValueIndex(ValueIndex),
}

#[derive(Debug)]
pub enum Value {
    MakeCompound(Vec<ValueOrLocation<CopyableValue>>),
    MakeTaggedCompound {
        fields: Vec<ValueOrLocation<CopyableValue>>,
        tag: u16,
    },
    Compound(Vec<CopyableValue>),
    TaggedCompound {
        fields: Vec<CopyableValue>,
        tag: u16,
    },
    MakeString(String),
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueIndex(pub usize);

impl From<ValueIndex> for usize {
    fn from(value: ValueIndex) -> Self {
        value.0
    }
}

trait Assemble {
    fn to_bytes(&self, compiled: &Compiled<'_, Ir<IrValue>>) -> Vec<u8>;
}

impl Assemble for IrInstruction<IrValue> {
    #[allow(clippy::too_many_lines)]
    fn to_bytes(&self, compiled: &Compiled<'_, Ir<IrValue>>) -> Vec<u8> {
        match self {
            Self::NoOp => vec![],
            Self::Unary { op, operand, to } => {
                let mut bytes = vec![u8::from(OpCode::Unary)];

                bytes.push(u8::from(*op));

                bytes.append(&mut to.to_bytes(compiled));

                bytes.append(&mut operand.to_bytes(compiled));

                bytes
            }
            Self::Binary { op, lhs, rhs, to } => {
                let mut bytes = vec![u8::from(OpCode::Binary)];

                bytes.push(u8::from(*op));

                bytes.append(&mut to.to_bytes(compiled));

                bytes.append(&mut lhs.to_bytes(compiled));

                bytes.append(&mut rhs.to_bytes(compiled));

                bytes
            }
            Self::Assign { value, to } => {
                let mut bytes = vec![u8::from(OpCode::Assign)];

                bytes.append(&mut to.to_bytes(compiled));

                bytes.append(&mut value.to_bytes(compiled));

                bytes
            }
            Self::Push(value) => {
                let mut bytes = vec![u8::from(OpCode::Push)];

                bytes.append(&mut value.to_bytes(compiled));

                bytes
            }
            Self::Call { callee, arity, to } => {
                let mut bytes = vec![];

                if let Ok(arity) = u8::try_from(*arity).map(u8::to_le_bytes) {
                    bytes.push(u8::from(OpCode::CallU8));
                    bytes.extend_from_slice(arity.as_slice());
                } else if let Ok(arity) = u16::try_from(*arity).map(u16::to_le_bytes) {
                    bytes.push(u8::from(OpCode::CallU16));
                    bytes.extend_from_slice(arity.as_slice());
                } else if let Ok(arity) = u32::try_from(*arity).map(u32::to_le_bytes) {
                    bytes.push(u8::from(OpCode::CallU32));
                    bytes.extend_from_slice(arity.as_slice());
                } else if let Ok(arity) = u64::try_from(*arity).map(u64::to_le_bytes) {
                    bytes.push(u8::from(OpCode::CallU64));
                    bytes.extend_from_slice(arity.as_slice());
                } else {
                    panic!("128-bit arity not allowed!  sorry!");
                }

                bytes.append(&mut to.to_bytes(compiled));

                bytes.append(&mut callee.to_bytes(compiled));

                bytes
            }
            Self::Access { index, of, to } => {
                let mut bytes = vec![];

                if let Ok(index) = u8::try_from(*index).map(u8::to_le_bytes) {
                    bytes.push(u8::from(OpCode::AccessU8));
                    bytes.extend_from_slice(index.as_slice());
                } else if let Ok(index) = u16::try_from(*index).map(u16::to_le_bytes) {
                    bytes.push(u8::from(OpCode::AccessU16));
                    bytes.extend_from_slice(index.as_slice());
                } else if let Ok(index) = u32::try_from(*index).map(u32::to_le_bytes) {
                    bytes.push(u8::from(OpCode::AccessU32));
                    bytes.extend_from_slice(index.as_slice());
                } else if let Ok(index) = u64::try_from(*index).map(u64::to_le_bytes) {
                    bytes.push(u8::from(OpCode::AccessU64));
                    bytes.extend_from_slice(index.as_slice());
                } else {
                    panic!("128-bit index not allowed!  sorry!");
                }

                bytes.append(&mut to.to_bytes(compiled));

                bytes.append(&mut of.to_bytes(compiled));

                bytes
            }
            Self::AccessAssign { index, of, value } => {
                let mut bytes = vec![];

                if let Ok(index) = u8::try_from(*index).map(u8::to_le_bytes) {
                    bytes.push(u8::from(OpCode::AccessAssignU8));
                    bytes.extend_from_slice(index.as_slice());
                } else if let Ok(index) = u16::try_from(*index).map(u16::to_le_bytes) {
                    bytes.push(u8::from(OpCode::AccessAssignU16));
                    bytes.extend_from_slice(index.as_slice());
                } else if let Ok(index) = u32::try_from(*index).map(u32::to_le_bytes) {
                    bytes.push(u8::from(OpCode::AccessAssignU32));
                    bytes.extend_from_slice(index.as_slice());
                } else if let Ok(index) = u64::try_from(*index).map(u64::to_le_bytes) {
                    bytes.push(u8::from(OpCode::AccessAssignU64));
                    bytes.extend_from_slice(index.as_slice());
                } else {
                    panic!("128-bit index not allowed!  sorry!");
                }

                bytes.append(&mut of.to_bytes(compiled));

                bytes.append(&mut value.to_bytes(compiled));

                bytes
            }
            Self::GetTag { of, to } => {
                let mut bytes = vec![u8::from(OpCode::GetTag)];

                bytes.append(&mut to.to_bytes(compiled));

                bytes.append(&mut of.to_bytes(compiled));

                bytes
            }
            Self::PopN(count) => {
                let mut bytes = vec![];

                if let Ok(count) = u8::try_from(*count).map(u8::to_le_bytes) {
                    bytes.push(u8::from(OpCode::PopNU8));
                    bytes.extend_from_slice(count.as_slice());
                } else if let Ok(count) = u16::try_from(*count).map(u16::to_le_bytes) {
                    bytes.push(u8::from(OpCode::PopNU16));
                    bytes.extend_from_slice(count.as_slice());
                } else if let Ok(count) = u32::try_from(*count).map(u32::to_le_bytes) {
                    bytes.push(u8::from(OpCode::PopNU32));
                    bytes.extend_from_slice(count.as_slice());
                } else if let Ok(count) = u64::try_from(*count).map(u64::to_le_bytes) {
                    bytes.push(u8::from(OpCode::PopNU64));
                    bytes.extend_from_slice(count.as_slice());
                } else {
                    panic!("128-bit pop count not allowed!  sorry!");
                }

                bytes
            }
            Self::Jump(destination) => {
                let mut bytes = vec![u8::from(OpCode::JumpU64)];

                bytes.extend_from_slice(
                    u64::try_from(usize::from(*destination))
                        .map(u64::to_le_bytes)
                        .expect("128-bit jump destination not allowed!  sorry!")
                        .as_slice(),
                );

                bytes
            }
            Self::Branch {
                condition,
                when_true,
                otherwise,
            } => {
                let mut bytes = vec![u8::from(OpCode::BranchU64)];

                bytes.extend_from_slice(
                    u64::try_from(usize::from(*when_true))
                        .map(u64::to_le_bytes)
                        .expect("128-bit jump destination not allowed!  sorry!")
                        .as_slice(),
                );

                bytes.extend_from_slice(
                    u64::try_from(usize::from(*otherwise))
                        .map(u64::to_le_bytes)
                        .expect("128-bit jump destination not allowed!  sorry!")
                        .as_slice(),
                );

                bytes.append(&mut condition.to_bytes(compiled));

                bytes
            }
            Self::Return(value) => {
                let mut bytes = vec![u8::from(OpCode::Return)];

                bytes.append(&mut value.to_bytes(compiled));

                bytes
            }
        }
    }
}

impl Assemble for ValueOrLocation<IrValue> {
    fn to_bytes(&self, compiled: &Compiled<'_, Ir<IrValue>>) -> Vec<u8> {
        match self {
            Self::Value(value) => {
                let mut bytes = vec![u8::from(ValueAt::Value)];

                bytes.append(&mut value.to_bytes(compiled));

                bytes
            }
            Self::At(location) => location.to_bytes(compiled),
        }
    }
}

impl Assemble for Location {
    fn to_bytes(&self, _: &Compiled<'_, Ir<IrValue>>) -> Vec<u8> {
        match self {
            Self::Register(index) => {
                let mut bytes = vec![];

                let register = usize::from(*index);

                if let Ok(register) = u8::try_from(register).map(u8::to_le_bytes) {
                    bytes.push(u8::from(ValueAt::RegisterU8));
                    bytes.extend_from_slice(register.as_slice());
                } else if let Ok(register) = u16::try_from(register).map(u16::to_le_bytes) {
                    bytes.push(u8::from(ValueAt::RegisterU16));
                    bytes.extend_from_slice(register.as_slice());
                } else if let Ok(register) = u32::try_from(register).map(u32::to_le_bytes) {
                    bytes.push(u8::from(ValueAt::RegisterU32));
                    bytes.extend_from_slice(register.as_slice());
                } else if let Ok(register) = u64::try_from(register).map(u64::to_le_bytes) {
                    bytes.push(u8::from(ValueAt::RegisterU64));
                    bytes.extend_from_slice(register.as_slice());
                } else {
                    panic!("128-bit location not allowed!  sorry!");
                }

                bytes
            }
        }
    }
}

impl Assemble for IrValue {
    #[allow(clippy::too_many_lines)]
    fn to_bytes(&self, compiled: &Compiled<'_, Ir<Self>>) -> Vec<u8> {
        match self {
            Self::U8(value) => {
                let mut bytes = vec![u8::from(TypeId::U8)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::I8(value) => {
                let mut bytes = vec![u8::from(TypeId::I8)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::U16(value) => {
                let mut bytes = vec![u8::from(TypeId::U16)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::I16(value) => {
                let mut bytes = vec![u8::from(TypeId::I16)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::U32(value) => {
                let mut bytes = vec![u8::from(TypeId::U32)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::I32(value) => {
                let mut bytes = vec![u8::from(TypeId::I32)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::U64(value) => {
                let mut bytes = vec![u8::from(TypeId::U64)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::I64(value) => {
                let mut bytes = vec![u8::from(TypeId::I64)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::F64(value) => {
                let mut bytes = vec![u8::from(TypeId::F64)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::Boolean(value) => {
                vec![u8::from(TypeId::Boolean), u8::from(*value)]
            }
            Self::Unit => {
                vec![u8::from(TypeId::Unit)]
            }
            Self::String(text) => {
                let mut bytes = vec![u8::from(TypeId::String)];

                bytes.extend_from_slice(
                    u64::try_from(text.len())
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytes.extend_from_slice(text.as_bytes());

                bytes
            }
            Self::Fn {
                block_index,
                max_register_count,
            } => {
                let mut bytes = vec![u8::from(TypeId::FnU64)];

                bytes.extend_from_slice(
                    u64::try_from(usize::from(*block_index))
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytes.extend_from_slice(
                    u32::try_from(*max_register_count)
                        .expect("a function's max register count was not a valid u32")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytes
            }
            Self::NativeFn(native_fn) => {
                let mut bytes = vec![u8::from(TypeId::NativeFn)];

                bytes.extend_from_slice(u16::from(*native_fn).to_le_bytes().as_slice());

                bytes
            }
            Self::Compound(values) => {
                let mut bytes = vec![u8::from(TypeId::Compound)];

                bytes.extend_from_slice(
                    u64::try_from(values.len())
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                for value in values {
                    bytes.extend_from_slice(value.to_bytes(compiled).as_slice());
                }

                bytes
            }
            Self::TaggedCompound { tag, fields } => {
                let mut bytes = vec![u8::from(TypeId::TaggedCompound)];

                bytes.extend_from_slice(tag.to_le_bytes().as_slice());

                bytes.extend_from_slice(
                    u64::try_from(fields.len())
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                for value in fields {
                    bytes.extend_from_slice(value.to_bytes(compiled).as_slice());
                }

                bytes
            }
        }
    }
}

pub trait Disassemble<T>
where
    T: Instructive,
{
    fn from_bytes(bytes: &[u8], instructive: &mut T) -> Self;
}

macro_rules! disassemble_number_from {
    (
        $bytes:ident, $instructive:ident ;
        $from:ident $(,)?
    ) => {{
        const SIZE: usize = std::mem::size_of::<$from>();

        $bytes[(*$instructive.ip_mut())..{
            *$instructive.ip_mut() += SIZE;
            *$instructive.ip_mut()
        }]
            .as_array::<SIZE>()
            .map(|array| $from::from_le_bytes(*array))
    }};
}

macro_rules! disassemble_number_and_convert {
    (
        $bytes:ident, $instructive:ident ;
        $from:ident, $to:ident $(,)?
    ) => {{
        disassemble_number_from!(
            $bytes, $instructive ;
            $from,
        )
        .and_then(|field_count| <$to>::try_from(field_count).ok())
    }};
}

pub trait Instructive: Sized {
    fn ip_mut(&mut self) -> &mut usize;

    fn instructions_mut(&mut self) -> &mut Vec<IrInstruction<CopyableValue>>;

    fn values_mut(&mut self) -> &mut Vec<Value>;

    #[allow(clippy::too_many_lines)]
    fn read(&mut self, bytes: &[u8]) {
        *self.ip_mut() = 8;

        let mut block_addresses = HashMap::new();

        while *self.ip_mut() < bytes.len() {
            block_addresses.insert(*self.ip_mut(), self.instructions_mut().len());

            let op_code = OpCode::from_bytes(bytes, self);

            match op_code {
                OpCode::Label => unreachable!("labels are eliminated before compilation ends"),
                OpCode::Unary => {
                    let op = UnaryOp::from_bytes(bytes, self);

                    let to = Location::from_bytes(bytes, self);

                    let operand = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::Unary { op, operand, to });
                }
                OpCode::Binary => {
                    let op = BinaryOp::from_bytes(bytes, self);

                    let to = Location::from_bytes(bytes, self);

                    let lhs = ValueOrLocation::from_bytes(bytes, self);

                    let rhs = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::Binary { op, lhs, rhs, to });
                }
                OpCode::Assign => {
                    let to = Location::from_bytes(bytes, self);

                    let value = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::Assign { to, value });
                }
                OpCode::Push => {
                    let value = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut().push(IrInstruction::Push(value));
                }
                OpCode::AccessU8 => {
                    let index = disassemble_number_and_convert!(
                        bytes, self ;
                        u8, usize,
                    )
                    .expect("access index is not a valid usize");

                    let to = Location::from_bytes(bytes, self);

                    let of = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::Access { index, of, to });
                }
                OpCode::AccessU16 => {
                    let index = disassemble_number_and_convert!(
                        bytes, self ;
                        u16, usize,
                    )
                    .expect("access index is not a valid usize");

                    let to = Location::from_bytes(bytes, self);

                    let of = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::Access { index, of, to });
                }
                OpCode::AccessU32 => {
                    let index = disassemble_number_and_convert!(
                        bytes, self ;
                        u32, usize,
                    )
                    .expect("access index is not a valid usize");

                    let to = Location::from_bytes(bytes, self);

                    let of = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::Access { index, of, to });
                }
                OpCode::AccessU64 => {
                    let index = disassemble_number_and_convert!(
                        bytes, self ;
                        u64, usize,
                    )
                    .expect("access index is not a valid usize");

                    let to = Location::from_bytes(bytes, self);

                    let of = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::Access { index, of, to });
                }
                OpCode::AccessAssignU8 => {
                    let index = disassemble_number_and_convert!(
                        bytes, self ;
                        u8, usize,
                    )
                    .expect("access index is not a valid usize");

                    let of = ValueOrLocation::from_bytes(bytes, self);

                    let value = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::AccessAssign { index, of, value });
                }
                OpCode::AccessAssignU16 => {
                    let index = disassemble_number_and_convert!(
                        bytes, self ;
                        u16, usize,
                    )
                    .expect("access index is not a valid usize");

                    let of = ValueOrLocation::from_bytes(bytes, self);

                    let value = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::AccessAssign { index, of, value });
                }
                OpCode::AccessAssignU32 => {
                    let index = disassemble_number_and_convert!(
                        bytes, self ;
                        u32, usize,
                    )
                    .expect("access index is not a valid usize");

                    let of = ValueOrLocation::from_bytes(bytes, self);

                    let value = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::AccessAssign { index, of, value });
                }
                OpCode::AccessAssignU64 => {
                    let index = disassemble_number_and_convert!(
                        bytes, self ;
                        u64, usize,
                    )
                    .expect("access index is not a valid usize");

                    let of = ValueOrLocation::from_bytes(bytes, self);

                    let value = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::AccessAssign { index, of, value });
                }
                OpCode::GetTag => {
                    let to = Location::from_bytes(bytes, self);

                    let of = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::GetTag { of, to });
                }
                OpCode::PopNU8 => {
                    let count = disassemble_number_and_convert!(
                        bytes, self ;
                        u8, usize,
                    )
                    .expect("pop count is not a valid usize");

                    self.instructions_mut().push(IrInstruction::PopN(count));
                }
                OpCode::PopNU16 => {
                    let count = disassemble_number_and_convert!(
                        bytes, self ;
                        u16, usize,
                    )
                    .expect("pop count is not a valid usize");

                    self.instructions_mut().push(IrInstruction::PopN(count));
                }
                OpCode::PopNU32 => {
                    let count = disassemble_number_and_convert!(
                        bytes, self ;
                        u32, usize,
                    )
                    .expect("pop count is not a valid usize");

                    self.instructions_mut().push(IrInstruction::PopN(count));
                }
                OpCode::PopNU64 => {
                    let count = disassemble_number_and_convert!(
                        bytes, self ;
                        u64, usize,
                    )
                    .expect("pop count is not a valid usize");

                    self.instructions_mut().push(IrInstruction::PopN(count));
                }
                OpCode::CallU8 => {
                    let arity = disassemble_number_and_convert!(
                        bytes, self ;
                        u8, usize,
                    )
                    .expect("call arity is not a valid usize");

                    let to = Location::from_bytes(bytes, self);

                    let callee = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::Call { callee, arity, to });
                }
                OpCode::CallU16 => {
                    let arity = disassemble_number_and_convert!(
                        bytes, self ;
                        u16, usize,
                    )
                    .expect("call arity is not a valid usize");

                    let to = Location::from_bytes(bytes, self);

                    let callee = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::Call { callee, arity, to });
                }
                OpCode::CallU32 => {
                    let arity = disassemble_number_and_convert!(
                        bytes, self ;
                        u32, usize,
                    )
                    .expect("call arity is not a valid usize");

                    let to = Location::from_bytes(bytes, self);

                    let callee = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::Call { callee, arity, to });
                }
                OpCode::CallU64 => {
                    let arity = disassemble_number_and_convert!(
                        bytes, self ;
                        u64, usize,
                    )
                    .expect("call arity is not a valid usize");

                    let to = Location::from_bytes(bytes, self);

                    let callee = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::Call { callee, arity, to });
                }
                OpCode::JumpU8 => {
                    let to = disassemble_number_and_convert!(
                        bytes, self ;
                        u8, usize,
                    )
                    .expect("jump address is not a valid usize");

                    self.instructions_mut()
                        .push(IrInstruction::Jump(BlockIndex(to)));
                }
                OpCode::JumpU16 => {
                    let to = disassemble_number_and_convert!(
                        bytes, self ;
                        u16, usize,
                    )
                    .expect("jump address is not a valid usize");

                    self.instructions_mut()
                        .push(IrInstruction::Jump(BlockIndex(to)));
                }
                OpCode::JumpU32 => {
                    let to = disassemble_number_and_convert!(
                        bytes, self ;
                        u32, usize,
                    )
                    .expect("jump address is not a valid usize");

                    self.instructions_mut()
                        .push(IrInstruction::Jump(BlockIndex(to)));
                }
                OpCode::JumpU64 => {
                    let to = disassemble_number_and_convert!(
                        bytes, self ;
                        u64, usize,
                    )
                    .expect("jump address is not a valid usize");

                    self.instructions_mut()
                        .push(IrInstruction::Jump(BlockIndex(to)));
                }
                OpCode::BranchU8 => {
                    let when_true = disassemble_number_and_convert!(
                        bytes, self ;
                        u8, usize,
                    )
                    .expect("jump address is not a valid usize");

                    let otherwise = disassemble_number_and_convert!(
                        bytes, self ;
                        u8, usize,
                    )
                    .expect("jump address is not a valid usize");

                    let condition = ValueOrLocation::from_bytes(bytes, self);

                    let when_true = BlockIndex(when_true);

                    let otherwise = BlockIndex(otherwise);

                    self.instructions_mut().push(IrInstruction::Branch {
                        condition,
                        when_true,
                        otherwise,
                    });
                }
                OpCode::BranchU16 => {
                    let when_true = disassemble_number_and_convert!(
                        bytes, self ;
                        u16, usize,
                    )
                    .expect("jump address is not a valid usize");

                    let otherwise = disassemble_number_and_convert!(
                        bytes, self ;
                        u16, usize,
                    )
                    .expect("jump address is not a valid usize");

                    let condition = ValueOrLocation::from_bytes(bytes, self);

                    let when_true = BlockIndex(when_true);

                    let otherwise = BlockIndex(otherwise);

                    self.instructions_mut().push(IrInstruction::Branch {
                        condition,
                        when_true,
                        otherwise,
                    });
                }
                OpCode::BranchU32 => {
                    let when_true = disassemble_number_and_convert!(
                        bytes, self ;
                        u32, usize,
                    )
                    .expect("jump address is not a valid usize");

                    let otherwise = disassemble_number_and_convert!(
                        bytes, self ;
                        u32, usize,
                    )
                    .expect("jump address is not a valid usize");

                    let condition = ValueOrLocation::from_bytes(bytes, self);

                    let when_true = BlockIndex(when_true);

                    let otherwise = BlockIndex(otherwise);

                    self.instructions_mut().push(IrInstruction::Branch {
                        condition,
                        when_true,
                        otherwise,
                    });
                }
                OpCode::BranchU64 => {
                    let when_true = disassemble_number_and_convert!(
                        bytes, self ;
                        u64, usize,
                    )
                    .expect("jump address is not a valid usize");

                    let otherwise = disassemble_number_and_convert!(
                        bytes, self ;
                        u64, usize,
                    )
                    .expect("jump address is not a valid usize");

                    let condition = ValueOrLocation::from_bytes(bytes, self);

                    let when_true = BlockIndex(when_true);

                    let otherwise = BlockIndex(otherwise);

                    self.instructions_mut().push(IrInstruction::Branch {
                        condition,
                        when_true,
                        otherwise,
                    });
                }
                OpCode::Return => {
                    let value = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut().push(IrInstruction::Return(value));
                }
            }
        }

        for instruction in self.instructions_mut() {
            match instruction {
                IrInstruction::NoOp | IrInstruction::PopN(_) => {}
                IrInstruction::Unary { operand: value, .. }
                | IrInstruction::Assign { value, .. }
                | IrInstruction::Push(value)
                | IrInstruction::Access { of: value, .. }
                | IrInstruction::GetTag { of: value, .. }
                | IrInstruction::Call { callee: value, .. }
                | IrInstruction::Return(value) => {
                    if let ValueOrLocation::Value(CopyableValue::Fn { address, .. }) = value {
                        *address = block_addresses.get(address).copied().unwrap_or_else(|| {
                            panic!(
                                "tried to backpatch a function value to an invalid address {address:#X}",
                            );
                        });
                    }
                }
                IrInstruction::Binary { lhs, rhs, .. }
                | IrInstruction::AccessAssign {
                    of: lhs,
                    value: rhs,
                    ..
                } => {
                    if let ValueOrLocation::Value(CopyableValue::Fn { address, .. }) = lhs {
                        *address = block_addresses.get(address).copied().unwrap_or_else(|| {
                            panic!(
                                "tried to backpatch a function value to an invalid address {address:#X}",
                            );
                        });
                    }

                    if let ValueOrLocation::Value(CopyableValue::Fn { address, .. }) = rhs {
                        *address = block_addresses.get(address).copied().unwrap_or_else(|| {
                            panic!(
                                "tried to backpatch a function value to an invalid address {address:#X}",
                            );
                        });
                    }
                }
                IrInstruction::Jump(destination) => {
                    *destination = BlockIndex(
                        block_addresses
                            .get(&usize::from(*destination))
                            .copied()
                            .unwrap_or_else(|| {
                                panic!(
                                "tried to backpatch a function value to an invalid address {:#X}",
                                    usize::from(*destination)
                                );
                            }),
                    );
                }
                IrInstruction::Branch {
                    condition,
                    when_true,
                    otherwise,
                } => {
                    if let ValueOrLocation::Value(CopyableValue::Fn { address, .. }) = condition {
                        *address = block_addresses.get(address).copied().unwrap_or_else(|| {
                            panic!(
                                "tried to backpatch a function value to an invalid address {address:#X}",
                            );
                        });
                    }

                    *when_true = BlockIndex(
                        block_addresses
                            .get(&usize::from(*when_true))
                            .copied()
                            .unwrap_or_else(|| {
                                panic!(
                                "tried to backpatch a function value to an invalid address {:#X}",
                                    usize::from(*when_true)
                                );
                            }),
                    );

                    *otherwise = BlockIndex(
                        block_addresses
                            .get(&usize::from(*otherwise))
                            .copied()
                            .unwrap_or_else(|| {
                                panic!(
                                    "tried to backpatch a function value to an invalid address {:#X}",
                                    usize::from(*otherwise)
                                );
                            }),
                    );
                }
            }
        }

        *self.ip_mut() = 0;
    }
}

impl<T> Disassemble<T> for OpCode
where
    T: Instructive,
{
    fn from_bytes(bytes: &[u8], instructive: &mut T) -> Self {
        match Self::try_from(
            bytes[{
                let ip = *instructive.ip_mut();
                *instructive.ip_mut() += 1;
                ip
            }],
        ) {
            Ok(op_code) => op_code,
            Err(error) => panic!("unknown opcode {error:#X}"),
        }
    }
}

impl<T> Disassemble<T> for UnaryOp
where
    T: Instructive,
{
    fn from_bytes(bytes: &[u8], instructive: &mut T) -> Self {
        match Self::try_from(
            bytes[{
                let ip = *instructive.ip_mut();
                *instructive.ip_mut() += 1;
                ip
            }],
        ) {
            Ok(op_code) => op_code,
            Err(error) => panic!("unknown unary op {error:#X}"),
        }
    }
}

impl<T> Disassemble<T> for BinaryOp
where
    T: Instructive,
{
    fn from_bytes(bytes: &[u8], instructive: &mut T) -> Self {
        match Self::try_from(
            bytes[{
                let ip = *instructive.ip_mut();
                *instructive.ip_mut() += 1;
                ip
            }],
        ) {
            Ok(op_code) => op_code,
            Err(error) => panic!("unknown binary op {error:#X}"),
        }
    }
}

impl<T> Disassemble<T> for ValueOrLocation<CopyableValue>
where
    T: Instructive,
{
    fn from_bytes(bytes: &[u8], instructive: &mut T) -> Self {
        match ValueAt::try_from(bytes[*instructive.ip_mut()]) {
            Ok(ValueAt::Value) => {
                *instructive.ip_mut() += 1;

                Self::Value(CopyableValue::from_bytes(bytes, instructive))
            }
            Ok(
                ValueAt::RegisterU8
                | ValueAt::RegisterU16
                | ValueAt::RegisterU32
                | ValueAt::RegisterU64,
            ) => Self::At(Location::from_bytes(bytes, instructive)),
            Err(error) => panic!("invalid value op {error:#X}"),
        }
    }
}

impl<T> Disassemble<T> for Location
where
    T: Instructive,
{
    fn from_bytes(bytes: &[u8], instructive: &mut T) -> Self {
        match ValueAt::try_from(bytes[*instructive.ip_mut()]) {
            Ok(
                ValueAt::RegisterU8
                | ValueAt::RegisterU16
                | ValueAt::RegisterU32
                | ValueAt::RegisterU64,
            ) => Self::Register(Register::from_bytes(bytes, instructive)),
            Ok(ValueAt::Value) => panic!("got a value where a location was expected"),
            Err(error) => panic!("invalid location op {error:#X}"),
        }
    }
}

impl<T> Disassemble<T> for Register
where
    T: Instructive,
{
    fn from_bytes(bytes: &[u8], instructive: &mut T) -> Self {
        match ValueAt::try_from(
            bytes[{
                let ip = *instructive.ip_mut();
                *instructive.ip_mut() += 1;
                ip
            }],
        ) {
            Ok(ValueAt::RegisterU8) => Self(
                disassemble_number_and_convert!(
                    bytes, instructive ;
                    u8, usize,
                )
                .expect("128-bit register not allowed!  sorry!"),
            ),
            Ok(ValueAt::RegisterU16) => Self(
                disassemble_number_and_convert!(
                    bytes, instructive ;
                    u16, usize,
                )
                .expect("128-bit register not allowed!  sorry!"),
            ),
            Ok(ValueAt::RegisterU32) => Self(
                disassemble_number_and_convert!(
                    bytes, instructive ;
                    u32, usize,
                )
                .expect("128-bit register not allowed!  sorry!"),
            ),
            Ok(ValueAt::RegisterU64) => Self(
                disassemble_number_and_convert!(
                    bytes, instructive ;
                    u64, usize,
                )
                .expect("128-bit register not allowed!  sorry!"),
            ),
            Ok(ValueAt::Value) => panic!("got a value where a register was expected"),
            Err(error) => panic!("invalid register op {error:#X}"),
        }
    }
}

impl<T> Disassemble<T> for CopyableValue
where
    T: Instructive,
{
    #[allow(clippy::too_many_lines)]
    fn from_bytes(bytes: &[u8], instructive: &mut T) -> Self {
        match TypeId::try_from(
            bytes[{
                let ip = *instructive.ip_mut();
                *instructive.ip_mut() += 1;
                ip
            }],
        ) {
            Ok(TypeId::U8) => Self::U8(
                disassemble_number_from!(
                    bytes, instructive ;
                    u8,
                )
                .expect("the range is the same length as the expected array"),
            ),
            Ok(TypeId::I8) => Self::I8(
                disassemble_number_from!(
                    bytes, instructive ;
                    i8,
                )
                .expect("the range is the same length as the expected array"),
            ),
            Ok(TypeId::U16) => Self::U16(
                disassemble_number_from!(
                    bytes, instructive ;
                    u16,
                )
                .expect("the range is the same length as the expected array"),
            ),
            Ok(TypeId::I16) => Self::I16(
                disassemble_number_from!(
                    bytes, instructive ;
                    i16,
                )
                .expect("the range is the same length as the expected array"),
            ),
            Ok(TypeId::U32) => Self::U32(
                disassemble_number_from!(
                    bytes, instructive ;
                    u32,
                )
                .expect("the range is the same length as the expected array"),
            ),
            Ok(TypeId::I32) => Self::I32(
                disassemble_number_from!(
                    bytes, instructive ;
                    i32,
                )
                .expect("the range is the same length as the expected array"),
            ),
            Ok(TypeId::U64) => Self::U64(
                disassemble_number_from!(
                    bytes, instructive ;
                    u64,
                )
                .expect("the range is the same length as the expected array"),
            ),
            Ok(TypeId::I64) => Self::I64(
                disassemble_number_from!(
                    bytes, instructive ;
                    i64,
                )
                .expect("the range is the same length as the expected array"),
            ),
            Ok(TypeId::F64) => Self::F64(
                disassemble_number_from!(
                    bytes, instructive ;
                    f64,
                )
                .expect("the range is the same length as the expected array"),
            ),
            Ok(TypeId::Boolean) => Self::Boolean(
                bool::try_from(
                    bytes[{
                        let ip = *instructive.ip_mut();
                        *instructive.ip_mut() += 1;
                        ip
                    }],
                )
                .expect("a boolean could not be created from its byte"),
            ),
            Ok(TypeId::Unit) => Self::Unit,
            Ok(TypeId::String) => {
                let text = disassemble_number_and_convert!(
                    bytes, instructive ;
                    u64, usize,
                )
                .and_then(|length| {
                    String::from_utf8(
                        bytes[(*instructive.ip_mut())..{
                            *instructive.ip_mut() += length;
                            *instructive.ip_mut()
                        }]
                            .to_vec(),
                    )
                    .ok()
                })
                .expect("a string wasn't valid utf-8");

                instructive.values_mut().push(Value::MakeString(text));

                Self::ValueIndex(ValueIndex(instructive.values_mut().len() - 1))
            }
            Ok(TypeId::FnU8) => {
                let block_index = disassemble_number_and_convert!(
                    bytes, instructive ;
                    u8, usize,
                )
                .expect("a function was not a valid usize");

                let max_register_count = disassemble_number_from!(
                    bytes, instructive ;
                    u32,
                )
                .expect("a function's max register count was not a valid u32");

                Self::Fn {
                    address: block_index,
                    max_register_count,
                }
            }
            Ok(TypeId::FnU16) => {
                let block_index = disassemble_number_and_convert!(
                    bytes, instructive ;
                    u16, usize,
                )
                .expect("a function was not a valid usize");

                let max_register_count = disassemble_number_from!(
                    bytes, instructive ;
                    u32,
                )
                .expect("a function's max register count was not a valid u32");

                Self::Fn {
                    address: block_index,
                    max_register_count,
                }
            }
            Ok(TypeId::FnU32) => {
                let block_index = disassemble_number_and_convert!(
                    bytes, instructive ;
                    u32, usize,
                )
                .expect("a function was not a valid usize");

                let max_register_count = disassemble_number_from!(
                    bytes, instructive ;
                    u32,
                )
                .expect("a function's max register count was not a valid u32");

                Self::Fn {
                    address: block_index,
                    max_register_count,
                }
            }
            Ok(TypeId::FnU64) => {
                let block_index = disassemble_number_and_convert!(
                    bytes, instructive ;
                    u64, usize,
                )
                .expect("a function was not a valid usize");

                let max_register_count = disassemble_number_from!(
                    bytes, instructive ;
                    u32,
                )
                .expect("a function's max register count was not a valid u32");

                Self::Fn {
                    address: block_index,
                    max_register_count,
                }
            }
            Ok(TypeId::NativeFn) => Self::NativeFn(
                disassemble_number_from!(
                    bytes, instructive ;
                    u16,
                )
                .and_then(|value| NativeFn::try_from(value).ok())
                .expect("a native function was not valid"),
            ),
            Ok(TypeId::Compound) => {
                let field_count = disassemble_number_and_convert!(
                    bytes, instructive ;
                    u64, usize,
                )
                .expect("a compound's field count was not a valid usize");

                let mut fields = vec![];

                for _ in 0..field_count {
                    let field = ValueOrLocation::from_bytes(bytes, instructive);

                    fields.push(field);
                }

                instructive.values_mut().push(Value::MakeCompound(fields));

                Self::ValueIndex(ValueIndex(instructive.values_mut().len() - 1))
            }
            Ok(TypeId::TaggedCompound) => {
                let tag = disassemble_number_from!(
                    bytes, instructive ;
                    u16,
                )
                .expect("a tagged compound's tag was not a valid u16");

                let field_count = disassemble_number_and_convert!(
                    bytes, instructive ;
                    u64, usize,
                )
                .expect("a tagged compound's field count was not a valid usize");

                let mut fields = vec![];

                for _ in 0..field_count {
                    let field = ValueOrLocation::from_bytes(bytes, instructive);

                    fields.push(field);
                }

                instructive
                    .values_mut()
                    .push(Value::MakeTaggedCompound { tag, fields });

                Self::ValueIndex(ValueIndex(instructive.values_mut().len() - 1))
            }
            Err(error) => panic!("unknown type id {error:#X}"),
        }
    }
}
