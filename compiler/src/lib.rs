pub mod ast;
pub mod codegen;
pub mod lexer;
pub mod parser;

pub use codegen::MEMORY_SIZE;
pub use emulator::Outcome;

use lexer::Token;

pub const MAX_STEPS: u64 = 20_000_000;

const PRELUDE: &str = include_str!("prelude.c");

#[derive(Debug, PartialEq)]
pub struct CompileError {
    pub line: usize,
    pub message: String,
}

pub struct Compilation {
    pub tokens: Vec<Token>,
    pub program: Option<ast::Program>,
    pub assembly: Option<String>,
    pub errors: Vec<CompileError>,
}

pub fn analyze(source: &str) -> Compilation {
    let mut compilation = Compilation {
        tokens: Vec::new(),
        program: None,
        assembly: None,
        errors: Vec::new(),
    };
    match lexer::tokenize(source) {
        Ok(tokens) => compilation.tokens = tokens,
        Err(error) => {
            compilation.errors.push(error);
            return compilation;
        }
    }
    let program = match parser::parse(compilation.tokens.clone()) {
        Ok(program) => program,
        Err(error) => {
            compilation.errors.push(error);
            return compilation;
        }
    };
    let prelude = lexer::tokenize(PRELUDE)
        .and_then(parser::parse)
        .expect("the prelude should compile");
    match codegen::generate(&program, &prelude) {
        Ok(assembly) => compilation.assembly = Some(assembly),
        Err(errors) => compilation.errors = errors,
    }
    compilation.program = Some(program);
    compilation
}

pub fn compile(source: &str) -> Result<String, Vec<CompileError>> {
    let compilation = analyze(source);
    match compilation.assembly {
        Some(assembly) => Ok(assembly),
        None => Err(compilation.errors),
    }
}

pub fn build(source: &str) -> Result<Vec<u32>, Vec<CompileError>> {
    let assembly = compile(source)?;
    assembler::assemble(&assembly).map_err(|errors| {
        errors
            .into_iter()
            .map(|e| CompileError {
                line: 0,
                message: format!("Assembler error on line {}: {}", e.line, e.message),
            })
            .collect()
    })
}

pub fn execute(words: &[u32], input: &str) -> Outcome {
    emulator::run(words, MEMORY_SIZE, input, MAX_STEPS)
}

pub fn run(source: &str, input: &str) -> Result<Outcome, Vec<CompileError>> {
    Ok(execute(&build(source)?, input))
}
