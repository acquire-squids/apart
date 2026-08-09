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
    name_resolve::Error as NameResolveError, parse::Error as ParseError,
    type_check::Error as TypeCheckError,
};

use reporting::{Reportable, Span, Spanned};

use std::{error, fmt, io::Write};

const CORE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/lang/core.txt");

const CORE_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/core.txt"));

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

/// # Errors
/// Will error if compilation fails, returning the errors for the relevant stage
#[allow(clippy::missing_panics_doc)]
pub fn compile<const MAX_REGISTERS: usize, O>(
    sources: &[(usize, &str)],
    out: &mut O,
) -> Result<Vec<u8>, Vec<Spanned<Error>>>
where
    O: Write,
{
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
                source_ids.iter().rev().find_map(|source_id| {
                    source_id
                        .checked_add(1)
                        .map_or_else(|| source_id.checked_sub(1), Some)
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

    evaluate::run::<MAX_REGISTERS, O>(&ssa, sources_with_core.as_slice(), out);

    Ok(vec![])
}
