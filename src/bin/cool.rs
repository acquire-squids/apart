use std::{
    env, fs,
    io::{self, BufRead, Write},
    path::Path,
    process::ExitCode,
};

fn main() -> ExitCode {
    let mut arguments = env::args();
    arguments.next();

    arguments.next().map_or_else(
        || {
            repl().map_or_else(
                |error| {
                    eprintln!("repl input/output error: {error}");
                    ExitCode::FAILURE
                },
                |()| ExitCode::SUCCESS,
            )
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

                        compile(input_path, source, false);

                        ExitCode::SUCCESS
                    }
                }
            }
        },
    )
}

fn compile(source_label: &str, source: &str, silent: bool) {
    match apart::compile([(0, source)].as_slice(), &mut io::stdout().lock()) {
        Ok(_) => {}
        Err(_) if silent => {}
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
        }
    }
}

fn repl() -> io::Result<()> {
    print!("> ");
    io::stdout().lock().flush()?;

    for line in io::stdin().lock().lines() {
        let source = line?;

        compile("stdin", source.as_str(), false);

        print!("> ");
        io::stdout().lock().flush()?;
    }

    println!();

    Ok(())
}
