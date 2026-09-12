mod allocate_registers;
mod basic_blocks;
mod evaluate;
mod lex;
mod name_resolve;
mod optimize;
mod parse;
mod ssa;
mod type_check;

pub use {
    basic_blocks::{Address, Instruction, Value},
    name_resolve::Error as NameResolveError,
    parse::Error as ParseError,
    ssa::{Argument, Block, BlockTerminator, JumpTo, Ssa},
    type_check::Error as TypeCheckError,
};

use reporting::{Reportable, Span, Spanned};

use std::{error, fmt, io::Write};

const CORE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/lang/core.txt");

const CORE_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/core.txt"));

pub struct Compiled<'a> {
    sources_with_core: Vec<(usize, &'a str)>,
    ssa: Ssa,
}

#[derive(Debug)]
pub enum Error {
    Parse(ParseError),
    NameResolve(NameResolveError),
    TypeCheck(TypeCheckError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "{error}"),
            Self::NameResolve(error) => write!(f, "{error}"),
            Self::TypeCheck(error) => write!(f, "{error}"),
        }
    }
}

impl error::Error for Error {}

impl Reportable for Error {}

pub fn evaluate<const MAX_REGISTERS: usize, O>(compiled: &Compiled<'_>, out: &mut O)
where
    O: Write,
{
    evaluate::run::<MAX_REGISTERS, O>(&compiled.ssa, compiled.sources_with_core.as_slice(), out);
}

/// # Errors
/// Will error if compilation fails, returning the errors for the relevant stage
#[allow(clippy::missing_panics_doc)]
pub fn compile<'a, const MAX_REGISTERS: usize>(
    sources: &[(usize, &'a str)],
) -> Result<Compiled<'a>, Vec<Spanned<Error>>> {
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

    let mut sources_with_core = vec![(core_source_id, CORE_SOURCE)];

    let mut ast = parse::Ast::new(core_source_id);

    for (source_id, source) in sources {
        let mut lexer = lex::Lexer::new(*source_id);

        lexer.push_source(source);

        parse::parse(&mut lexer, &mut ast).map_err(|errors| {
            errors
                .into_iter()
                .map(|error| error.transmute(Error::Parse))
                .collect::<Vec<_>>()
        })?;

        sources_with_core.push((*source_id, source));
    }

    let names = name_resolve::resolve_names(&ast).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.transmute(Error::NameResolve))
            .collect::<Vec<_>>()
    })?;

    let types = type_check::check_types(&ast, &names).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.transmute(Error::TypeCheck))
            .collect::<Vec<_>>()
    })?;

    let basic_blocks = basic_blocks::translate(&ast, &names, &types);

    if cfg!(feature = "print_blocks") {
        print!("{basic_blocks}");
    }

    let mut ssa = ssa::convert(&basic_blocks);

    if cfg!(feature = "print_ssa") {
        print!("{ssa}");
    }

    if cfg!(feature = "optimized") {
        optimize::optimize(&mut ssa);

        if cfg!(feature = "print_optimized") {
            print!("{ssa}");
        }
    }

    allocate_registers::allocate::<MAX_REGISTERS>(&mut ssa);

    if cfg!(feature = "print_allocated") {
        print!("{ssa}");
    }

    Ok(Compiled {
        sources_with_core,
        ssa,
    })
}

#[macro_export]
macro_rules! __test_evaluation_output {
    (
        $test_name:ident, $test_file_name:literal, $expected_output:literal $(,)?
    ) => {
        #[cfg(test)]
        mod $test_name {
            const SOURCE: &str = include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/lang/tests/",
                $test_file_name
            ));

            const NO_REGISTERS: usize = 0;
            const MAX_REGISTERS: usize = 32;

            #[test]
            fn no_registers() {
                let mut out = vec![];

                $crate::compile::<NO_REGISTERS>([(0, SOURCE)].as_slice())
                    .map(|compiled| {
                        $crate::evaluate::<NO_REGISTERS, _>(&compiled, &mut out);
                    })
                    .expect("examples should always compile");

                assert_eq!(str::from_utf8(out.as_slice()), Ok($expected_output));
            }

            #[test]
            fn max_registers() {
                let mut out = vec![];

                $crate::compile::<MAX_REGISTERS>([(0, SOURCE)].as_slice())
                    .map(|compiled| {
                        $crate::evaluate::<MAX_REGISTERS, _>(&compiled, &mut out);
                    })
                    .expect("examples should always compile");

                assert_eq!(str::from_utf8(out.as_slice()), Ok($expected_output));
            }
        }
    };
}

pub use __test_evaluation_output as test_evaluation_output;

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

            const NO_REGISTERS: usize = 0;

            #[test]
            fn compilation_error() {
                let mut out = vec![];

                let compiled = $crate::compile::<NO_REGISTERS>([(0, SOURCE)].as_slice()).map(|compiled| {
                    $crate::evaluate::<NO_REGISTERS, _>(&compiled, &mut out);
                });

                let errors = compiled.as_ref().map_err(|errors| {
                    errors
                        .iter()
                        .map(reporting::Spanned::kind)
                        .collect::<Vec<_>>()
                });

                let errors = errors.as_ref().map_err(std::vec::Vec::as_slice);

                std::assert_matches!(
                    errors,
                    Err($expected_errors) $(if $expected_errors_conditional)?
                );
            }
        }
    }
}

pub use __test_compilation_errors as test_compilation_errors;
