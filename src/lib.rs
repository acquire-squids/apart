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
    ssa::{Block, BlockTerminator, JumpTo, Ssa},
    type_check::Error as TypeCheckError,
};

use reporting::{Reportable, Span, Spanned};

use std::{error, fmt, io::Write};

const CORE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/lang/core.txt");

const CORE_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/core.txt"));

pub struct Compiled<'a, T> {
    sources_with_core: Vec<(usize, &'a str)>,
    result: T,
}

impl<'a, T> Compiled<'a, T> {
    #[allow(dead_code)]
    #[must_use]
    pub const fn sources(&self) -> &[(usize, &'a str)] {
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

pub fn evaluate<O>(compiled: &Compiled<'_, Ssa>, out: &mut O)
where
    O: Write,
{
    evaluate::run(
        compiled.result(),
        compiled.sources_with_core.as_slice(),
        out,
    );
}

/// # Errors
/// Will error if compilation fails, returning the errors for the relevant stage
#[allow(clippy::missing_panics_doc)]
pub fn compile<'a>(
    sources: &[(usize, &'a str)],
    max_registers: usize,
    optimized: bool,
) -> Result<Compiled<'a, Ssa>, Compiled<'a, Vec<Spanned<Error>>>> {
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

    for (source_id, source) in sources {
        sources_with_core.push((*source_id, source));
    }

    let mut ast = parse::Ast::new(core_source_id);

    for (source_id, source) in sources {
        let mut lexer = lex::Lexer::new(*source_id);

        lexer.push_source(source);

        parse::parse(&mut lexer, &mut ast).map_err(|errors| Compiled {
            sources_with_core: sources_with_core.clone(),
            result: errors
                .into_iter()
                .map(|error| error.transmute(Error::Parse))
                .collect::<Vec<_>>(),
        })?;
    }

    let names = name_resolve::resolve_names(&ast).map_err(|errors| Compiled {
        sources_with_core: sources_with_core.clone(),
        result: errors
            .into_iter()
            .map(|error| error.transmute(Error::NameResolve))
            .collect::<Vec<_>>(),
    })?;

    let types = type_check::check_types(&ast, &names).map_err(|errors| Compiled {
        sources_with_core: sources_with_core.clone(),
        result: errors
            .into_iter()
            .map(|error| error.transmute(Error::TypeCheck))
            .collect::<Vec<_>>(),
    })?;

    let basic_blocks = basic_blocks::translate(&ast, &names, &types);

    if cfg!(feature = "print_blocks") {
        print!("{basic_blocks}");
    }

    let mut ssa = ssa::convert(&basic_blocks, max_registers);

    if cfg!(feature = "print_ssa") {
        print!("{ssa}");
    }

    if optimized {
        optimize::optimize(&mut ssa);

        if cfg!(feature = "print_optimized") {
            print!("{ssa}");
        }
    }

    allocate_registers::allocate(&mut ssa);

    if cfg!(feature = "print_allocated") {
        print!("{ssa}");
    }

    Ok(Compiled {
        sources_with_core,
        result: ssa,
    })
}

#[macro_export]
macro_rules! __test_evaluation_output_single_function {
    (
        $source:ident, $test_file_name:literal, $expected_output:literal ;
        $test_name:ident, $registers:literal, $optimized:literal $(,)?
    ) => {
        #[test]
        fn $test_name() {
            let mut out = vec![];

            let result = $crate::compile([(0, SOURCE)].as_slice(), $registers, $optimized)
                .map(|compiled| {
                    $crate::evaluate(&compiled, &mut out);
                })
                .map(|()| str::from_utf8(out.as_slice()).expect("only utf-8!  sorry!"));

            match result {
                Ok(output) => {
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
                            compiled.sources()[source_index].1,
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

pub use __test_evaluation_output_single_function as test_evaluation_output_single_function;

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

            $crate::test_evaluation_output_single_function!(
                SOURCE, $test_file_name, $expected_output ;
                no_registers_unoptimized, 0, false
            );

            $crate::test_evaluation_output_single_function!(
                SOURCE, $test_file_name, $expected_output ;
                no_registers_optimized, 0, true
            );

            $crate::test_evaluation_output_single_function!(
                SOURCE, $test_file_name, $expected_output ;
                registers_unoptimized, 32, false
            );

            $crate::test_evaluation_output_single_function!(
                SOURCE, $test_file_name, $expected_output ;
                registers_optimized, 32, true
            );
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
                let result = $crate::compile([(0, SOURCE)].as_slice(), NO_REGISTERS, false)
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
