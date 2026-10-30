use crate::{
    ApartOptions, Compiled,
    basic_blocks::BlockIndex,
    low_ir::{
        BinaryOp, Block, Instruction as IrInstruction, Ir, Location, NativeFn, Register, UnaryOp,
        Value as IrValue, ValueOrLocation,
    },
};

use std::{collections::HashSet, path::PathBuf};

#[allow(clippy::too_many_lines)]
#[must_use]
pub fn compile(
    options: &ApartOptions,
    compiled: &Compiled<'_, Ir<IrValue>>,
) -> Vec<(PathBuf, String)> {
    let ssa = compiled.result();

    let mut output = vec![];

    let block_data = ssa.blocks().iter().flat_map(Block::instructions).fold(
        {
            let mut block_data = HashSet::new();

            block_data.insert(BlockIndex(0));

            block_data
        },
        |mut accum, instruction| {
            match instruction {
                IrInstruction::FnLabel(_)
                | IrInstruction::NoOp
                | IrInstruction::Panic
                | IrInstruction::PopN(_)
                | IrInstruction::Jump(_) => {}
                IrInstruction::Unary { operand: value, .. }
                | IrInstruction::Assign { value, .. }
                | IrInstruction::Push(value)
                | IrInstruction::Access { of: value, .. }
                | IrInstruction::Call { callee: value, .. }
                | IrInstruction::GetTag { of: value, .. }
                | IrInstruction::Branch {
                    condition: value, ..
                }
                | IrInstruction::Return(value) => {
                    find_functions(&mut accum, value);
                }
                IrInstruction::Binary { lhs, rhs, .. }
                | IrInstruction::AccessAssign {
                    of: lhs,
                    value: rhs,
                    ..
                } => {
                    find_functions(&mut accum, lhs);
                    find_functions(&mut accum, rhs);
                }
            }

            accum
        },
    );

    let mangled_prefix = if options.strip_debug_info {
        let mut mangled_prefix = "__apart".to_string();
        let mut mangled_prefix_changed = true;

        while mangled_prefix_changed {
            mangled_prefix_changed = false;

            for block in ssa.blocks() {
                if let Some(IrInstruction::FnLabel(function_label)) = block.instructions().first()
                    && function_label
                        .function_label()
                        .contains(mangled_prefix.as_str())
                {
                    mangled_prefix.push('_');

                    mangled_prefix_changed = true;
                }
            }
        }

        mangled_prefix
    } else {
        String::new()
    };

    let mangled_prefix = mangled_prefix.as_str();

    let mut function_blocks = block_data.iter().copied().collect::<Vec<_>>();

    function_blocks.sort();

    let mut native_functions_seen = HashSet::new();

    let mut seen_eq = false;

    // TODO: split into multiple files
    let mut output_source = String::new();

    for function_block in function_blocks {
        output_source.push_str(
            compile_block(
                options,
                ssa,
                mangled_prefix,
                &block_data,
                u32::from(function_block),
                (
                    &mut HashSet::new(),
                    &mut native_functions_seen,
                    &mut HashSet::new(),
                    &mut seen_eq,
                ),
            )
            .as_str(),
        );

        output_source.push(' ');
    }

    let mut native_functions = native_functions_seen.iter().copied().collect::<Vec<_>>();

    native_functions.sort();

    for native_function in native_functions {
        let native_function = match native_function {
            NativeFn::PrintI64
            | NativeFn::PrintF64
            | NativeFn::PrintBool
            | NativeFn::PrintUnit
            | NativeFn::PrintU8
            | NativeFn::PrintI8
            | NativeFn::PrintU16
            | NativeFn::PrintI16
            | NativeFn::PrintU32
            | NativeFn::PrintI32
            | NativeFn::PrintU64
            | NativeFn::PrintF32 => String::new(),
            NativeFn::PrintString => format!(
                "function {mangled_prefix}_print_string(_text) {{ trace(_text[0]); return \"{{}}\"; }}"
            ),
            NativeFn::ExtendString => format!(
                "function {mangled_prefix}_extend_string(_a, _b) {{ _a[0] += _b[0]; return \"{{}}\"; }}"
            ),
            NativeFn::SliceString => format!(
                "function {mangled_prefix}_slice_string(_text, _start, _end) {{ var _length = string_length(_text[0]); return (_start >= 0 && _end >= 0 && _start <= _length && _end <= _length ? {{ tag: {OPTION_SOME}, a0: [string_copy(_text[0], _start + 1, (_end - _start))] }} : {{ tag: {OPTION_NONE} }}); }}"
            ),
            NativeFn::LengthString => format!(
                "function {mangled_prefix}_length_string(_text) {{ return string_length(_text[0]); }}"
            ),
            NativeFn::TruncateString => format!(
                "function {mangled_prefix}_truncate_string(_text, _end) {{ _text[0] = string_delete(_text[0], _end + 1, string_length(_text[0]) - (_end - 1)); return \"{{}}\"; }}"
            ),
            NativeFn::FloorCharBoundaryString => {
                format!(
                    "function {mangled_prefix}_floor_char_boundary_string(_text, _at) {{ return clamp(_at, 0, string_length(_text[0])); }}"
                )
            }
            NativeFn::CeilCharBoundaryString => {
                format!(
                    "function {mangled_prefix}_ceil_char_boundary_string(_text, _at) {{ return clamp(_at, 0, string_length(_text[0])); }}"
                )
            }
            NativeFn::Clock => format!(
                "function {mangled_prefix}_clock() {{ return get_timer_nonsync() / 1000.0; }}"
            ),
            NativeFn::NewVec => format!(
                "function {mangled_prefix}_new_vec() {{ return {{ length: 0, values: [] }}; }}"
            ),
            NativeFn::GetVec => format!(
                "function {mangled_prefix}_get_vec(_vec, _index) {{ if _index >= 0 && _index < _vec.length {{ return {{ tag: {OPTION_SOME}, a0: _vec.values[_index] }}; }} else {{ return {{ tag: {OPTION_NONE} }}; }} }}"
            ),
            NativeFn::ExtendVec => format!(
                "function {mangled_prefix}_extend_vec(_a, _b) {{ array_copy(_a.values, _a.length, _b.values, 0, _b.length); _a.length += _b.length; return \"{{}}\"; }}"
            ),
            NativeFn::SliceVec => format!(
                "function {mangled_prefix}_slice_vec(_vec, _start, _end) {{ if _start >= 0 && _end >= 0 && _start <= _vec.length && _end <= _vec.length {{ var _new = []; array_copy(_new, 0, _vec.values, 0, _vec.length); return {{ tag: {OPTION_SOME}, a0: {{ length: _end - _start, values: _new }} }}; }} else {{ return {{ tag: {OPTION_NONE} }}; }} }}"
            ),
            NativeFn::LengthVec => {
                format!("function {mangled_prefix}_length_vec(_vec) {{ return _vec.length; }}")
            }
            NativeFn::TruncateVec => format!(
                "function {mangled_prefix}_truncate_vec(_vec, _end) {{ var _new = []; array_copy(_new, 0, _vec.values, 0, _vec.length - _end); _vec.length = _end; _vec.values = _new; return \"{{}}\"; }}"
            ),
            NativeFn::PushVec => format!(
                "function {mangled_prefix}_push_vec(_vec, _value) {{ array_push(_vec.values, _value); _vec.length += 1; return \"{{}}\"; }}"
            ),
            NativeFn::PopVec => format!(
                "function {mangled_prefix}_pop_vec(_vec) {{ if _vec.length > 0 {{ var _popped = _vec.values[_vec.length - 1]; var _new = []; array_copy(_new, 0, _vec.values, 0, _vec.length); _vec.length -= 1; return {{ tag: {OPTION_SOME}, a0: _popped }} }} else {{ return {{ tag: {OPTION_NONE} }}; }} }}"
            ),
        };

        if !native_function.is_empty() {
            output_source.push_str(native_function.as_str());

            output_source.push(' ');
        }
    }

    if seen_eq {
        output_source.push_str(eq_impl(mangled_prefix).as_str());

        output_source.push(' ');
    }

    output_source.push_str(format!("{mangled_prefix}_0();").as_str());

    output.push((
        PathBuf::from(options.output.clone())
            .file_name()
            .map_or_else(
                || PathBuf::from("main.mod.ntgml"),
                |file_name| {
                    let mut file_name = file_name.to_os_string();

                    file_name.push("_main");

                    PathBuf::from(file_name)
                        .with_added_extension("mod")
                        .with_added_extension("ntgml")
                },
            ),
        output_source,
    ));

    output
}

#[allow(dead_code)]
const OPTION_SOME: u16 = 1;

#[allow(dead_code)]
const OPTION_NONE: u16 = 0;

#[allow(dead_code)]
const RESULT_OK: u16 = 1;

#[allow(dead_code)]
const RESULT_ERR: u16 = 0;

fn is_function_hook(name: &str) -> bool {
    matches!(name, "main" | "step" | "draw")
}

fn normalize_function_name(name: &str) -> String {
    name.replace('~', "_").replace(['[', ']'], "__")
}

fn compile_location(mangled_prefix: &str, location: Location) -> String {
    match location {
        Location::Register(register) => format!("{mangled_prefix}_local{}", u16::from(register)),
    }
}

fn eq_impl(mangled_prefix: &str) -> String {
    format!(
        "function {mangled_prefix}_eq(a, b) {{ return (is_real(a) && is_real(b) && a == b) || (is_string(a) && is_string(b) && a == b) || (is_array(a) && is_array(b) && array_length(a) == array_length(b) && {mangled_prefix}_arrays_eq(a, b)) || (is_object(a) && is_object(b) && struct_names_count(a) == struct_names_count(b) && {mangled_prefix}_objects_eq(a, b)); }} function {mangled_prefix}_arrays_eq(a, b) {{ for (var i = 0, _length = array_length(a); i < _length; i ++) {{ if !{mangled_prefix}_eq(a[i], b[i]) {{ return false; }} }} return true; }} function {mangled_prefix}_objects_eq(a, b) {{ var _aFields = struct_get_names(a); var _bFields = struct_get_names(b); array_sort(_aFields, true); array_sort(_bFields, true); for (var i = 0, _length = array_length(a); i < _length; i ++) {{ if !{mangled_prefix}_eq(struct_get(a, _aFields[i]), struct_get(b, _bFields[i])) {{ return false; }} }} return true; }}"
    )
}

#[allow(clippy::too_many_lines)]
fn compile_block(
    options: &ApartOptions,
    ssa: &Ir<IrValue>,
    mangled_prefix: &str,
    block_data: &HashSet<BlockIndex>,
    b: u32,
    (seen_locations, seen_native_functions, seen_blocks, seen_eq): (
        &mut HashSet<Location>,
        &mut HashSet<NativeFn>,
        &mut HashSet<BlockIndex>,
        &mut bool,
    ),
) -> String {
    let mut buffer = String::new();

    if seen_blocks.insert(BlockIndex(b))
        && let Some(block) = ssa.get_block(BlockIndex(b))
    {
        if block_data.contains(&BlockIndex(b)) {
            buffer.push_str("function ");

            if let Some(IrInstruction::FnLabel(function_label)) = ssa
                .get_block(BlockIndex(b))
                .map(Block::instructions)
                .and_then(|instructions| instructions.first())
            {
                let normalized = normalize_function_name(function_label.function_label());

                let normalized = normalized.as_str();

                if !options.strip_debug_info || is_function_hook(normalized) {
                    buffer.push_str(normalized);
                } else {
                    buffer.push_str(format!("{mangled_prefix}_{b}").as_str());
                }
            } else {
                buffer.push_str(format!("{mangled_prefix}_{b}").as_str());
            }

            buffer.push('(');

            for i in 0..(block.call_argument_count()) {
                if i != 0 {
                    buffer.push_str(", ");
                }

                buffer.push_str(
                    compile_location(mangled_prefix, Location::Register(Register(u16::from(i))))
                        .as_str(),
                );
            }

            buffer.push(')');

            let mut locals = (0..(block.max_register_count()))
                .skip(usize::from(block.call_argument_count()))
                .map(|i| {
                    format!(
                        "{}{} = null",
                        if i > u16::from(block.call_argument_count()) {
                            ", "
                        } else {
                            ""
                        },
                        compile_location(mangled_prefix, Location::Register(Register(i)))
                    )
                })
                .fold(" var ".to_string(), |mut accum, local| {
                    accum.push_str(local.as_str());

                    accum
                });

            locals.push(';');

            buffer.push_str(format!(" {{ var {mangled_prefix}_block_index = {b};{} while true {{ switch {mangled_prefix}_block_index {{", if block.max_register_count() > u16::from(block.call_argument_count()) { locals.as_str() } else { "" }).as_str());
        }

        let mut call_arguments = vec![];

        buffer.push_str(format!(" case {b}: {{").as_str());

        let mut terminators = (None, None);

        for instruction in block.instructions() {
            match instruction {
                IrInstruction::FnLabel(_) | IrInstruction::NoOp | IrInstruction::PopN(_) => {}
                IrInstruction::Panic => {
                    buffer.push_str(" throw \"panicked!\";");
                }
                IrInstruction::Unary { to, op, operand } => {
                    buffer.push_str(
                        format!(
                            " {} = {} {}; ",
                            compile_location(mangled_prefix, *to),
                            match op {
                                UnaryOp::Not => "!",
                                UnaryOp::Negate => "-",
                            },
                            compile_value(
                                options,
                                ssa,
                                mangled_prefix,
                                operand,
                                seen_native_functions
                            )
                        )
                        .as_str(),
                    );
                }
                IrInstruction::Binary { to, op, lhs, rhs } => {
                    buffer.push_str(
                        match op {
                            BinaryOp::Equal => {
                                *seen_eq |= true;

                                format!(
                                    " {} = {mangled_prefix}_eq({}, {});",
                                    compile_location(mangled_prefix, *to),
                                    compile_value(
                                        options,
                                        ssa,
                                        mangled_prefix,
                                        lhs,
                                        seen_native_functions
                                    ),
                                    compile_value(
                                        options,
                                        ssa,
                                        mangled_prefix,
                                        rhs,
                                        seen_native_functions
                                    )
                                )
                            }
                            BinaryOp::NotEqual => {
                                *seen_eq |= true;

                                format!(
                                    " {} = !{mangled_prefix}_eq({}, {});",
                                    compile_location(mangled_prefix, *to),
                                    compile_value(
                                        options,
                                        ssa,
                                        mangled_prefix,
                                        lhs,
                                        seen_native_functions
                                    ),
                                    compile_value(
                                        options,
                                        ssa,
                                        mangled_prefix,
                                        rhs,
                                        seen_native_functions
                                    )
                                )
                            }
                            BinaryOp::Multiply
                            | BinaryOp::IntegerDivide
                            | BinaryOp::Divide
                            | BinaryOp::Remainder
                            | BinaryOp::Add
                            | BinaryOp::Subtract
                            | BinaryOp::Less
                            | BinaryOp::Greater
                            | BinaryOp::LessOrEqual
                            | BinaryOp::GreaterOrEqual
                            | BinaryOp::And
                            | BinaryOp::Or => format!(
                                " {} = {} {} {};",
                                compile_location(mangled_prefix, *to),
                                compile_value(
                                    options,
                                    ssa,
                                    mangled_prefix,
                                    lhs,
                                    seen_native_functions
                                ),
                                match op {
                                    BinaryOp::Multiply => "*",
                                    BinaryOp::IntegerDivide => "div",
                                    BinaryOp::Divide => "/",
                                    BinaryOp::Remainder => "%",
                                    BinaryOp::Add => "+",
                                    BinaryOp::Subtract => "-",
                                    BinaryOp::Less => "<",
                                    BinaryOp::Greater => ">",
                                    BinaryOp::LessOrEqual => "<=",
                                    BinaryOp::GreaterOrEqual => ">=",
                                    BinaryOp::And => "&&",
                                    BinaryOp::Or => "||",
                                    BinaryOp::Equal | BinaryOp::NotEqual =>
                                        unreachable!("the outer match would prevent this"),
                                },
                                compile_value(
                                    options,
                                    ssa,
                                    mangled_prefix,
                                    rhs,
                                    seen_native_functions
                                ),
                            ),
                        }
                        .as_str(),
                    );
                }
                IrInstruction::Assign { to, value } => {
                    buffer.push_str(
                        format!(
                            " {} = {};",
                            compile_location(mangled_prefix, *to),
                            compile_value(
                                options,
                                ssa,
                                mangled_prefix,
                                value,
                                seen_native_functions
                            )
                        )
                        .as_str(),
                    );
                }
                IrInstruction::Push(value) => {
                    call_arguments.push(compile_value(
                        options,
                        ssa,
                        mangled_prefix,
                        value,
                        seen_native_functions,
                    ));
                }
                IrInstruction::Access { to, index, of } => {
                    buffer.push_str(
                        format!(
                            " {} = ({}).a{};",
                            compile_location(mangled_prefix, *to),
                            compile_value(options, ssa, mangled_prefix, of, seen_native_functions),
                            index,
                        )
                        .as_str(),
                    );
                }
                IrInstruction::AccessAssign { of, index, value } => {
                    buffer.push_str(
                        format!(
                            "({}).a{} = {};",
                            compile_value(options, ssa, mangled_prefix, of, seen_native_functions),
                            index,
                            compile_value(
                                options,
                                ssa,
                                mangled_prefix,
                                value,
                                seen_native_functions
                            ),
                        )
                        .as_str(),
                    );
                }
                IrInstruction::Call { to, arity, callee } => {
                    buffer.push_str(
                        format!(
                            " {} = ({})({});",
                            compile_location(mangled_prefix, *to),
                            compile_value(
                                options,
                                ssa,
                                mangled_prefix,
                                callee,
                                seen_native_functions
                            ),
                            call_arguments
                                .drain((call_arguments.len() - usize::from(*arity))..)
                                .enumerate()
                                .fold(String::new(), |mut accum, (i, value)| {
                                    if i == 0 {
                                        accum.push_str(value.as_str());
                                    } else {
                                        accum.push_str(format!(", {value}").as_str());
                                    }

                                    accum
                                }),
                        )
                        .as_str(),
                    );
                }
                IrInstruction::GetTag { to, of } => {
                    buffer.push_str(
                        format!(
                            " {} = {}.tag;",
                            compile_location(mangled_prefix, *to),
                            compile_value(options, ssa, mangled_prefix, of, seen_native_functions)
                        )
                        .as_str(),
                    );
                }
                IrInstruction::Jump(destination) => {
                    buffer.push_str(
                        format!(
                            " {mangled_prefix}_block_index = {};",
                            u32::from(*destination),
                        )
                        .as_str(),
                    );

                    terminators.0 = Some(u32::from(*destination));
                }
                IrInstruction::Branch {
                    condition,
                    when_true,
                    otherwise,
                } => {
                    buffer.push_str(
                        format!(
                            " if {} {{ {mangled_prefix}_block_index = {}; }} else {{ {mangled_prefix}_block_index = {}; }}",
                            compile_value(
                                options,
                                ssa,
                                mangled_prefix,
                                condition,
                                seen_native_functions
                            ),
                            u32::from(*when_true),
                            u32::from(*otherwise),
                        )
                        .as_str(),
                    );

                    terminators = (Some(u32::from(*when_true)), Some(u32::from(*otherwise)));
                }
                IrInstruction::Return(value) => {
                    buffer.push_str(
                        format!(
                            " return {};",
                            compile_value(
                                options,
                                ssa,
                                mangled_prefix,
                                value,
                                seen_native_functions
                            )
                        )
                        .as_str(),
                    );
                }
            }
        }

        if matches!(block.instructions().last(), Some(IrInstruction::Return(_))) {
            buffer.push_str(" }");
        } else {
            buffer.push_str(" break; }");
        }

        if let Some(next_block) = terminators.0 {
            buffer.push_str(
                compile_block(
                    options,
                    ssa,
                    mangled_prefix,
                    block_data,
                    next_block,
                    (seen_locations, seen_native_functions, seen_blocks, seen_eq),
                )
                .as_str(),
            );
        }

        if let Some(next_block) = terminators.1 {
            buffer.push_str(
                compile_block(
                    options,
                    ssa,
                    mangled_prefix,
                    block_data,
                    next_block,
                    (seen_locations, seen_native_functions, seen_blocks, seen_eq),
                )
                .as_str(),
            );
        }

        if block_data.contains(&BlockIndex(b)) {
            buffer.push_str(" default: { throw \"panicked!\"; } } } }");
        }
    }

    buffer
}

#[allow(clippy::too_many_lines)]
fn compile_value(
    options: &ApartOptions,
    ssa: &Ir<IrValue>,
    mangled_prefix: &str,
    value: &ValueOrLocation<IrValue>,
    seen_native_functions: &mut HashSet<NativeFn>,
) -> String {
    match value {
        ValueOrLocation::At(location) => compile_location(mangled_prefix, *location),
        ValueOrLocation::Value(value) => match value {
            IrValue::Never => "null".to_string(),
            IrValue::U8(value) => value.to_string(),
            IrValue::I8(value) => value.to_string(),
            IrValue::U16(value) => value.to_string(),
            IrValue::I16(value) => value.to_string(),
            IrValue::U32(value) => value.to_string(),
            IrValue::I32(value) => value.to_string(),
            IrValue::U64(value) => value.to_string(),
            IrValue::I64(value) => value.to_string(),
            IrValue::F32(value) => format!("{value:?}"),
            IrValue::F64(value) => format!("{value:?}"),
            IrValue::Boolean(value) => value.to_string(),
            IrValue::Unit => "\"{}\"".to_string(),
            IrValue::String(value) => format!("[\"{value}\"]"),
            IrValue::Vec(values) => {
                let mut buffer = format!("{{ length: {}, values: [", values.len());

                if let Some(value) = values.first() {
                    buffer.push_str(
                        compile_value(options, ssa, mangled_prefix, value, seen_native_functions)
                            .as_str(),
                    );

                    for value in values.iter().skip(1) {
                        buffer.push_str(", ");

                        buffer.push_str(
                            compile_value(
                                options,
                                ssa,
                                mangled_prefix,
                                value,
                                seen_native_functions,
                            )
                            .as_str(),
                        );
                    }
                }

                buffer.push_str("] }");

                buffer
            }
            IrValue::Fn { block_index, .. } => {
                if let Some(IrInstruction::FnLabel(function_label)) =
                    usize::try_from(u32::from(*block_index))
                        .ok()
                        .and_then(|block_index| ssa.blocks().get(block_index))
                        .and_then(|block| block.instructions().first())
                {
                    let normalized = normalize_function_name(function_label.function_label());

                    if !options.strip_debug_info || is_function_hook(normalized.as_str()) {
                        normalized
                    } else {
                        format!("{mangled_prefix}_{}", u32::from(*block_index))
                    }
                } else {
                    format!("{mangled_prefix}_{}", u32::from(*block_index))
                }
            }
            IrValue::NativeFn(native_fn) => {
                seen_native_functions.insert(*native_fn);

                match native_fn {
                    NativeFn::PrintI64
                    | NativeFn::PrintF64
                    | NativeFn::PrintBool
                    | NativeFn::PrintUnit
                    | NativeFn::PrintU8
                    | NativeFn::PrintI8
                    | NativeFn::PrintU16
                    | NativeFn::PrintI16
                    | NativeFn::PrintU32
                    | NativeFn::PrintI32
                    | NativeFn::PrintU64
                    | NativeFn::PrintF32 => "trace".to_string(),
                    NativeFn::PrintString => format!("{mangled_prefix}_print_string"),
                    NativeFn::ExtendString => format!("{mangled_prefix}_extend_string"),
                    NativeFn::SliceString => format!("{mangled_prefix}_slice_string"),
                    NativeFn::LengthString => format!("{mangled_prefix}_length_string"),
                    NativeFn::TruncateString => format!("{mangled_prefix}_truncate_string"),
                    NativeFn::FloorCharBoundaryString => {
                        format!("{mangled_prefix}_floor_char_boundary_string")
                    }
                    NativeFn::CeilCharBoundaryString => {
                        format!("{mangled_prefix}_ceil_char_boundary_string")
                    }
                    NativeFn::Clock => format!("{mangled_prefix}_clock"),
                    NativeFn::NewVec => format!("{mangled_prefix}_new_vec"),
                    NativeFn::GetVec => format!("{mangled_prefix}_get_vec"),
                    NativeFn::ExtendVec => format!("{mangled_prefix}_extend_vec"),
                    NativeFn::SliceVec => format!("{mangled_prefix}_slice_vec"),
                    NativeFn::LengthVec => format!("{mangled_prefix}_length_vec"),
                    NativeFn::TruncateVec => format!("{mangled_prefix}_truncate_vec"),
                    NativeFn::PushVec => {
                        format!("{mangled_prefix}_push_vec")
                    }
                    NativeFn::PopVec => {
                        format!("{mangled_prefix}_pop_vec")
                    }
                }
            }
            IrValue::Compound(values) => {
                let mut buffer = "{ ".to_string();

                if let Some(value) = values.first() {
                    buffer.push_str("a0: ");

                    buffer.push_str(
                        compile_value(options, ssa, mangled_prefix, value, seen_native_functions)
                            .as_str(),
                    );

                    for (field_index, value) in values.iter().enumerate().skip(1) {
                        buffer.push_str(format!(", a{field_index}: ").as_str());

                        buffer.push_str(
                            compile_value(
                                options,
                                ssa,
                                mangled_prefix,
                                value,
                                seen_native_functions,
                            )
                            .as_str(),
                        );
                    }
                }

                buffer.push_str(" }");

                buffer
            }
            IrValue::TaggedCompound {
                tag,
                fields: values,
            } => {
                let mut buffer = format!("{{ tag: {tag}");

                for (field_index, value) in values.iter().enumerate() {
                    buffer.push_str(format!(", a{field_index}: ").as_str());

                    buffer.push_str(
                        compile_value(options, ssa, mangled_prefix, value, seen_native_functions)
                            .as_str(),
                    );
                }

                buffer.push_str(" }");

                buffer
            }
        },
    }
}

fn find_functions(block_data: &mut HashSet<BlockIndex>, value: &ValueOrLocation<IrValue>) {
    match value {
        ValueOrLocation::At(Location::Register(_)) => {}
        ValueOrLocation::Value(value) => match value {
            IrValue::Never
            | IrValue::U8(_)
            | IrValue::I8(_)
            | IrValue::U16(_)
            | IrValue::I16(_)
            | IrValue::U32(_)
            | IrValue::I32(_)
            | IrValue::U64(_)
            | IrValue::I64(_)
            | IrValue::F32(_)
            | IrValue::F64(_)
            | IrValue::Boolean(_)
            | IrValue::Unit
            | IrValue::String(_)
            | IrValue::NativeFn(_) => {}
            IrValue::Fn { block_index, .. } => {
                block_data.insert(*block_index);
            }
            IrValue::Compound(values)
            | IrValue::TaggedCompound { fields: values, .. }
            | IrValue::Vec(values) => {
                for value in values {
                    find_functions(block_data, value);
                }
            }
        },
    }
}
