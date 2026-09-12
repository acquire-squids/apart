use std::{env, fs, io, path::Path, process::ExitCode};

const MAX_REGISTERS: usize = 0;

fn main() -> ExitCode {
    let mut arguments = env::args();
    arguments.next();

    arguments.next().map_or_else(
        || {
            eprintln!(
                "Usage: {} [path_to_source]",
                option_env!("CARGO_BIN_NAME").unwrap_or("apart")
            );
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
                        let source = source.as_str();

                        compile::<MAX_REGISTERS>(input_path, source).map_or(
                            ExitCode::FAILURE,
                            |compiled| {
                                if cfg!(feature = "evaluate") {
                                    evaluate::<MAX_REGISTERS>(&compiled);
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

fn compile<'a, const MAX_REGISTERS: usize>(
    source_label: &str,
    source: &'a str,
) -> Option<apart::Compiled<'a>> {
    match apart::compile::<MAX_REGISTERS>([(0, source)].as_slice()) {
        Ok(compiled) => Some(compiled),
        Err(errors) => {
            let report_data = reporting::ReportData::new(
                source,
                "error",
                source_label,
                "...",
                reporting::ReportColors::new(),
            );

            for error in errors {
                let _ = report_data.report(&error, &mut io::stderr().lock());
            }

            None
        }
    }
}

fn evaluate<const MAX_REGISTERS: usize>(compiled: &apart::Compiled<'_>) {
    apart::evaluate::<MAX_REGISTERS, _>(compiled, &mut io::stdout().lock());
}
