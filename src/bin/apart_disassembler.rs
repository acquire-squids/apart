use apart::{
    NativeFn,
    targets::vm::{Instructive, prelude::*},
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
    literals: Vec<ValueIndex>,
    arena: Vec<GarbageCollectedValue>,
}

impl Instructive for Disassembler {
    fn ip_mut(&mut self) -> &mut usize {
        &mut self.ip
    }

    fn literals_mut(&mut self) -> &mut Vec<ValueIndex> {
        &mut self.literals
    }

    fn arena_mut(&mut self) -> &mut Vec<GarbageCollectedValue> {
        &mut self.arena
    }

    fn ip(&self) -> usize {
        self.ip
    }

    fn literals(&self) -> &[ValueIndex] {
        self.literals.as_slice()
    }

    fn arena(&self) -> &[GarbageCollectedValue] {
        self.arena.as_slice()
    }
}

fn print_register(register: u16) {
    print!("r{register}");
}

#[allow(clippy::too_many_lines)]
fn print_value(disassembler: &Disassembler, value: &Value) {
    match value {
        Value::Never => print!("#never"),
        Value::U8 { value } => print!("{value}u8"),
        Value::I8 { value } => print!("{value}i8"),
        Value::U16 { value } => print!("{value}u16"),
        Value::I16 { value } => print!("{value}i16"),
        Value::U32 { value } => print!("{value}u32"),
        Value::I32 { value } => print!("{value}i32"),
        Value::U64 { value } => print!("{value}u64"),
        Value::I64 { value } => print!("{value}i64"),
        Value::F32 { value } => print!("{value:?}f32"),
        Value::F64 { value } => print!("{value:?}f64"),
        Value::Boolean { value } => print!("{value}"),
        Value::Unit => print!("{{}}"),
        Value::ApartFn {
            address,
            max_register_count,
        } => print!("label{address}({max_register_count})"),
        Value::ApartNativeFn { value: native_fn } => match native_fn {
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
        Value::ApartRegister { index } => print!("r{index}"),
        Value::ValueIndex { index: value_index } => {
            match disassembler
                .arena
                .get(usize::from(*value_index))
                .expect("the value will exist")
            {
                GarbageCollectedValue::ApartString { value: text } => {
                    print!("\"{text}\"");
                }
                GarbageCollectedValue::Compound { fields, .. } => {
                    print!("Compound([ ");

                    for field in fields {
                        print_value(disassembler, field);
                        print!(", ");
                    }

                    print!("])");
                }
                GarbageCollectedValue::TaggedCompound { tag, fields, .. } => {
                    print!("TaggedCompound {{ tag: {tag}, fields: [ ");

                    for field in fields {
                        print_value(disassembler, field);
                        print!(", ");
                    }

                    print!("] }}");
                }
                GarbageCollectedValue::ApartVec { values, .. } => {
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
        literals: vec![],
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
            Instruction::NoOp => println!("    no_op"),
            Instruction::ApartFnLabel { fn_label } => {
                println!(
                    "    ; {} in {}",
                    fn_label.file_name(),
                    fn_label.function_label()
                );
            }
            Instruction::Not { to, operand } => {
                print!("    not ");

                print_register(*to);

                print!(", ");

                print_register(*operand);

                println!();
            }
            Instruction::NotValue { to, operand } => {
                print!("    not ");

                print_register(*to);

                print!(", ");

                print_value(&disassembler, operand);

                println!();
            }
            Instruction::Negate { to, operand } => {
                print!("    negate ");

                print_register(*to);

                print!(", ");

                print_register(*operand);

                println!();
            }
            Instruction::NegateValue { to, operand } => {
                print!("    negate ");

                print_register(*to);

                print!(", ");

                print_value(&disassembler, operand);

                println!();
            }
            Instruction::Multiply { to, lhs, rhs } => {
                print_binary_instruction!(
                    "multiply", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::MultiplyRegisterValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "multiply", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::MultiplyValueRegister { to, lhs, rhs } => {
                print_binary_instruction!(
                    "multiply", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::MultiplyValueValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "multiply", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::Divide { to, lhs, rhs } => {
                print_binary_instruction!(
                    "divide", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::DivideRegisterValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "divide", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::DivideValueRegister { to, lhs, rhs } => {
                print_binary_instruction!(
                    "divide", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::DivideValueValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "divide", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::Remainder { to, lhs, rhs } => {
                print_binary_instruction!(
                    "remainder", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::RemainderRegisterValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "remainder", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::RemainderValueRegister { to, lhs, rhs } => {
                print_binary_instruction!(
                    "remainder", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::RemainderValueValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "remainder", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::Add { to, lhs, rhs } => {
                print_binary_instruction!(
                    "add", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::AddRegisterValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "add", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::AddValueRegister { to, lhs, rhs } => {
                print_binary_instruction!(
                    "add", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::AddValueValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "add", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::Subtract { to, lhs, rhs } => {
                print_binary_instruction!(
                    "subtract", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::SubtractRegisterValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "subtract", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::SubtractValueRegister { to, lhs, rhs } => {
                print_binary_instruction!(
                    "subtract", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::SubtractValueValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "subtract", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::Less { to, lhs, rhs } => {
                print_binary_instruction!(
                    "less", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::LessRegisterValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "less", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::LessValueRegister { to, lhs, rhs } => {
                print_binary_instruction!(
                    "less", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::LessValueValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "less", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::Greater { to, lhs, rhs } => {
                print_binary_instruction!(
                    "greater", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::GreaterRegisterValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "greater", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::GreaterValueRegister { to, lhs, rhs } => {
                print_binary_instruction!(
                    "greater", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::GreaterValueValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "greater", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::LessOrEqual { to, lhs, rhs } => {
                print_binary_instruction!(
                    "less_or_equal", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::LessOrEqualRegisterValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "less_or_equal", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::LessOrEqualValueRegister { to, lhs, rhs } => {
                print_binary_instruction!(
                    "less_or_equal", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::LessOrEqualValueValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "less_or_equal", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::GreaterOrEqual { to, lhs, rhs } => {
                print_binary_instruction!(
                    "greater_or_equal", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::GreaterOrEqualRegisterValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "greater_or_equal", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::GreaterOrEqualValueRegister { to, lhs, rhs } => {
                print_binary_instruction!(
                    "greater_or_equal", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::GreaterOrEqualValueValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "greater_or_equal", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::Equal { to, lhs, rhs } => {
                print_binary_instruction!(
                    "equal", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::EqualRegisterValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "equal", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::EqualValueRegister { to, lhs, rhs } => {
                print_binary_instruction!(
                    "equal", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::EqualValueValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "equal", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::NotEqual { to, lhs, rhs } => {
                print_binary_instruction!(
                    "not_equal", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::NotEqualRegisterValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "not_equal", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::NotEqualValueRegister { to, lhs, rhs } => {
                print_binary_instruction!(
                    "not_equal", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::NotEqualValueValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "not_equal", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::And { to, lhs, rhs } => {
                print_binary_instruction!(
                    "and", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::AndRegisterValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "and", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::AndValueRegister { to, lhs, rhs } => {
                print_binary_instruction!(
                    "and", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::AndValueValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "and", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::Or { to, lhs, rhs } => {
                print_binary_instruction!(
                    "or", r, r ;
                    to, lhs, rhs,
                );
            }
            Instruction::OrRegisterValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "or", r, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::OrValueRegister { to, lhs, rhs } => {
                print_binary_instruction!(
                    "or", v, r ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::OrValueValue { to, lhs, rhs } => {
                print_binary_instruction!(
                    "or", v, v ;
                    disassembler ;
                    to, lhs, rhs,
                );
            }
            Instruction::Assign { to, operand } => {
                print!("    assign ");

                print_register(*to);

                print!(", ");

                print_register(*operand);

                println!();
            }
            Instruction::AssignValue { to, operand } => {
                print!("    assign ");

                print_register(*to);

                print!(", ");

                print_value(&disassembler, operand);

                println!();
            }
            Instruction::Push { operand } => {
                print!("    push ");

                print_register(*operand);

                println!();
            }
            Instruction::PushValue { operand } => {
                print!("    push ");

                print_value(&disassembler, operand);

                println!();
            }
            Instruction::Access { to, index, of } => {
                print!("    access ");

                print_register(*to);

                print!(", ");

                print_register(*of);

                println!("[{index}]");
            }
            Instruction::AccessValue { to, index, of } => {
                print!("    access ");

                print_register(*to);

                print!(", ");

                print_value(&disassembler, of);

                println!("[{index}]");
            }
            Instruction::AccessAssign { to, index, value } => {
                print!("    access_assign ");

                print_register(*to);

                print!("[{index}], ");

                print_register(*value);

                println!();
            }
            Instruction::AccessAssignRegisterValue { to, index, value } => {
                print!("    access_assign ");

                print_register(*to);

                print!("[{index}], ");

                print_value(&disassembler, value);

                println!();
            }
            Instruction::AccessAssignValueRegister { to, index, value } => {
                print!("    access_assign ");

                print_value(&disassembler, to);

                print!("[{index}], ");

                print_register(*value);

                println!();
            }
            Instruction::AccessAssignValueValue { to, index, value } => {
                print!("    access_assign ");

                print_value(&disassembler, to);

                print!("[{index}], ");

                print_value(&disassembler, value);

                println!();
            }
            Instruction::GetTag { to, of } => {
                print!("    get_tag ");

                print_register(*to);

                print!(", ");

                print_register(*of);

                println!();
            }
            Instruction::GetTagValue { to, of } => {
                print!("    get_tag ");

                print_register(*to);

                print!(", ");

                print_value(&disassembler, of);

                println!();
            }
            Instruction::Call { to, arity, callee } => {
                print!("    call ");

                print_register(*to);

                print!(", ");

                print_register(*callee);

                println!("({arity})");
            }
            Instruction::CallValue { to, arity, callee } => {
                print!("    call ");

                print_register(*to);

                print!(", ");

                print_value(&disassembler, callee);

                println!("({arity})");
            }
            Instruction::Jump { destination } => {
                println!("    jump label{destination}");
            }
            Instruction::Branch {
                condition,
                when_true,
                otherwise,
            } => {
                print!("    branch ");

                print_register(*condition);

                println!(", label{when_true}, label{otherwise}");
            }
            Instruction::BranchValue {
                condition,
                when_true,
                otherwise,
            } => {
                print!("    branch ");

                print_value(&disassembler, condition);

                println!(", label{when_true}, label{otherwise}");
            }
            Instruction::Return { value } => {
                print!("    return ");

                print_register(*value);

                println!();
            }
            Instruction::ReturnValue { value } => {
                print!("    return ");

                print_value(&disassembler, value);

                println!();
            }
            Instruction::Panic => {
                println!("    panic");
            }
            Instruction::Label => {}
        }
    }

    ExitCode::SUCCESS
}

fn find_labels(labels: &mut [bool], disassembler: &Disassembler, value: &Value) {
    match value {
        Value::Never
        | Value::U8 { .. }
        | Value::I8 { .. }
        | Value::U16 { .. }
        | Value::I16 { .. }
        | Value::U32 { .. }
        | Value::I32 { .. }
        | Value::U64 { .. }
        | Value::I64 { .. }
        | Value::F32 { .. }
        | Value::F64 { .. }
        | Value::Boolean { .. }
        | Value::Unit
        | Value::ApartNativeFn { .. }
        | Value::ApartRegister { .. } => {}
        Value::ApartFn { address, .. } => {
            labels[usize::try_from(*address)
                .unwrap_or_else(|_| panic!("function address was not a valid usize"))] = true;
        }
        Value::ValueIndex { index: value_index } => {
            match disassembler.arena.get(usize::from(*value_index)) {
                Some(
                    GarbageCollectedValue::Compound { fields, .. }
                    | GarbageCollectedValue::TaggedCompound { fields, .. },
                ) => {
                    for field in fields {
                        find_labels(labels, disassembler, field);
                    }
                }
                Some(GarbageCollectedValue::ApartVec { values, .. }) => {
                    for value in values {
                        find_labels(labels, disassembler, value);
                    }
                }
                Some(GarbageCollectedValue::ApartString { .. }) | None => {}
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
fn find_labels_in_instruction(
    labels: &mut [bool],
    disassembler: &Disassembler,
    instruction: &Instruction,
) {
    match instruction {
        Instruction::NoOp
        | Instruction::Panic
        | Instruction::Label
        | Instruction::ApartFnLabel { .. }
        | Instruction::Not { .. }
        | Instruction::Negate { .. }
        | Instruction::Multiply { .. }
        | Instruction::Divide { .. }
        | Instruction::Remainder { .. }
        | Instruction::Add { .. }
        | Instruction::Subtract { .. }
        | Instruction::Less { .. }
        | Instruction::Greater { .. }
        | Instruction::LessOrEqual { .. }
        | Instruction::GreaterOrEqual { .. }
        | Instruction::Equal { .. }
        | Instruction::NotEqual { .. }
        | Instruction::And { .. }
        | Instruction::Or { .. }
        | Instruction::Assign { .. }
        | Instruction::Push { .. }
        | Instruction::Access { .. }
        | Instruction::AccessAssign { .. }
        | Instruction::Call { .. }
        | Instruction::GetTag { .. }
        | Instruction::Return { .. } => {}
        Instruction::NotValue { operand: value, .. }
        | Instruction::NegateValue { operand: value, .. }
        | Instruction::MultiplyRegisterValue { rhs: value, .. }
        | Instruction::MultiplyValueRegister { lhs: value, .. }
        | Instruction::DivideRegisterValue { rhs: value, .. }
        | Instruction::DivideValueRegister { lhs: value, .. }
        | Instruction::RemainderRegisterValue { rhs: value, .. }
        | Instruction::RemainderValueRegister { lhs: value, .. }
        | Instruction::AddRegisterValue { rhs: value, .. }
        | Instruction::AddValueRegister { lhs: value, .. }
        | Instruction::SubtractRegisterValue { rhs: value, .. }
        | Instruction::SubtractValueRegister { lhs: value, .. }
        | Instruction::LessRegisterValue { rhs: value, .. }
        | Instruction::LessValueRegister { lhs: value, .. }
        | Instruction::GreaterRegisterValue { rhs: value, .. }
        | Instruction::GreaterValueRegister { lhs: value, .. }
        | Instruction::LessOrEqualRegisterValue { rhs: value, .. }
        | Instruction::LessOrEqualValueRegister { lhs: value, .. }
        | Instruction::GreaterOrEqualRegisterValue { rhs: value, .. }
        | Instruction::GreaterOrEqualValueRegister { lhs: value, .. }
        | Instruction::EqualRegisterValue { rhs: value, .. }
        | Instruction::EqualValueRegister { lhs: value, .. }
        | Instruction::NotEqualRegisterValue { rhs: value, .. }
        | Instruction::NotEqualValueRegister { lhs: value, .. }
        | Instruction::AndRegisterValue { rhs: value, .. }
        | Instruction::AndValueRegister { lhs: value, .. }
        | Instruction::OrRegisterValue { rhs: value, .. }
        | Instruction::OrValueRegister { lhs: value, .. }
        | Instruction::AssignValue { operand: value, .. }
        | Instruction::PushValue { operand: value }
        | Instruction::AccessValue { of: value, .. }
        | Instruction::AccessAssignRegisterValue { value, .. }
        | Instruction::AccessAssignValueRegister { to: value, .. }
        | Instruction::CallValue { callee: value, .. }
        | Instruction::GetTagValue { of: value, .. }
        | Instruction::ReturnValue { value } => {
            find_labels(labels, disassembler, value);
        }
        Instruction::MultiplyValueValue { lhs, rhs, .. }
        | Instruction::DivideValueValue { lhs, rhs, .. }
        | Instruction::RemainderValueValue { lhs, rhs, .. }
        | Instruction::AddValueValue { lhs, rhs, .. }
        | Instruction::SubtractValueValue { lhs, rhs, .. }
        | Instruction::LessValueValue { lhs, rhs, .. }
        | Instruction::GreaterValueValue { lhs, rhs, .. }
        | Instruction::LessOrEqualValueValue { lhs, rhs, .. }
        | Instruction::GreaterOrEqualValueValue { lhs, rhs, .. }
        | Instruction::EqualValueValue { lhs, rhs, .. }
        | Instruction::NotEqualValueValue { lhs, rhs, .. }
        | Instruction::AndValueValue { lhs, rhs, .. }
        | Instruction::OrValueValue { lhs, rhs, .. }
        | Instruction::AccessAssignValueValue {
            to: lhs,
            value: rhs,
            ..
        } => {
            find_labels(labels, disassembler, lhs);
            find_labels(labels, disassembler, rhs);
        }
        Instruction::Jump { destination } => {
            labels[usize::try_from(*destination)
                .unwrap_or_else(|_| panic!("function address was not a valid usize"))] = true;
        }
        Instruction::Branch {
            when_true,
            otherwise,
            ..
        } => {
            labels[usize::try_from(*when_true)
                .unwrap_or_else(|_| panic!("function address was not a valid usize"))] = true;

            labels[usize::try_from(*otherwise)
                .unwrap_or_else(|_| panic!("function address was not a valid usize"))] = true;
        }
        Instruction::BranchValue {
            condition,
            when_true,
            otherwise,
        } => {
            find_labels(labels, disassembler, condition);

            labels[usize::try_from(*when_true)
                .unwrap_or_else(|_| panic!("function address was not a valid usize"))] = true;

            labels[usize::try_from(*otherwise)
                .unwrap_or_else(|_| panic!("function address was not a valid usize"))] = true;
        }
    }
}
