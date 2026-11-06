use apart::{
    NativeFn, Register,
    targets::vm::{CopyableValue, Instruction, Instructive, Value, ValueIndex},
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
    values: Vec<Value>,
}

impl Instructive for Disassembler {
    fn ip_mut(&mut self) -> &mut usize {
        &mut self.ip
    }

    fn constants_mut(&mut self) -> &mut Vec<ValueIndex> {
        &mut self.constants
    }

    fn values_mut(&mut self) -> &mut Vec<Value> {
        &mut self.values
    }
}

fn print_register(register: Register) {
    print!("r{}", u16::from(register));
}

fn print_value(disassembler: &Disassembler, value: CopyableValue) {
    match value {
        CopyableValue::Never => print!("#never"),
        CopyableValue::U8(value) => print!("{value}u8"),
        CopyableValue::I8(value) => print!("{value}i8"),
        CopyableValue::U16(value) => print!("{value}u16"),
        CopyableValue::I16(value) => print!("{value}i16"),
        CopyableValue::U32(value) => print!("{value}u32"),
        CopyableValue::I32(value) => print!("{value}i32"),
        CopyableValue::U64(value) => print!("{value}u64"),
        CopyableValue::I64(value) => print!("{value}i64"),
        CopyableValue::F32(value) => print!("{value:?}f32"),
        CopyableValue::F64(value) => print!("{value:?}f64"),
        CopyableValue::Boolean(value) => print!("{value}"),
        CopyableValue::Unit => print!("{{}}"),
        CopyableValue::Fn {
            address,
            max_register_count,
        } => print!("label{address}({max_register_count})"),
        CopyableValue::NativeFn(native_fn) => match native_fn {
            NativeFn::PrintU8 => print!("print_u8"),
            NativeFn::PrintI8 => print!("print_i8"),
            NativeFn::PrintU16 => print!("print_u16"),
            NativeFn::PrintI16 => print!("print_i16"),
            NativeFn::PrintU32 => print!("print_u32"),
            NativeFn::PrintI32 => print!("print_i32"),
            NativeFn::PrintU64 => print!("print_u64"),
            NativeFn::PrintI64 => print!("print_i64"),
            NativeFn::PrintF32 => print!("print_f32"),
            NativeFn::PrintF64 => print!("print_f64"),
            NativeFn::PrintBool => print!("print_bool"),
            NativeFn::PrintUnit => print!("print_unit"),
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
        CopyableValue::Register(index) => print!("r{index}"),
        CopyableValue::ValueIndex(value_index) => {
            match disassembler
                .values
                .get(usize::from(value_index))
                .expect("the value will exist")
            {
                Value::String(text) => print!("\"{text}\""),
                Value::Compound(values) => {
                    print!("Compound([ ");

                    for value in values {
                        print_value(disassembler, *value);
                        print!(", ");
                    }

                    print!("])");
                }
                Value::TaggedCompound {
                    tag,
                    fields: values,
                } => {
                    print!("TaggedCompound {{ tag: {tag}, fields: [ ");

                    for value in values {
                        print_value(disassembler, *value);
                        print!(", ");
                    }

                    print!("] }}");
                }
                Value::Vec(values) => {
                    print!("Vec([ ");

                    for value in values {
                        print_value(disassembler, *value);
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

        $crate::print_value(&$disassembler, *$rhs);

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

        $crate::print_value(&$disassembler, *$lhs);

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

        $crate::print_value(&$disassembler, *$lhs);

        print!(", ");

        $crate::print_value(&$disassembler, *$rhs);

        println!();
    }};
}

#[allow(clippy::too_many_lines)]
fn disassemble(bytecode: &[u8]) -> ExitCode {
    let mut disassembler = Disassembler {
        ip: 0,
        constants: vec![],
        values: vec![],
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
            Instruction::FnLabel { fn_label } => {
                println!(
                    "    ; {} in {}",
                    fn_label.file_name(),
                    fn_label.function_label()
                );
            }
            Instruction::NotRegister { to, operand } => {
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

                print_value(&disassembler, *operand);

                println!();
            }
            Instruction::NegateRegister { to, operand } => {
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

                print_value(&disassembler, *operand);

                println!();
            }
            Instruction::MultiplyRegisterRegister { to, lhs, rhs } => {
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
            Instruction::DivideRegisterRegister { to, lhs, rhs } => {
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
            Instruction::RemainderRegisterRegister { to, lhs, rhs } => {
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
            Instruction::AddRegisterRegister { to, lhs, rhs } => {
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
            Instruction::SubtractRegisterRegister { to, lhs, rhs } => {
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
            Instruction::LessRegisterRegister { to, lhs, rhs } => {
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
            Instruction::GreaterRegisterRegister { to, lhs, rhs } => {
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
            Instruction::LessOrEqualRegisterRegister { to, lhs, rhs } => {
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
            Instruction::GreaterOrEqualRegisterRegister { to, lhs, rhs } => {
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
            Instruction::EqualRegisterRegister { to, lhs, rhs } => {
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
            Instruction::NotEqualRegisterRegister { to, lhs, rhs } => {
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
            Instruction::AndRegisterRegister { to, lhs, rhs } => {
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
            Instruction::OrRegisterRegister { to, lhs, rhs } => {
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
            Instruction::AssignRegister { to, operand } => {
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

                print_value(&disassembler, *operand);

                println!();
            }
            Instruction::PushRegister { operand } => {
                print!("    push ");

                print_register(*operand);

                println!();
            }
            Instruction::PushValue { operand } => {
                print!("    push ");

                print_value(&disassembler, *operand);

                println!();
            }
            Instruction::AccessRegister { to, index, of } => {
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

                print_value(&disassembler, *of);

                println!("[{index}]");
            }
            Instruction::AccessAssignRegisterRegister { to, index, value } => {
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

                print_value(&disassembler, *value);

                println!();
            }
            Instruction::AccessAssignValueRegister { to, index, value } => {
                print!("    access_assign ");

                print_value(&disassembler, *to);

                print!("[{index}], ");

                print_register(*value);

                println!();
            }
            Instruction::AccessAssignValueValue { to, index, value } => {
                print!("    access_assign ");

                print_value(&disassembler, *to);

                print!("[{index}], ");

                print_value(&disassembler, *value);

                println!();
            }
            Instruction::GetTagRegister { to, of } => {
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

                print_value(&disassembler, *of);

                println!();
            }
            Instruction::CallRegister { to, arity, callee } => {
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

                print_value(&disassembler, *callee);

                println!("({arity})");
            }
            Instruction::Jump { destination } => {
                println!("    jump label{destination}");
            }
            Instruction::BranchRegister {
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

                print_value(&disassembler, *condition);

                println!(", label{when_true}, label{otherwise}");
            }
            Instruction::ReturnRegister { value } => {
                print!("    return ");

                print_register(*value);

                println!();
            }
            Instruction::ReturnValue { value } => {
                print!("    return ");

                print_value(&disassembler, *value);

                println!();
            }
            Instruction::Panic => {
                println!("    panic");
            }
            Instruction::Label
            | Instruction::I64
            | Instruction::F64
            | Instruction::Boolean
            | Instruction::Unit
            | Instruction::Fn
            | Instruction::NativeFn
            | Instruction::Compound
            | Instruction::TaggedCompound
            | Instruction::U8
            | Instruction::I8
            | Instruction::U16
            | Instruction::I16
            | Instruction::U32
            | Instruction::I32
            | Instruction::U64
            | Instruction::String
            | Instruction::Vec
            | Instruction::F32
            | Instruction::Never
            | Instruction::Register => {}
        }
    }

    ExitCode::SUCCESS
}

fn find_labels(labels: &mut [bool], disassembler: &Disassembler, value: CopyableValue) {
    match value {
        CopyableValue::Never
        | CopyableValue::U8(_)
        | CopyableValue::I8(_)
        | CopyableValue::U16(_)
        | CopyableValue::I16(_)
        | CopyableValue::U32(_)
        | CopyableValue::I32(_)
        | CopyableValue::U64(_)
        | CopyableValue::I64(_)
        | CopyableValue::F32(_)
        | CopyableValue::F64(_)
        | CopyableValue::Boolean(_)
        | CopyableValue::Unit
        | CopyableValue::NativeFn(_)
        | CopyableValue::Register(_) => {}
        CopyableValue::Fn { address, .. } => {
            labels[usize::try_from(address)
                .unwrap_or_else(|_| panic!("function address was not a valid usize"))] = true;
        }
        CopyableValue::ValueIndex(value_index) => {
            match disassembler.values.get(usize::from(value_index)) {
                Some(
                    Value::Compound(values)
                    | Value::TaggedCompound { fields: values, .. }
                    | Value::Vec(values),
                ) => {
                    for value in values {
                        find_labels(labels, disassembler, *value);
                    }
                }
                Some(Value::String(_)) | None => {}
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
        | Instruction::FnLabel { .. }
        | Instruction::NotRegister { .. }
        | Instruction::NegateRegister { .. }
        | Instruction::MultiplyRegisterRegister { .. }
        | Instruction::DivideRegisterRegister { .. }
        | Instruction::RemainderRegisterRegister { .. }
        | Instruction::AddRegisterRegister { .. }
        | Instruction::SubtractRegisterRegister { .. }
        | Instruction::LessRegisterRegister { .. }
        | Instruction::GreaterRegisterRegister { .. }
        | Instruction::LessOrEqualRegisterRegister { .. }
        | Instruction::GreaterOrEqualRegisterRegister { .. }
        | Instruction::EqualRegisterRegister { .. }
        | Instruction::NotEqualRegisterRegister { .. }
        | Instruction::AndRegisterRegister { .. }
        | Instruction::OrRegisterRegister { .. }
        | Instruction::AssignRegister { .. }
        | Instruction::PushRegister { .. }
        | Instruction::AccessRegister { .. }
        | Instruction::AccessAssignRegisterRegister { .. }
        | Instruction::CallRegister { .. }
        | Instruction::GetTagRegister { .. }
        | Instruction::ReturnRegister { .. }
        | Instruction::I64
        | Instruction::F64
        | Instruction::Boolean
        | Instruction::Unit
        | Instruction::Fn
        | Instruction::NativeFn
        | Instruction::Compound
        | Instruction::TaggedCompound
        | Instruction::U8
        | Instruction::I8
        | Instruction::U16
        | Instruction::I16
        | Instruction::U32
        | Instruction::I32
        | Instruction::U64
        | Instruction::String
        | Instruction::F32
        | Instruction::Never
        | Instruction::Vec
        | Instruction::Register => {}
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
            find_labels(labels, disassembler, *value);
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
            find_labels(labels, disassembler, *lhs);
            find_labels(labels, disassembler, *rhs);
        }
        Instruction::Jump { destination } => {
            labels[usize::try_from(*destination)
                .unwrap_or_else(|_| panic!("function address was not a valid usize"))] = true;
        }
        Instruction::BranchRegister {
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
            find_labels(labels, disassembler, *condition);

            labels[usize::try_from(*when_true)
                .unwrap_or_else(|_| panic!("function address was not a valid usize"))] = true;

            labels[usize::try_from(*otherwise)
                .unwrap_or_else(|_| panic!("function address was not a valid usize"))] = true;
        }
    }
}
