use apart::{Ir, IrValue, Target};

use std::{env, fs, iter, path::Path, process::ExitCode};

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

            let input_file_path = Path::new(input_path);

            if !input_file_path.exists() {
                eprintln!("\"{input_path}\" does not exist");
                ExitCode::FAILURE
            } else if !input_file_path.is_file() {
                eprintln!("\"{input_path}\" is not a file");
                ExitCode::FAILURE
            } else {
                match fs::read_to_string(input_file_path) {
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
                            let output_path = Path::new(options.output.as_str());

                            if let Err(error) = fs::write(
                                output_path,
                                match options.target {
                                    Target::Vm => apart::targets::vm::compile(&compiled),
                                },
                            ) {
                                eprintln!(
                                    "failed to write compiled output to \"{}\"; see next line",
                                    options.output
                                );

                                eprintln!("{error}");

                                return ExitCode::FAILURE;
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
) -> Option<apart::Compiled<'a, Ir<IrValue>>> {
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
                    source_index
                        .checked_sub(1)
                        .and_then(|source_index| source_labels.get(source_index))
                        .copied()
                        .unwrap_or("core"),
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

fn print_usage() {
    eprintln!("{}", DESCRIPTION.trim());
}

fn print_targets() {
    eprintln!("{}", TARGETS.trim());
}

const DESCRIPTION: &str = concat!(
    "Usage: ",
    env!("CARGO_BIN_NAME"),
    " [option]... PATH_TO_SOURCE

Description:
    Compile a program written in the \"apart\" language

    -l, --list-targets
        list all valid targets

    -o, --output OUTPUT_PATH
        specify the path of the compilation's output

    -O, --optimize
        perform optimizations during compilation

    -R, --registers usize
        specify the maximum number of registers to compile with, e.g. \"-r 0\" or \"--registers 32\"

        zero registers effectively works as a stack machine

    -T, --target TARGET_NAME
        compile for only the target TARGET_NAME
"
);

const TARGETS: &str = concat!(
    "Valid ",
    env!("CARGO_BIN_NAME"),
    " compilation targets:
    vm
"
);

struct ApartOptions {
    output: String,
    optimize: bool,
    max_registers: usize,
    target: Target,
}

fn parse_options(arguments: &mut iter::Peekable<env::Args>) -> Result<ApartOptions, ExitCode> {
    let mut options = ApartOptions {
        output: "main.apart".to_string(),
        optimize: false,
        max_registers: 0,
        target: Target::Vm,
    };

    while let Some(argument) = arguments.peek() {
        match argument.as_str() {
            "-?" | "--help" => {
                print_usage();

                return Err(ExitCode::SUCCESS);
            }
            "-l" | "--list-targets" => {
                print_targets();

                return Err(ExitCode::SUCCESS);
            }
            "-o" | "--output" => {
                let argument = arguments
                    .next()
                    .expect("we just matched while peeking, it will exist");

                if let Some(output) = arguments.next() {
                    options.output = output;
                } else {
                    eprintln!("expected the output path after {argument}");

                    return Err(ExitCode::FAILURE);
                }
            }
            "-O" | "--optimize" => {
                arguments.next();

                options.optimize = true;
            }
            "-R" | "--registers" => {
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
            "-T" | "--target" => {
                arguments.next();

                let target = arguments.next();

                #[allow(clippy::single_match_else)]
                match target.as_deref() {
                    Some("vm") => {
                        options.target = Target::Vm;
                    }
                    _ => {
                        eprintln!("invalid compilation target");

                        eprintln!(
                            "run with \"-l\" or \"--list-targets\" to view a list of valid targets"
                        );

                        return Err(ExitCode::FAILURE);
                    }
                }
            }
            _ => break,
        }
    }

    Ok(options)
}
