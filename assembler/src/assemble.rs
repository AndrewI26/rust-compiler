use std::collections::HashMap;

use crate::instruction::{Condition, Instruction, Register, ValidationError};
use crate::mc::{compile_line, fields};
use crate::munch::munch;
use crate::token::Token::{self, *};

/// An error on one line of the source.
#[derive(Debug, PartialEq)]
pub struct AssembleError {
    /// 1-based line number.
    pub line: usize,
    pub message: String,
}

/// A parsed line: either an instruction or a `.8byte` data value.
pub enum Item {
    Instruction(Instruction),
    Data(u64),
}

impl Item {
    /// Encodes the item as 32-bit words. Data is two words, high word first.
    pub fn encode(&self) -> Vec<u32> {
        match self {
            Item::Instruction(instruction) => vec![compile_line(instruction)],
            Item::Data(value) => vec![(value >> 32) as u32, *value as u32],
        }
    }

    /// Bit widths of each field in one encoded word, for display.
    pub fn fields(&self) -> &'static [u32] {
        match self {
            Item::Instruction(instruction) => fields(instruction),
            Item::Data(_) => &[32],
        }
    }
}

/// An item along with where it came from.
pub struct ParsedLine {
    /// 1-based source line number.
    pub line: usize,
    /// Index of the item's first word in the output.
    pub index: i32,
    pub item: Item,
}

/// The result of each stage of assembling a program.
pub struct Program {
    /// Stage 1: the tokens of each source line, or why it failed to lex.
    pub tokens: Vec<Result<Vec<Token>, String>>,
    /// Stage 2: each label and the word index it points at, in source order.
    pub labels: Vec<(String, i32)>,
    /// Stage 3: every line that parsed and validated successfully.
    pub items: Vec<ParsedLine>,
    /// Every error from every stage, sorted by line.
    pub errors: Vec<AssembleError>,
}

/// Assembles a whole program into machine code as 32-bit words. Each
/// instruction is one word, and each `.8byte` directive is two words
/// (big-endian, high word first).
///
/// # Errors
///
/// Returns every error found, sorted by line, if the input is invalid ARM
/// syntax, uses an unknown label, or fails validation.
pub fn assemble(input: &str) -> Result<Vec<u32>, Vec<AssembleError>> {
    let program = analyze(input);
    if !program.errors.is_empty() {
        return Err(program.errors);
    }

    Ok(program.items.iter().flat_map(|p| p.item.encode()).collect())
}

/// Runs every stage before encoding and keeps each stage's result.
///
/// Runs in two passes: the first records the word index of every label, the
/// second parses each line, so branches can jump forward.
pub fn analyze(input: &str) -> Program {
    let mut errors = Vec::new();
    let mut error = |number: usize, message: String| {
        errors.push(AssembleError {
            line: number + 1,
            message,
        })
    };

    // Stage 1: tokenize each line.
    let tokens: Vec<Result<Vec<Token>, String>> = input
        .lines()
        .map(|line| munch(&line.to_string()).map(|tokens| tokens.0))
        .collect();

    for (number, line) in tokens.iter().enumerate() {
        if let Err(message) = line {
            error(number, message.clone());
        }
    }

    // Stage 2 (pass 1): record which word each label points at.
    let mut labels = Vec::new();
    let mut lookup: HashMap<String, i32> = HashMap::new();
    let mut index = 0;

    for (number, line) in tokens.iter().enumerate() {
        let Ok(line) = line else {
            // Guess that a line which didn't lex was one instruction.
            index += 1;
            continue;
        };
        let (line_labels, rest) = split_labels(line);

        for label in line_labels {
            if lookup.insert(label.to_string(), index).is_some() {
                error(number, format!("Duplicate label {:?}", label));
            } else {
                labels.push((label.to_string(), index));
            }
        }

        index += words(rest);
    }

    // Stage 3 (pass 2): parse and validate each instruction or directive.
    let mut items = Vec::new();
    let mut index = 0;

    for (number, line) in tokens.iter().enumerate() {
        let Ok(line) = line else {
            index += 1;
            continue;
        };
        let (_, rest) = split_labels(line);

        if !rest.is_empty() {
            match parse_item(rest, index, &lookup) {
                Ok(item) => items.push(ParsedLine {
                    line: number + 1,
                    index,
                    item,
                }),
                Err(message) => error(number, message),
            }
        }

        index += words(rest);
    }

    errors.sort_by_key(|e| e.line);

    Program {
        tokens,
        labels,
        items,
        errors,
    }
}

/// Parses and validates one line (labels already removed).
fn parse_item(rest: &[Token], index: i32, labels: &HashMap<String, i32>) -> Result<Item, String> {
    match rest {
        [Dotid(directive), value] if directive == ".8byte" => Ok(Item::Data(immediate64(value)?)),

        _ => {
            let instruction = parse_line(rest, index, labels)?;
            instruction.validate().map_err(|e| match e {
                ValidationError::InvalidRegister => {
                    String::from("xzr can't be used as the destination or base register")
                }
                ValidationError::InvalidOffset => String::from("Offset is out of range"),
            })?;

            Ok(Item::Instruction(instruction))
        }
    }
}

/// Splits the leading labels (without their `:`) off a line of tokens.
fn split_labels(line: &[Token]) -> (Vec<&str>, &[Token]) {
    let mut labels = Vec::new();
    let mut rest = line;

    while let [Label(label), tail @ ..] = rest {
        labels.push(label.trim_end_matches(':'));
        rest = tail;
    }

    (labels, rest)
}

/// Number of 32-bit words a line (labels already removed) takes up.
fn words(rest: &[Token]) -> i32 {
    match rest {
        [] => 0,
        [Dotid(directive), ..] if directive == ".8byte" => 2,
        _ => 1,
    }
}

/// Parses the tokens of a single instruction (labels already removed).
fn parse_line(
    tokens: &[Token],
    index: i32,
    labels: &HashMap<String, i32>,
) -> Result<Instruction, String> {
    let instruction = match tokens {
        // `b.cond` lexes as `b` followed by a dotid such as `.eq`.
        [Id(op), Dotid(cond), t] if op == "b" => Instruction::BCond {
            cond: condition(cond)?,
            offset: target(t, index, labels)?,
        },

        [Id(op), operands @ ..] => match (op.as_str(), operands) {
            ("add", [d, Comma, n, Comma, m]) => Instruction::Add {
                rd: reg(d)?,
                rn: reg(n)?,
                rm: reg(m)?,
            },
            ("sub", [d, Comma, n, Comma, m]) => Instruction::Sub {
                rd: reg(d)?,
                rn: reg(n)?,
                rm: reg(m)?,
            },
            ("mul", [d, Comma, n, Comma, m]) => Instruction::Mul {
                rd: reg(d)?,
                rn: reg(n)?,
                rm: reg(m)?,
            },
            ("smulh", [d, Comma, n, Comma, m]) => Instruction::Smulh {
                rd: reg(d)?,
                rn: reg(n)?,
                rm: reg(m)?,
            },
            ("umulh", [d, Comma, n, Comma, m]) => Instruction::Umulh {
                rd: reg(d)?,
                rn: reg(n)?,
                rm: reg(m)?,
            },
            ("sdiv", [d, Comma, n, Comma, m]) => Instruction::Sdiv {
                rd: reg(d)?,
                rn: reg(n)?,
                rm: reg(m)?,
            },
            ("udiv", [d, Comma, n, Comma, m]) => Instruction::Udiv {
                rd: reg(d)?,
                rn: reg(n)?,
                rm: reg(m)?,
            },
            ("cmp", [n, Comma, m]) => Instruction::Cmp {
                rn: reg(n)?,
                rm: reg(m)?,
            },
            ("br", [n]) => Instruction::Br { rn: reg(n)? },
            ("blr", [n]) => Instruction::Blr { rn: reg(n)? },

            ("ldur", [d, Comma, Lbrack, n, Comma, i, Rbrack]) => Instruction::Ldur {
                rd: reg(d)?,
                rn: reg(n)?,
                offset: immediate(i)?,
            },
            ("stur", [d, Comma, Lbrack, n, Comma, i, Rbrack]) => Instruction::Stur {
                rd: reg(d)?,
                rn: reg(n)?,
                offset: immediate(i)?,
            },

            ("ldr", [d, Comma, t]) => Instruction::Ldr {
                rd: reg(d)?,
                offset: target(t, index, labels)?,
            },
            ("b", [t]) => Instruction::B {
                offset: target(t, index, labels)?,
            },

            _ => {
                return Err(match syntax(op) {
                    Some(syntax) => format!("Wrong operands, expected `{}`", syntax),
                    None => format!("Unknown instruction `{}`", op),
                });
            }
        },

        _ => return Err(String::from("Expected an instruction")),
    };

    Ok(instruction)
}

/// The expected syntax of each instruction, for error messages.
fn syntax(op: &str) -> Option<&'static str> {
    match op {
        "add" => Some("add xd, xn, xm"),
        "sub" => Some("sub xd, xn, xm"),
        "mul" => Some("mul xd, xn, xm"),
        "smulh" => Some("smulh xd, xn, xm"),
        "umulh" => Some("umulh xd, xn, xm"),
        "sdiv" => Some("sdiv xd, xn, xm"),
        "udiv" => Some("udiv xd, xn, xm"),
        "cmp" => Some("cmp xn, xm"),
        "br" => Some("br xn"),
        "blr" => Some("blr xn"),
        "ldur" => Some("ldur xd, [xn, i]"),
        "stur" => Some("stur xd, [xn, i]"),
        "ldr" => Some("ldr xd, i"),
        "b" => Some("b i"),
        _ => None,
    }
}

/// Converts a register token (`x0`-`x30` or `xzr`) into a `Register`.
fn reg(token: &Token) -> Result<Register, String> {
    use Register::*;

    const REGISTERS: [Register; 31] = [
        X0, X1, X2, X3, X4, X5, X6, X7, X8, X9, X10, X11, X12, X13, X14, X15, X16, X17, X18, X19,
        X20, X21, X22, X23, X24, X25, X26, X27, X28, X29, X30,
    ];

    match token {
        Zreg => Ok(XZR),
        Reg(name) => {
            let number: usize = name[1..].parse().unwrap();
            match REGISTERS.get(number) {
                Some(register) => Ok(*register),
                None => return Err(format!("Invalid register {:?}", name)),
            }
        }
        _ => return Err(format!("Expected a register, found {:?}", token)),
    }
}

/// Converts an `Int` or `Hexint` token into its value.
fn immediate(token: &Token) -> Result<i32, String> {
    match token {
        Int(value) => value.parse().map_err(|_| out_of_range(value)),
        Hexint(value) => i32::from_str_radix(&value[2..], 16).map_err(|_| out_of_range(value)),
        _ => return Err(format!("Expected an integer, found {:?}", token)),
    }
}

/// Converts an `Int` or `Hexint` token into a 64-bit value for `.8byte`.
/// Negative integers are stored in two's complement.
fn immediate64(token: &Token) -> Result<u64, String> {
    match token {
        Int(value) => value
            .parse::<i64>()
            .map(|value| value as u64)
            .map_err(|_| out_of_range(value)),
        Hexint(value) => u64::from_str_radix(&value[2..], 16).map_err(|_| out_of_range(value)),
        _ => return Err(format!("Expected an integer, found {:?}", token)),
    }
}

fn out_of_range(value: &str) -> String {
    format!("Integer {:?} out of range", value)
}

/// Converts a branch/ldr target into an offset in instructions. The target is
/// either a label name or a literal offset.
fn target(token: &Token, index: i32, labels: &HashMap<String, i32>) -> Result<i32, String> {
    match token {
        Id(label) => match labels.get(label) {
            Some(target_index) => Ok(target_index - index),
            None => return Err(format!("Unknown label {:?}", label)),
        },
        _ => immediate(token),
    }
}

/// Converts a `.cond` suffix such as `.eq` into a `Condition`.
fn condition(suffix: &str) -> Result<Condition, String> {
    match suffix {
        ".eq" => Ok(Condition::Eq),
        ".ne" => Ok(Condition::Ne),
        ".hs" => Ok(Condition::Hs),
        ".lo" => Ok(Condition::Lo),
        ".hi" => Ok(Condition::Hi),
        ".ls" => Ok(Condition::Ls),
        ".ge" => Ok(Condition::Ge),
        ".lt" => Ok(Condition::Lt),
        ".gt" => Ok(Condition::Gt),
        ".le" => Ok(Condition::Le),
        _ => return Err(format!("Unknown condition {:?}", suffix)),
    }
}

#[cfg(test)]
mod tests {
    use super::AssembleError;

    fn assemble(input: &str) -> Vec<u32> {
        super::assemble(input).unwrap()
    }

    fn is_error(input: &str) -> bool {
        super::assemble(input).is_err()
    }

    #[test]
    fn empty_program() {
        assert_eq!(assemble(""), vec![]);
    }

    #[test]
    fn program_with_labels() {
        let program = "
main: add x1, x2, x3
      b.eq main
      b end
      cmp x1, xzr
end:  br x30
";
        assert_eq!(
            assemble(program),
            vec![0x8B236041, 0x54FFFFE0, 0x14000002, 0xEB3F603F, 0xD61F03C0]
        );
    }

    #[test]
    fn label_on_its_own_line() {
        assert_eq!(assemble("loop:\n  b loop\n"), vec![0x14000000]);
    }

    #[test]
    fn loads_and_stores() {
        assert_eq!(
            assemble("ldur x0, [x1, 0x10]\nstur x2, [x3, -8]\nldr x4, 2"),
            vec![0xF8410020, 0xF81F8062, 0x58000044]
        );
    }

    #[test]
    fn eight_byte_directive() {
        assert_eq!(
            assemble(".8byte 0x123456789ABCDEF0\n.8byte -1\n.8byte 5"),
            vec![0x12345678, 0x9ABCDEF0, 0xFFFFFFFF, 0xFFFFFFFF, 0, 5]
        );
    }

    #[test]
    fn ldr_from_data_label() {
        let program = "
       ldr x1, value
       br x30
value: .8byte 0x123456789ABCDEF0
";
        assert_eq!(
            assemble(program),
            vec![0x58000041, 0xD61F03C0, 0x12345678, 0x9ABCDEF0]
        );
    }

    #[test]
    fn labels_after_data_count_two_words() {
        assert_eq!(
            assemble(".8byte 5\nb end\nend: br x30"),
            vec![0, 5, 0x14000001, 0xD61F03C0]
        );
    }

    #[test]
    fn errors_include_line_number() {
        assert_eq!(
            super::assemble("add x1, x2, x3\nb nowhere"),
            Err(vec![AssembleError {
                line: 2,
                message: String::from("Unknown label \"nowhere\""),
            }])
        );
    }

    #[test]
    fn reports_every_error() {
        let lines: Vec<usize> = super::assemble("add x1, #5\nadd x1, x2, x3\nfoo x1\nb nowhere")
            .unwrap_err()
            .iter()
            .map(|e| e.line)
            .collect();
        assert_eq!(lines, vec![1, 3, 4]);
    }

    #[test]
    fn analyze_keeps_every_stage() {
        let program = super::analyze("start: add x1, x2, x3\n\nb start\nfoo");

        assert_eq!(program.tokens.len(), 4);
        assert_eq!(program.labels, vec![(String::from("start"), 0)]);
        assert_eq!(
            program
                .items
                .iter()
                .map(|p| (p.line, p.index))
                .collect::<Vec<_>>(),
            vec![(1, 0), (3, 1)]
        );
        assert_eq!(program.errors.len(), 1);
        assert_eq!(program.errors[0].line, 4);
    }

    #[test]
    fn helpful_messages() {
        let message = |input| super::assemble(input).unwrap_err()[0].message.clone();

        assert_eq!(
            message("add x1, x2"),
            "Wrong operands, expected `add xd, xn, xm`"
        );
        assert_eq!(message("mov x1, x2"), "Unknown instruction `mov`");
        assert_eq!(message("ldur x1, [x2, 300]"), "Offset is out of range");
    }

    #[test]
    fn unknown_directive_is_error() {
        assert!(is_error(".word 5"));
    }

    #[test]
    fn unknown_label_is_error() {
        assert!(is_error("b nowhere"));
    }

    #[test]
    fn duplicate_label_is_error() {
        assert!(is_error("a: add x1, x2, x3\na: add x1, x2, x3"));
    }

    #[test]
    fn unknown_instruction_is_error() {
        assert!(is_error("mov x1, x2"));
    }

    #[test]
    fn invalid_register_is_error() {
        assert!(is_error("add x31, x1, x2"));
    }

    #[test]
    fn failed_validation_is_error() {
        assert!(is_error("add xzr, x1, x2"));
    }
}
