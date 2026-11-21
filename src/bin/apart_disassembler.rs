use apart::{
    NativeFn,
    targets::vm::{Instructive, ValueIndex, prelude::*},
};

use std::{env, fs, path::Path, process::ExitCode};

fn main() -> ExitCode {
    let mut arguments = env::args();
    arguments.next();

    arguments.next().map_or_else(
        || {
            print_usage();

            ExitCode::FAILURE
        },
        |file_path| {
            let input_path = file_path.as_str();

            let input_file_path = Path::new(input_path);

            if !input_file_path.exists() || !input_file_path.is_file() {
                eprintln!("\"{input_path}\" does not exist as a file");
                ExitCode::FAILURE
            } else {
                match fs::read(input_file_path) {
                    Err(error) => {
                        eprintln!("error while reading \"{input_path}\": {error}");
                        ExitCode::FAILURE
                    }
                    Ok(bytecode) => disassemble(bytecode.as_slice()),
                }
            }
        },
    )
}

fn print_usage() {
    eprintln!(
        "Usage: {} [path_to_binary]\n\n{}",
        option_env!("CARGO_BIN_NAME").unwrap_or("apart_disassembler"),
        DESCRIPTION.trim(),
    );
}

const DESCRIPTION: &str = "Description:
    Disassemble a binary compiled by apart_c for the vm target
";

struct Disassembler {
    ip: usize,
    constants: Vec<ValueIndex>,
    arena: Vec<GarbageCollectedValue>,
}

impl Instructive for Disassembler {
    fn ip_mut(&mut self) -> &mut usize {
        &mut self.ip
    }

    fn constants_mut(&mut self) -> &mut Vec<ValueIndex> {
        &mut self.constants
    }

    fn arena_mut(&mut self) -> &mut Vec<GarbageCollectedValue> {
        &mut self.arena
    }
}

fn print_register(register: u16) {
    print!("r{register}");
}

#[allow(clippy::too_many_lines)]
fn print_value(disassembler: &Disassembler, value: &Value) {
    match value {
        Value::Never(_) => print!("#never"),
        Value::U8(U8 { value }) => print!("{value}u8"),
        Value::I8(I8 { value }) => print!("{value}i8"),
        Value::U16(U16 { value }) => print!("{value}u16"),
        Value::I16(I16 { value }) => print!("{value}i16"),
        Value::U32(U32 { value }) => print!("{value}u32"),
        Value::I32(I32 { value }) => print!("{value}i32"),
        Value::U64(U64 { value }) => print!("{value}u64"),
        Value::I64(I64 { value }) => print!("{value}i64"),
        Value::F32(F32 { value }) => print!("{value:?}f32"),
        Value::F64(F64 { value }) => print!("{value:?}f64"),
        Value::Boolean(Boolean { value }) => print!("{value}"),
        Value::Unit(_) => print!("{{}}"),
        Value::ApartFn(ApartFn {
            address,
            max_register_count,
        }) => print!("label{address}({max_register_count})"),
        Value::ApartNativeFn(ApartNativeFn { value: native_fn }) => match native_fn {
            NativeFn::PrintF32 => print!("print_f32"),
            NativeFn::PrintF64 => print!("print_f64"),
            NativeFn::PrintString => print!("print_string"),
            NativeFn::ExtendString => print!("extend_string"),
            NativeFn::SliceString => print!("slice_string"),
            NativeFn::LengthString => print!("length_string"),
            NativeFn::TruncateString => print!("truncate_string"),
            NativeFn::FloorCharBoundaryString => print!("floor_char_boundary_string"),
            NativeFn::CeilCharBoundaryString => print!("ceil_char_boundary_string"),
            NativeFn::Clock => print!("clock"),
            NativeFn::Sleep => print!("sleep"),
            NativeFn::NewVec => print!("new_vec"),
            NativeFn::GetVec => print!("get_vec"),
            NativeFn::ExtendVec => print!("extend_vec"),
            NativeFn::SliceVec => print!("slice_vec"),
            NativeFn::LengthVec => print!("length_vec"),
            NativeFn::TruncateVec => print!("truncate_vec"),
            NativeFn::PushVec => print!("push_vec"),
            NativeFn::PopVec => print!("pop_vec"),
            NativeFn::LogF64 => print!("log_f64"),
            NativeFn::FloorF64 => print!("floor_f64"),
            NativeFn::ToU64F64 => print!("to_u64"),
            NativeFn::FromU64F64 => print!("from_u64"),
        },
        Value::ApartRegister(ApartRegister { index }) => print!("r{index}"),
        Value::ValueIndex(value_index) => {
            match disassembler
                .arena
                .get(usize::from(*value_index))
                .expect("the value will exist")
            {
                GarbageCollectedValue::ApartString(ApartString { value: text }) => {
                    print!("\"{text}\"");
                }
                GarbageCollectedValue::Compound(Compound { fields, .. }) => {
                    print!("Compound([ ");

                    for field in fields {
                        print_value(disassembler, field);
                        print!(", ");
                    }

                    print!("])");
                }
                GarbageCollectedValue::TaggedCompound(TaggedCompound { tag, fields, .. }) => {
                    print!("TaggedCompound {{ tag: {tag}, fields: [ ");

                    for field in fields {
                        print_value(disassembler, field);
                        print!(", ");
                    }

                    print!("] }}");
                }
                GarbageCollectedValue::ApartVec(ApartVec { values, .. }) => {
                    print!("Vec([ ");

                    for value in values {
                        print_value(disassembler, value);
                        print!(", ");
                    }

                    print!("])");
                }
            }
        }
    }
}

macro_rules! print_binary_instruction {
    (
        $instruction:literal, r, r ;
        $to:ident, $lhs:ident, $rhs:ident $(,)?
    ) => {{
        print!("    {} ", $instruction);

        $crate::print_register(*$to);

        print!(", ");

        $crate::print_register(*$lhs);

        print!(", ");

        $crate::print_register(*$rhs);

        println!();
    }};

    (
        $instruction:literal, r, v ;
        $disassembler:ident ;
        $to:ident, $lhs:ident, $rhs:ident $(,)?
    ) => {{
        print!("    {} ", $instruction);

        $crate::print_register(*$to);

        print!(", ");

        $crate::print_register(*$lhs);

        print!(", ");

        $crate::print_value(&$disassembler, $rhs);

        println!();
    }};

    (
        $instruction:literal, v, r ;
        $disassembler:ident ;
        $to:ident, $lhs:ident, $rhs:ident $(,)?
    ) => {{
        print!("    {} ", $instruction);

        $crate::print_register(*$to);

        print!(", ");

        $crate::print_value(&$disassembler, $lhs);

        print!(", ");

        $crate::print_register(*$rhs);

        println!();
    }};

    (
        $instruction:literal, v, v ;
        $disassembler:ident ;
        $to:ident, $lhs:ident, $rhs:ident $(,)?
    ) => {{
        print!("    {} ", $instruction);

        $crate::print_register(*$to);

        print!(", ");

        $crate::print_value(&$disassembler, $lhs);

        print!(", ");

        $crate::print_value(&$disassembler, $rhs);

        println!();
    }};
}

#[allow(clippy::too_many_lines)]
fn disassemble(bytecode: &[u8]) -> ExitCode {
    let mut disassembler = Disassembler {
        ip: 0,
        constants: vec![],
        arena: vec![],
    };

    let instructions = apart::targets::vm::decompile(bytecode, &mut disassembler);

    let mut labels = vec![false; instructions.len()];

    for instruction in &instructions {
        find_labels_in_instruction(labels.as_mut_slice(), &disassembler, instruction);
    }

    println!("start:");

    for (i, instruction) in instructions.iter().enumerate() {
        if labels[i] {
            println!("label{i}:");
        }

        match instruction {
            Instruction::NoOp(_) => println!("    no_op"),
            Instruction::ApartFnLabel(ApartFnLabel { fn_label }) => {
                println!(
                    "    ; {} in {}",
                    fn_label.file_name(),
                    fn_label.function_label()
                );
            }
            Instruction::NotRegister(NotRegister { to, operand }) => {
                print!("    not ");

                print_register(*to);

                print!(", ");

                print_register(*operand);

                println!();
            }
            Instruction::NotValue(NotValue { to, operand }) => {
                print!("    not ");

                print_register(*to);

                print!(", ");

                print_value(&disassembler, operand);

                println!();
            }
            Instruction::NegateRegister(NegateRegister { to, operand }) => {
                print!("    negate ");

                print_register(*to);

                print!(", ");

                print_register(*operand);

                println!();
            }
            Instruction::NegateValue(NegateValue { to, operand }) => {
                print!("    negate ");

                print_register(*to);

                print!(", ");

                print_value(&disassembler, operand);

                println!();
            }
            Instruction::MultiplyRegisterRegister(MultiplyRegisterRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "multiply", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::MultiplyRegisterValue(MultiplyRegisterValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "multiply", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::MultiplyValueRegister(MultiplyValueRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "multiply", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::MultiplyValueValue(MultiplyValueValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "multiply", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::DivideRegisterRegister(DivideRegisterRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "divide", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::DivideRegisterValue(DivideRegisterValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "divide", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::DivideValueRegister(DivideValueRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "divide", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::DivideValueValue(DivideValueValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "divide", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::RemainderRegisterRegister(RemainderRegisterRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "remainder", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::RemainderRegisterValue(RemainderRegisterValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "remainder", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::RemainderValueRegister(RemainderValueRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "remainder", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::RemainderValueValue(RemainderValueValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "remainder", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::AddRegisterRegister(AddRegisterRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "add", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::AddRegisterValue(AddRegisterValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "add", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::AddValueRegister(AddValueRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "add", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::AddValueValue(AddValueValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "add", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::SubtractRegisterRegister(SubtractRegisterRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "subtract", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::SubtractRegisterValue(SubtractRegisterValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "subtract", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::SubtractValueRegister(SubtractValueRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "subtract", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::SubtractValueValue(SubtractValueValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "subtract", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::LessRegisterRegister(LessRegisterRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "less", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::LessRegisterValue(LessRegisterValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "less", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::LessValueRegister(LessValueRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "less", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::LessValueValue(LessValueValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "less", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::GreaterRegisterRegister(GreaterRegisterRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "greater", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::GreaterRegisterValue(GreaterRegisterValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "greater", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::GreaterValueRegister(GreaterValueRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "greater", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::GreaterValueValue(GreaterValueValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "greater", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::LessOrEqualRegisterRegister(LessOrEqualRegisterRegister {
                to,
                lhs,
                rhs,
            }) => {
                print_binary_instruction!(
                    "less_or_equal", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::LessOrEqualRegisterValue(LessOrEqualRegisterValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "less_or_equal", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::LessOrEqualValueRegister(LessOrEqualValueRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "less_or_equal", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::LessOrEqualValueValue(LessOrEqualValueValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "less_or_equal", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::GreaterOrEqualRegisterRegister(GreaterOrEqualRegisterRegister {
                to,
                lhs,
                rhs,
            }) => {
                print_binary_instruction!(
                    "greater_or_equal", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::GreaterOrEqualRegisterValue(GreaterOrEqualRegisterValue {
                to,
                lhs,
                rhs,
            }) => {
                print_binary_instruction!(
                    "greater_or_equal", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::GreaterOrEqualValueRegister(GreaterOrEqualValueRegister {
                to,
                lhs,
                rhs,
            }) => {
                print_binary_instruction!(
                    "greater_or_equal", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::GreaterOrEqualValueValue(GreaterOrEqualValueValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "greater_or_equal", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::EqualRegisterRegister(EqualRegisterRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "equal", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::EqualRegisterValue(EqualRegisterValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "equal", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::EqualValueRegister(EqualValueRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "equal", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::EqualValueValue(EqualValueValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "equal", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::NotEqualRegisterRegister(NotEqualRegisterRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "not_equal", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::NotEqualRegisterValue(NotEqualRegisterValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "not_equal", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::NotEqualValueRegister(NotEqualValueRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "not_equal", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::NotEqualValueValue(NotEqualValueValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "not_equal", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::AndRegisterRegister(AndRegisterRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "and", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::AndRegisterValue(AndRegisterValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "and", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::AndValueRegister(AndValueRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "and", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::AndValueValue(AndValueValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "and", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::OrRegisterRegister(OrRegisterRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "or", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::OrRegisterValue(OrRegisterValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "or", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::OrValueRegister(OrValueRegister { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "or", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::OrValueValue(OrValueValue { to, lhs, rhs }) => {
                print_binary_instruction!(
                    "or", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::AssignRegister(AssignRegister { to, operand }) => {
                print!("    assign ");

                print_register(*to);

                print!(", ");

                print_register(*operand);

                println!();
            }
            Instruction::AssignValue(AssignValue { to, operand }) => {
                print!("    assign ");

                print_register(*to);

                print!(", ");

                print_value(&disassembler, operand);

                println!();
            }
            Instruction::PushRegister(PushRegister { operand }) => {
                print!("    push ");

                print_register(*operand);

                println!();
            }
            Instruction::PushValue(PushValue { operand }) => {
                print!("    push ");

                print_value(&disassembler, operand);

                println!();
            }
            Instruction::AccessRegister(AccessRegister { to, index, of }) => {
                print!("    access ");

                print_register(*to);

                print!(", ");

                print_register(*of);

                println!("[{index}]");
            }
            Instruction::AccessValue(AccessValue { to, index, of }) => {
                print!("    access ");

                print_register(*to);

                print!(", ");

                print_value(&disassembler, of);

                println!("[{index}]");
            }
            Instruction::AccessAssignRegisterRegister(AccessAssignRegisterRegister {
                to,
                index,
                value,
            }) => {
                print!("    access_assign ");

                print_register(*to);

                print!("[{index}], ");

                print_register(*value);

                println!();
            }
            Instruction::AccessAssignRegisterValue(AccessAssignRegisterValue {
                to,
                index,
                value,
            }) => {
                print!("    access_assign ");

                print_register(*to);

                print!("[{index}], ");

                print_value(&disassembler, value);

                println!();
            }
            Instruction::AccessAssignValueRegister(AccessAssignValueRegister {
                to,
                index,
                value,
            }) => {
                print!("    access_assign ");

                print_value(&disassembler, to);

                print!("[{index}], ");

                print_register(*value);

                println!();
            }
            Instruction::AccessAssignValueValue(AccessAssignValueValue { to, index, value }) => {
                print!("    access_assign ");

                print_value(&disassembler, to);

                print!("[{index}], ");

                print_value(&disassembler, value);

                println!();
            }
            Instruction::GetTagRegister(GetTagRegister { to, of }) => {
                print!("    get_tag ");

                print_register(*to);

                print!(", ");

                print_register(*of);

                println!();
            }
            Instruction::GetTagValue(GetTagValue { to, of }) => {
                print!("    get_tag ");

                print_register(*to);

                print!(", ");

                print_value(&disassembler, of);

                println!();
            }
            Instruction::CallRegister(CallRegister { to, arity, callee }) => {
                print!("    call ");

                print_register(*to);

                print!(", ");

                print_register(*callee);

                println!("({arity})");
            }
            Instruction::CallValue(CallValue { to, arity, callee }) => {
                print!("    call ");

                print_register(*to);

                print!(", ");

                print_value(&disassembler, callee);

                println!("({arity})");
            }
            Instruction::Jump(Jump { destination }) => {
                println!("    jump label{destination}");
            }
            Instruction::BranchRegister(BranchRegister {
                condition,
                when_true,
                otherwise,
            }) => {
                print!("    branch ");

                print_register(*condition);

                println!(", label{when_true}, label{otherwise}");
            }
            Instruction::BranchValue(BranchValue {
                condition,
                when_true,
                otherwise,
            }) => {
                print!("    branch ");

                print_value(&disassembler, condition);

                println!(", label{when_true}, label{otherwise}");
            }
            Instruction::ReturnRegister(ReturnRegister { value }) => {
                print!("    return ");

                print_register(*value);

                println!();
            }
            Instruction::ReturnValue(ReturnValue { value }) => {
                print!("    return ");

                print_value(&disassembler, value);

                println!();
            }
            Instruction::Panic(_) => {
                println!("    panic");
            }
            Instruction::Label(_) => {}
        }
    }

    ExitCode::SUCCESS
}

fn find_labels(labels: &mut [bool], disassembler: &Disassembler, value: &Value) {
    match value {
        Value::Never(_)
        | Value::U8(_)
        | Value::I8(_)
        | Value::U16(_)
        | Value::I16(_)
        | Value::U32(_)
        | Value::I32(_)
        | Value::U64(_)
        | Value::I64(_)
        | Value::F32(_)
        | Value::F64(_)
        | Value::Boolean(_)
        | Value::Unit(_)
        | Value::ApartNativeFn(_)
        | Value::ApartRegister(_) => {}
        Value::ApartFn(ApartFn { address, .. }) => {
            labels[usize::try_from(*address)
                .unwrap_or_else(|_| panic!("function address was not a valid usize"))] = true;
        }
        Value::ValueIndex(value_index) => match disassembler.arena.get(usize::from(*value_index)) {
            Some(
                GarbageCollectedValue::Compound(Compound { fields, .. })
                | GarbageCollectedValue::TaggedCompound(TaggedCompound { fields, .. }),
            ) => {
                for field in fields {
                    find_labels(labels, disassembler, field);
                }
            }
            Some(GarbageCollectedValue::ApartVec(ApartVec { values, .. })) => {
                for value in values {
                    find_labels(labels, disassembler, value);
                }
            }
            Some(GarbageCollectedValue::ApartString(_)) | None => {}
        },
    }
}

#[allow(clippy::too_many_lines)]
fn find_labels_in_instruction(
    labels: &mut [bool],
    disassembler: &Disassembler,
    instruction: &Instruction,
) {
    match instruction {
        Instruction::NoOp(_)
        | Instruction::Panic(_)
        | Instruction::Label(_)
        | Instruction::ApartFnLabel(_)
        | Instruction::NotRegister(_)
        | Instruction::NegateRegister(_)
        | Instruction::MultiplyRegisterRegister(_)
        | Instruction::DivideRegisterRegister(_)
        | Instruction::RemainderRegisterRegister(_)
        | Instruction::AddRegisterRegister(_)
        | Instruction::SubtractRegisterRegister(_)
        | Instruction::LessRegisterRegister(_)
        | Instruction::GreaterRegisterRegister(_)
        | Instruction::LessOrEqualRegisterRegister(_)
        | Instruction::GreaterOrEqualRegisterRegister(_)
        | Instruction::EqualRegisterRegister(_)
        | Instruction::NotEqualRegisterRegister(_)
        | Instruction::AndRegisterRegister(_)
        | Instruction::OrRegisterRegister(_)
        | Instruction::AssignRegister(_)
        | Instruction::PushRegister(_)
        | Instruction::AccessRegister(_)
        | Instruction::AccessAssignRegisterRegister(_)
        | Instruction::CallRegister(_)
        | Instruction::GetTagRegister(_)
        | Instruction::ReturnRegister(_) => {}
        Instruction::NotValue(NotValue { operand: value, .. })
        | Instruction::NegateValue(NegateValue { operand: value, .. })
        | Instruction::MultiplyRegisterValue(MultiplyRegisterValue { rhs: value, .. })
        | Instruction::MultiplyValueRegister(MultiplyValueRegister { lhs: value, .. })
        | Instruction::DivideRegisterValue(DivideRegisterValue { rhs: value, .. })
        | Instruction::DivideValueRegister(DivideValueRegister { lhs: value, .. })
        | Instruction::RemainderRegisterValue(RemainderRegisterValue { rhs: value, .. })
        | Instruction::RemainderValueRegister(RemainderValueRegister { lhs: value, .. })
        | Instruction::AddRegisterValue(AddRegisterValue { rhs: value, .. })
        | Instruction::AddValueRegister(AddValueRegister { lhs: value, .. })
        | Instruction::SubtractRegisterValue(SubtractRegisterValue { rhs: value, .. })
        | Instruction::SubtractValueRegister(SubtractValueRegister { lhs: value, .. })
        | Instruction::LessRegisterValue(LessRegisterValue { rhs: value, .. })
        | Instruction::LessValueRegister(LessValueRegister { lhs: value, .. })
        | Instruction::GreaterRegisterValue(GreaterRegisterValue { rhs: value, .. })
        | Instruction::GreaterValueRegister(GreaterValueRegister { lhs: value, .. })
        | Instruction::LessOrEqualRegisterValue(LessOrEqualRegisterValue { rhs: value, .. })
        | Instruction::LessOrEqualValueRegister(LessOrEqualValueRegister { lhs: value, .. })
        | Instruction::GreaterOrEqualRegisterValue(GreaterOrEqualRegisterValue {
            rhs: value,
            ..
        })
        | Instruction::GreaterOrEqualValueRegister(GreaterOrEqualValueRegister {
            lhs: value,
            ..
        })
        | Instruction::EqualRegisterValue(EqualRegisterValue { rhs: value, .. })
        | Instruction::EqualValueRegister(EqualValueRegister { lhs: value, .. })
        | Instruction::NotEqualRegisterValue(NotEqualRegisterValue { rhs: value, .. })
        | Instruction::NotEqualValueRegister(NotEqualValueRegister { lhs: value, .. })
        | Instruction::AndRegisterValue(AndRegisterValue { rhs: value, .. })
        | Instruction::AndValueRegister(AndValueRegister { lhs: value, .. })
        | Instruction::OrRegisterValue(OrRegisterValue { rhs: value, .. })
        | Instruction::OrValueRegister(OrValueRegister { lhs: value, .. })
        | Instruction::AssignValue(AssignValue { operand: value, .. })
        | Instruction::PushValue(PushValue { operand: value })
        | Instruction::AccessValue(AccessValue { of: value, .. })
        | Instruction::AccessAssignRegisterValue(AccessAssignRegisterValue { value, .. })
        | Instruction::AccessAssignValueRegister(AccessAssignValueRegister { to: value, .. })
        | Instruction::CallValue(CallValue { callee: value, .. })
        | Instruction::GetTagValue(GetTagValue { of: value, .. })
        | Instruction::ReturnValue(ReturnValue { value }) => {
            find_labels(labels, disassembler, value);
        }
        Instruction::MultiplyValueValue(MultiplyValueValue { lhs, rhs, .. })
        | Instruction::DivideValueValue(DivideValueValue { lhs, rhs, .. })
        | Instruction::RemainderValueValue(RemainderValueValue { lhs, rhs, .. })
        | Instruction::AddValueValue(AddValueValue { lhs, rhs, .. })
        | Instruction::SubtractValueValue(SubtractValueValue { lhs, rhs, .. })
        | Instruction::LessValueValue(LessValueValue { lhs, rhs, .. })
        | Instruction::GreaterValueValue(GreaterValueValue { lhs, rhs, .. })
        | Instruction::LessOrEqualValueValue(LessOrEqualValueValue { lhs, rhs, .. })
        | Instruction::GreaterOrEqualValueValue(GreaterOrEqualValueValue { lhs, rhs, .. })
        | Instruction::EqualValueValue(EqualValueValue { lhs, rhs, .. })
        | Instruction::NotEqualValueValue(NotEqualValueValue { lhs, rhs, .. })
        | Instruction::AndValueValue(AndValueValue { lhs, rhs, .. })
        | Instruction::OrValueValue(OrValueValue { lhs, rhs, .. })
        | Instruction::AccessAssignValueValue(AccessAssignValueValue {
            to: lhs,
            value: rhs,
            ..
        }) => {
            find_labels(labels, disassembler, lhs);
            find_labels(labels, disassembler, rhs);
        }
        Instruction::Jump(Jump { destination }) => {
            labels[usize::try_from(*destination)
                .unwrap_or_else(|_| panic!("function address was not a valid usize"))] = true;
        }
        Instruction::BranchRegister(BranchRegister {
            when_true,
            otherwise,
            ..
        }) => {
            labels[usize::try_from(*when_true)
                .unwrap_or_else(|_| panic!("function address was not a valid usize"))] = true;

            labels[usize::try_from(*otherwise)
                .unwrap_or_else(|_| panic!("function address was not a valid usize"))] = true;
        }
        Instruction::BranchValue(BranchValue {
            condition,
            when_true,
            otherwise,
        }) => {
            find_labels(labels, disassembler, condition);

            labels[usize::try_from(*when_true)
                .unwrap_or_else(|_| panic!("function address was not a valid usize"))] = true;

            labels[usize::try_from(*otherwise)
                .unwrap_or_else(|_| panic!("function address was not a valid usize"))] = true;
        }
    }
}
