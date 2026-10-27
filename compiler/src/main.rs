use std::io::Read;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let emit_assembly = args.iter().any(|a| a == "-S");
    let Some(path) = args.iter().find(|a| !a.starts_with('-')) else {
        eprintln!("usage: compiler [-S] <file.c>");
        return ExitCode::from(2);
    };
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{}: {}", path, error);
            return ExitCode::from(2);
        }
    };

    let report = |errors: Vec<compiler::CompileError>| {
        for error in errors {
            eprintln!("{}:{}: error: {}", path, error.line, error.message);
        }
        ExitCode::from(1)
    };

    if emit_assembly {
        return match compiler::compile(&source) {
            Ok(assembly) => {
                print!("{}", assembly);
                ExitCode::SUCCESS
            }
            Err(errors) => report(errors),
        };
    }

    let words = match compiler::build(&source) {
        Ok(words) => words,
        Err(errors) => return report(errors),
    };
    let mut input = String::new();
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        let _ = std::io::stdin().read_to_string(&mut input);
    }
    let outcome = compiler::execute(&words, &input);
    print!("{}", outcome.output);
    if let Some(error) = outcome.error {
        eprintln!("runtime error: {}", error);
        return ExitCode::from(1);
    }
    ExitCode::from(outcome.exit_code.unwrap_or(0) as u8)
}
