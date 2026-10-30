use apart::{ApartOptions, Ir, IrValue, Target};

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

            if !input_file_path.exists() || !input_file_path.is_file() {
                eprintln!("\"{input_path}\" does not exist as an input file and is not a known {} flag", env!("CARGO_BIN_NAME"));
                ExitCode::FAILURE
            } else {
                match fs::read_to_string(input_file_path) {
                    Err(error) => {
                        eprintln!("error while reading \"{input_path}\": {error}");
                        ExitCode::FAILURE
                    }
                    Ok(source) => {
                        let sources = [(0, (input_path, source.as_str()))];

                        let sources = sources.as_slice();

                        compile(&options, sources).map_or(
                            ExitCode::FAILURE,
                            |compiled| {
                                if options.run {
                                    return apart::vm::run(
                                        apart::targets::vm::compile(&options, &compiled).as_slice(),
                                        &mut std::io::stdout().lock()
                                    ).map_or(ExitCode::FAILURE, |()| ExitCode::SUCCESS);
                                }

                                let output_path = Path::new(options.output.as_str());

                                if let Some(directory) = output_path.parent() && let Err(error) = fs::create_dir_all(directory) {
                                    eprintln!("failed to write compiled output to \"{}\"; see next line", options.output);

                                    eprintln!("{error}");

                                    return ExitCode::FAILURE;
                                }

                                match options.target {
                                    Target::NttGml => {
                                        if let Err(error) = fs::create_dir_all(output_path) {
                                            eprintln!("failed to write compiled output to \"{}\"; see next line", options.output);

                                            eprintln!("{error}");

                                            return ExitCode::FAILURE;
                                        }

                                        let mut ntt_gml_files = vec![];

                                        let output = apart::targets::ntt_gml::compile(&options, &compiled);

                                        let mut errored = false;

                                        for (ntt_output_path, ntt_output_source) in &output {
                                            ntt_gml_files.push(ntt_output_path);

                                            let ntt_output_path = output_path.join(ntt_output_path);

                                            if let Some(directory) = ntt_output_path.parent() && let Err(error) = fs::create_dir_all(directory) {
                                                eprintln!("failed to write compiled output to \"{}\"; see next line", ntt_output_path.display());

                                                eprintln!("{error}");

                                                errored |= true;

                                                continue;
                                            }

                                            if let Err(error) = fs::write(&ntt_output_path, ntt_output_source) {
                                                eprintln!("failed to write compiled output to \"{}\"; see next line", ntt_output_path.display());

                                                eprintln!("{error}");

                                                errored |= true;
                                            }
                                        }

                                        if errored {
                                            return ExitCode::FAILURE;
                                        }

                                        if let Err(error) = fs::write(output_path.join("main.txt"), ntt_gml_files.iter().map(|ntt_gml_file| format!("/loadlive {}", ntt_gml_file.display())).fold(String::new(), |mut accum, command| {
                                            accum.push_str(command.as_str());

                                            accum.push('\n');

                                            accum
                                        })) {
                                            eprintln!("failed to write compiled output to \"{}\"; see next line", output_path.join("main.txt").display());

                                            eprintln!("{error}");

                                            return ExitCode::FAILURE;
                                        }
                                    }
                                    Target::Vm => {
                                        if let Err(error) = fs::write(
                                            output_path.with_extension("apart"), apart::targets::vm::compile(&options, &compiled)
                                        ) {
                                            eprintln!(
                                                "failed to write compiled output to \"{}\"; see next line",
                                                options.output
                                            );

                                            eprintln!("{error}");

                                            return ExitCode::FAILURE;
                                        }
                                    },
                                }

                                ExitCode::SUCCESS
                            },
                        )
                    }
                }
            }
        },
    )
}

fn compile<'a>(
    options: &ApartOptions,
    sources: &[(usize, (&'a str, &'a str))],
) -> Option<apart::Compiled<'a, Ir<IrValue>>> {
    match apart::compile(options, sources) {
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
                    sources[source_index].1.1,
                    "error",
                    sources
                        .get(source_index)
                        .map(|(_, (file_name, _))| file_name)
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

    -h, --help
        view this text

    -l, --list-targets
        list all valid targets

    -o, --output OUTPUT_PATH
        specify the path of the compilation's output

    -O, --optimize
        perform optimizations during compilation

    -r, --run
        also compile to bytecode and run the bytecode directly, without outputting to a file

    -S, --strip-debug-info
        compile without including location data for panics

    -T, --target TARGET_NAME
        compile for only the target TARGET_NAME
"
);

const TARGETS: &str = concat!(
    "Valid ",
    env!("CARGO_BIN_NAME"),
    " compilation targets:
    ntt_gml
    vm
"
);

fn parse_options(arguments: &mut iter::Peekable<env::Args>) -> Result<ApartOptions, ExitCode> {
    let mut options = ApartOptions {
        output: "main".to_string(),
        strip_debug_info: false,
        optimize: false,
        target: Target::Vm,
        run: false,
    };

    while let Some(argument) = arguments.peek() {
        match argument.as_str() {
            "-h" | "--help" => {
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

                if let Some(specified_output) = arguments.next() {
                    options.output = specified_output;
                } else {
                    eprintln!("expected the output path after {argument}");

                    return Err(ExitCode::FAILURE);
                }
            }
            "-O" | "--optimize" => {
                arguments.next();

                options.optimize = true;
            }
            "-r" | "--run" => {
                arguments.next();

                options.run = true;
            }
            "-S" | "--strip-debug-info" => {
                arguments.next();

                options.strip_debug_info = true;
            }
            "-T" | "--target" => {
                arguments.next();

                let target = arguments.next();

                match target.as_deref() {
                    Some("ntt_gml") => {
                        options.target = Target::NttGml;
                    }
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
