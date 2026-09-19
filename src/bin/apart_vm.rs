use std::{env, fs, io, path::Path, process::ExitCode};

fn main() -> ExitCode {
    let mut arguments = env::args();
    arguments.next();

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
                match fs::read(input_file_path) {
                    Err(error) => {
                        eprintln!("error while reading \"{input_path}\": {error}");
                        ExitCode::FAILURE
                    }
                    Ok(bytecode) => {
                        run(bytecode.as_slice());

                        ExitCode::SUCCESS
                    }
                }
            }
        },
    )
}

fn run(bytecode: &[u8]) {
    apart::vm::run(bytecode, &mut io::stdout().lock());
}

fn print_usage() {
    eprintln!(
        "Usage: {} [path_to_binary]\n\n{}",
        option_env!("CARGO_BIN_NAME").unwrap_or("apart_vm"),
        DESCRIPTION.trim(),
    );
}

const DESCRIPTION: &str = "Description:
    Run a binary compiled by apart_c for the vm target
";
