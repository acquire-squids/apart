use apart::Ssa;

use std::{env, fs, io, iter, path::Path, process::ExitCode};

fn main() -> ExitCode {
    let mut arguments = env::args().peekable();
    arguments.next();

    let options = match parse_options(&mut arguments) {
        Ok(options) => options,
        Err(exit_code) => return exit_code,
    };

    arguments.next().map_or_else(
        || {
            print_usage();

            ExitCode::FAILURE
        },
        |file_path| {
            let input_path = file_path.as_str();

            let file_path = Path::new(input_path);

            if !file_path.exists() {
                eprintln!("\"{input_path}\" does not exist");
                ExitCode::FAILURE
            } else if !file_path.is_file() {
                eprintln!("\"{input_path}\" is not a file");
                ExitCode::FAILURE
            } else {
                match fs::read_to_string(file_path) {
                    Err(error) => {
                        eprintln!("error while reading \"{input_path}\": {error}");
                        ExitCode::FAILURE
                    }
                    Ok(source) => {
                        let source_labels = [input_path];

                        let sources = [(0, source.as_str())];

                        let source_labels = source_labels.as_slice();
                        let sources = sources.as_slice();

                        compile(
                            source_labels,
                            sources,
                            options.max_registers,
                            options.optimize,
                        )
                        .map_or(ExitCode::FAILURE, |compiled| {
                            if options.evaluate {
                                evaluate(&compiled);
                            }

                            ExitCode::SUCCESS
                        })
                    }
                }
            }
        },
    )
}

fn compile<'a>(
    source_labels: &[&str],
    sources: &[(usize, &'a str)],
    max_registers: usize,
    optimize: bool,
) -> Option<apart::Compiled<'a, Ssa>> {
    match apart::compile(sources, max_registers, optimize) {
        Ok(compiled) => Some(compiled),
        Err(compiled) => {
            let sources = compiled.sources();

            let errors = compiled.result();

            for error in errors {
                let source_index = sources
                    .iter()
                    .position(|(id, _)| *id == error.span().source_id())
                    .expect("the source must exist");

                let report_data = reporting::ReportData::new(
                    sources[source_index].1,
                    "error",
                    source_labels.get(source_index).copied().unwrap_or("core"),
                    "...",
                    reporting::ReportColors::new(),
                );

                let mut err = vec![];

                let _ = report_data.report(error, &mut err);

                eprint!(
                    "{}",
                    str::from_utf8(err.as_slice()).expect("only utf-8!  sorry!")
                );
            }

            None
        }
    }
}

fn evaluate(compiled: &apart::Compiled<'_, Ssa>) {
    apart::evaluate(compiled, &mut io::stdout().lock());
}

fn print_usage() {
    eprintln!(
        "Usage: {} [option]... [path_to_source]\n\n{}",
        option_env!("CARGO_BIN_NAME").unwrap_or("apart_c"),
        DESCRIPTION.trim(),
    );
}

const DESCRIPTION: &str = "Description:
    Compile (and optionally, evaluate) a program written in the \"apart\" language

    -e, --evaluate
        evaluate the program after compiling it

    -o, --optimize
        perform optimizations during compilation

    -r, --registers usize
        specify the maximum number of registers to compile with, e.g. \"-r 0\" or \"--registers 32\"

        zero registers effectively works as a stack machine
";

struct ApartOptions {
    evaluate: bool,
    optimize: bool,
    max_registers: usize,
}

fn parse_options(arguments: &mut iter::Peekable<env::Args>) -> Result<ApartOptions, ExitCode> {
    let mut options = ApartOptions {
        evaluate: false,
        optimize: false,
        max_registers: 0,
    };

    while let Some(argument) = arguments.peek() {
        match argument.as_str() {
            "-?" | "--help" => {
                print_usage();

                return Err(ExitCode::SUCCESS);
            }
            "-e" | "--evaluate" => {
                arguments.next();

                options.evaluate = true;
            }
            "-o" | "--optimize" => {
                arguments.next();

                options.optimize = true;
            }
            "-r" | "--registers" => {
                let argument = arguments
                    .next()
                    .expect("we just matched while peeking, it will exist");

                if let Some(registers) = arguments.next() {
                    match registers.parse::<usize>() {
                        Ok(max_registers) => {
                            options.max_registers = max_registers;
                        }
                        Err(error) => {
                            eprintln!(
                                "failed to parse the maximum registers as a usize; see next line"
                            );
                            eprintln!("{error}");

                            return Err(ExitCode::FAILURE);
                        }
                    }
                } else {
                    eprintln!("expected the number of registers to compile with after {argument}");

                    return Err(ExitCode::FAILURE);
                }
            }
            _ => break,
        }
    }

    Ok(options)
}
