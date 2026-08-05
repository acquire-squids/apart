mod allocate_registers;
mod basic_blocks;
mod evaluate;
mod lex;
mod name_resolve;
mod optimize;
mod parse;
mod ssa;
mod type_check;

use reporting::{Reportable, Span, Spanned};

use std::{error::Error, fmt, io::Write};

const MAX_REGISTERS: usize = 0;

const CORE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/lang/core.txt");

const CORE_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/core.txt"));

pub struct ErrorBox<E: ?Sized>(Box<E>);

impl<E> fmt::Debug for ErrorBox<E>
where
    E: fmt::Debug + ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl<E> fmt::Display for ErrorBox<E>
where
    E: fmt::Display + ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<E> Error for ErrorBox<E> where E: Error + ?Sized {}

impl<E> Reportable for ErrorBox<E> where E: Reportable + ?Sized {}

/// # Errors
/// Will error if compilation fails, returning the errors for the relevant stage
#[allow(clippy::missing_panics_doc)]
pub fn compile<O>(
    sources: &[(usize, &str)],
    out: &mut O,
) -> Result<Vec<u8>, Vec<Spanned<ErrorBox<dyn Reportable>>>>
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
                .map(|error| {
                    error.transmute(|error| ErrorBox(Box::new(error) as Box<dyn Reportable>))
                })
                .collect::<Vec<_>>()
        })?;

        sources_with_core.push((*source_id, source));
    }

    let names = name_resolve::resolve_names(&ast).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.transmute(|error| ErrorBox(Box::new(error) as Box<dyn Reportable>)))
            .collect::<Vec<_>>()
    })?;

    let types = type_check::check_types(&ast, &names).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.transmute(|error| ErrorBox(Box::new(error) as Box<dyn Reportable>)))
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
