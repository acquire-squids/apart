mod allocate_registers;
mod basic_blocks;
mod expand_macros;
mod lex;
mod low_ir;
mod name_resolve;
mod optimize;
mod parse;
mod ssa;
pub mod targets;
mod token_tree;
mod type_check;
pub mod vm;

pub use {
    allocate_registers::Error as AllocateRegistersError,
    expand_macros::Error as MacroExpansionError,
    low_ir::{
        BinaryOp, Block, Instruction as IrInstruction, Ir, Location, Register, UnaryOp,
        Value as IrValue, ValueOrLocation,
    },
    name_resolve::Error as NameResolveError,
    parse::Error as ParseError,
    token_tree::Error as TokenTreeError,
    type_check::Error as TypeCheckError,
};

use reporting::{Reportable, Span, Spanned};

use std::{error, fmt};

const CORE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/lang/core.txt");

const CORE_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/core.txt"));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    Vm,
}

#[derive(Debug, Clone)]
pub struct ApartOptions {
    pub optimize: bool,
    pub strip_debug_info: bool,
    pub target: Target,
}

pub struct Compiled<'a, T> {
    sources_with_core: Vec<(usize, (&'a str, &'a str))>,
    result: T,
}

impl<'a, T> Compiled<'a, T> {
    #[allow(dead_code)]
    #[must_use]
    pub const fn sources(&self) -> &[(usize, (&'a str, &'a str))] {
        self.sources_with_core.as_slice()
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn result(&self) -> &T {
        &self.result
    }
}

#[derive(Debug)]
pub enum Error {
    TokenTree(TokenTreeError),
    Parse(ParseError),
    ExpandMacros(MacroExpansionError),
    NameResolve(NameResolveError),
    TypeCheck(TypeCheckError),
    AllocateRegisters(AllocateRegistersError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TokenTree(error) => write!(f, "{error}"),
            Self::Parse(error) => write!(f, "{error}"),
            Self::ExpandMacros(error) => write!(f, "{error}"),
            Self::NameResolve(error) => write!(f, "{error}"),
            Self::TypeCheck(error) => write!(f, "{error}"),
            Self::AllocateRegisters(error) => write!(f, "{error}"),
        }
    }
}

impl error::Error for Error {}

impl Reportable for Error {}

/// # Errors
/// Will error if compilation fails, returning the errors for the relevant stage
#[allow(clippy::missing_panics_doc, clippy::too_many_lines)]
pub fn compile<'a>(
    options: &ApartOptions,
    sources: &[(usize, (&'a str, &'a str))],
) -> Result<Compiled<'a, Ir<IrValue>>, Compiled<'a, Vec<Spanned<Error>>>> {
    let mut source_ids = sources
        .iter()
        .map(|(source_id, _)| *source_id)
        .collect::<Vec<_>>();

    source_ids.sort_unstable();

    let core_source_id = source_ids
        .last()
        .copied()
        .unwrap_or(0)
        .checked_add(1)
        .map_or_else(
            || source_ids.first().copied().unwrap_or(1).checked_sub(1),
            Some,
        )
        .map_or_else(
            || {
                source_ids
                    .iter()
                    .rev()
                    .filter_map(|source_id| {
                        source_id
                            .checked_add(1)
                            .map_or_else(|| source_id.checked_sub(1), Some)
                    })
                    .find(|source_id| {
                        !source_ids
                            .iter()
                            .rev()
                            .any(|existing_source_id| source_id == existing_source_id)
                    })
            },
            Some,
        )
        .expect("there wasn't a free source id");

    let mut sources_with_core = vec![(core_source_id, (CORE_PATH, CORE_SOURCE))];

    for (source_id, (file_name, source)) in sources {
        sources_with_core.push((*source_id, (file_name, source)));
    }

    let mut ast = parse::Ast::new(core_source_id);

    let mut errors = vec![];

    for (source_id, (_, source)) in sources {
        let mut lexer = lex::Lexer::new(*source_id);

        lexer.push_source(source);

        let token_trees = match token_tree::tokens_to_token_trees(&mut lexer) {
            Ok(token_trees) => token_trees,
            Err(token_tree_errors) => {
                for token_tree_error in token_tree_errors {
                    errors.push(token_tree_error.transmute(Error::TokenTree));
                }

                continue;
            }
        };

        match parse::parse(&token_trees, (source, *source_id), &mut ast) {
            Ok(()) => {}
            Err(parse_errors) => {
                for parse_error in parse_errors {
                    errors.push(parse_error.transmute(Error::Parse));
                }
            }
        }
    }

    if !errors.is_empty() {
        return Err(Compiled {
            sources_with_core: sources_with_core.clone(),
            result: errors,
        });
    }

    expand_macros::expand(options, &mut ast).map_err(|errors| Compiled {
        sources_with_core: sources_with_core.clone(),
        result: errors
            .into_iter()
            .map(|error| error.transmute(Error::ExpandMacros))
            .collect::<Vec<_>>(),
    })?;

    let names = name_resolve::resolve_names(options, &ast).map_err(|errors| Compiled {
        sources_with_core: sources_with_core.clone(),
        result: errors
            .into_iter()
            .map(|error| error.transmute(Error::NameResolve))
            .collect::<Vec<_>>(),
    })?;

    let types = type_check::check_types(options, sources_with_core.as_slice(), &ast, &names)
        .map_err(|errors| Compiled {
            sources_with_core: sources_with_core.clone(),
            result: errors
                .into_iter()
                .map(|error| error.transmute(Error::TypeCheck))
                .collect::<Vec<_>>(),
        })?;

    let basic_blocks = basic_blocks::translate(
        options,
        sources_with_core.as_slice(),
        (&ast, &names, &types),
    );

    if cfg!(feature = "print_blocks") {
        print!("{basic_blocks}");
    }

    let mut ssa = ssa::convert(options, &basic_blocks);

    if cfg!(feature = "print_ssa") {
        print!("{ssa}");
    }

    if options.optimize {
        optimize::optimize(options, &mut ssa);

        if cfg!(feature = "print_optimized") {
            print!("{ssa}");
        }
    }

    if let Err(errors) = allocate_registers::allocate(options, &mut ssa) {
        // TODO: it'd be nice to have a span for the register allocation error somehow
        let span = sources_with_core.get(1).map_or_else(
            || {
                Span::new(
                    *sources_with_core
                        .first()
                        .map(|(source_id, (_, _))| source_id)
                        .expect("there is always at least the core"),
                    0,
                    0,
                )
            },
            |(source_id, (_, _))| Span::new(*source_id, 0, 0),
        );

        return Err(Compiled {
            sources_with_core,
            result: errors
                .into_iter()
                .map(|error| Spanned::new(Error::AllocateRegisters(error), span))
                .collect::<Vec<_>>(),
        });
    }

    if cfg!(feature = "print_allocated") {
        print!("{ssa}");
    }

    let compiled = Compiled {
        sources_with_core,
        result: ssa,
    };

    let low_ir = low_ir::lower(options, &compiled);

    if cfg!(feature = "print_lowir") {
        print!("{low_ir}");
    }

    Ok(Compiled {
        sources_with_core: compiled.sources().to_vec(),
        result: low_ir,
    })
}

#[macro_export]
macro_rules! __test_vm_output_single_function {
    (
        $source:ident, $test_file_name:literal, $expected_output:expr ;
        $test_name:ident, $optimized:literal $(,)?
    ) => {
        #[test]
        fn $test_name() {
            let mut out = vec![];

            let result = $crate::compile(
                &$crate::ApartOptions {
                    optimize: $optimized,
                    strip_debug_info: true,
                    target: $crate::Target::Vm,
                },
                [(0, ($test_file_name, $source))].as_slice(),
            )
            .map(|compiled| $crate::targets::vm::compile(&compiled));

            match result {
                Ok(compiled) => {
                    let output = $crate::vm::run(compiled.as_slice(), &mut out)
                        .map(|()| str::from_utf8(out.as_slice()).expect("only utf-8!  sorry!"))
                        .map_err(|status| {
                            (
                                str::from_utf8(out.as_slice()).expect("only utf-8!  sorry!"),
                                status,
                            )
                        });

                    assert_eq!(output, $expected_output);
                }
                Err(compiled) => {
                    let errors = compiled.result();

                    for error in errors {
                        let source_index = compiled
                            .sources()
                            .iter()
                            .position(|(id, _)| *id == error.span().source_id())
                            .expect("the source must exist");

                        let report_data = reporting::ReportData::new(
                            compiled.sources()[source_index].1.1,
                            "error",
                            $test_file_name,
                            "...",
                            reporting::ReportColors::new(),
                        );

                        let mut err = vec![];

                        let _ = report_data.report(&error, &mut err);

                        eprint!(
                            "{}",
                            str::from_utf8(err.as_slice()).expect("only utf-8!  sorry!")
                        );
                    }

                    panic!("test failed with one or more errors");
                }
            }
        }
    };
}

pub use __test_vm_output_single_function as test_vm_output_single_function;

#[macro_export]
macro_rules! __test_vm_output {
    (
        $test_name:ident, $test_file_name:literal, $expected_output:expr $(,)?
    ) => {
        #[cfg(test)]
        mod $test_name {
            const SOURCE: &str = include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/lang/tests/",
                $test_file_name
            ));

            $crate::test_vm_output_single_function!(
                SOURCE, $test_file_name, $expected_output ;
                unoptimized, false
            );

            $crate::test_vm_output_single_function!(
                SOURCE, $test_file_name, $expected_output ;
                optimized, true
            );
        }
    };
}

pub use __test_vm_output as test_vm_output;

#[macro_export]
macro_rules! __test_compilation_errors {
    (
        $test_name:ident, $test_file_name:literal, $expected_errors:pat $(if $expected_errors_conditional:expr)? $(,)?
    ) => {
        #[cfg(test)]
        mod $test_name {
            const SOURCE: &str = include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/lang/tests/",
                $test_file_name
            ));

            #[test]
            fn compilation_error() {
                let result = $crate::compile(
                    &$crate::ApartOptions {
                        optimize: false,
                        strip_debug_info: true,
                        target: $crate::Target::Vm,
                    },
                    [(0, ($test_file_name, SOURCE))].as_slice()
                )
                .map(|_| "ERRONEOUS SUCCESSFUL COMPILATION");

                let result = result.as_ref()
                    .map_err($crate::Compiled::result)
                    .map_err(|errors| {
                        errors.iter()
                            .map(reporting::Spanned::kind)
                            .collect::<Vec<_>>()
                    });

                let result = result.as_ref()
                    .map_err(|errors| errors.as_slice());

                std::assert_matches!(
                    result,
                    Err($expected_errors) $(if $expected_errors_conditional)?
                );
            }
        }
    }
}

pub use __test_compilation_errors as test_compilation_errors;

#[macro_export]
macro_rules! __int_enum {
    (
        $v:vis $name:ident as $int:ty ;
        $($variant:ident => $value:literal,)+
    ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $v enum $name {
            $($variant,)+
        }

        impl From<$name> for $int {
            fn from(value: $name) -> Self {
                match value {
                    $($name::$variant => $value,)+
                }
            }
        }

        impl TryFrom<$int> for $name {
            type Error = $int;

            fn try_from(value: $int) -> Result<Self, Self::Error> {
                match value {
                    $($value => Ok(Self::$variant),)+
                    _ => Err(value),
                }
            }
        }
    };
}

pub use __int_enum as int_enum;
