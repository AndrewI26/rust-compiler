use crate::instruction::{Instruction, Register};

pub enum ParseError {
    SyntaxError,
}

pub fn parse_line(line: &String) -> Result<Instruction, ParseError> {
    let split_line: Vec<&str> = line.split(" ").collect();
    if split_line.is_empty() || split_line[0].is_empty() {
        return Err(ParseError::SyntaxError);
    };

    let instruction_type = split_line[0].to_lowercase();

    match instruction_type.as_str() {
        "mov" => {
            let [rd, source] = operands(&split_line)?;
            if source.starts_with('#') {
                Ok(Instruction::MovImmediate {
                    rd: register(rd)?,
                    immediate: immediate(source)?,
                })
            } else {
                Ok(Instruction::Mov {
                    rd: register(rd)?,
                    rm: register(source)?,
                })
            }
        }
        "add" => {
            let [rd, rn, source] = operands(&split_line)?;
            if source.starts_with('#') {
                Ok(Instruction::AddImmediate {
                    rd: register(rd)?,
                    rn: register(rn)?,
                    immediate: immediate(source)?,
                })
            } else {
                Ok(Instruction::Add {
                    rd: register(rd)?,
                    rn: register(rn)?,
                    rm: register(source)?,
                })
            }
        }
        "sub" => {
            let [rd, rn, source] = operands(&split_line)?;
            if source.starts_with('#') {
                Ok(Instruction::SubImmediate {
                    rd: register(rd)?,
                    rn: register(rn)?,
                    immediate: immediate(source)?,
                })
            } else {
                Ok(Instruction::Sub {
                    rd: register(rd)?,
                    rn: register(rn)?,
                    rm: register(source)?,
                })
            }
        }
        "mul" => {
            let [rd, rn, rm] = operands(&split_line)?;
            Ok(Instruction::Mul {
                rd: register(rd)?,
                rn: register(rn)?,
                rm: register(rm)?,
            })
        }
        "ldr" => {
            if let Ok([rt, rn, offset]) = operands(&split_line) {
                Ok(Instruction::LdrOffset {
                    rt: register(rt)?,
                    rn: register(rn)?,
                    offset: immediate(offset)?,
                })
            } else {
                let [rt, rn] = operands(&split_line)?;
                Ok(Instruction::Ldr {
                    rt: register(rt)?,
                    rn: register(rn)?,
                })
            }
        }
        "str" => {
            if let Ok([rt, rn, offset]) = operands(&split_line) {
                Ok(Instruction::StrOffset {
                    rt: register(rt)?,
                    rn: register(rn)?,
                    offset: immediate(offset)?,
                })
            } else {
                let [rt, rn] = operands(&split_line)?;
                Ok(Instruction::Str {
                    rt: register(rt)?,
                    rn: register(rn)?,
                })
            }
        }
        "cmp" => {
            let [rn, source] = operands(&split_line)?;
            if source.starts_with('#') {
                Ok(Instruction::CmpImmediate {
                    rn: register(rn)?,
                    immediate: immediate(source)?,
                })
            } else {
                Ok(Instruction::Cmp {
                    rn: register(rn)?,
                    rm: register(source)?,
                })
            }
        }
        "b" => {
            let [label] = operands(&split_line)?;
            Ok(Instruction::B {
                label: label.to_string(),
            })
        }
        "bl" => {
            let [label] = operands(&split_line)?;
            Ok(Instruction::Bl {
                label: label.to_string(),
            })
        }
        "b.eq" => {
            let [label] = operands(&split_line)?;
            Ok(Instruction::Beq {
                label: label.to_string(),
            })
        }
        "b.lt" => {
            let [label] = operands(&split_line)?;
            Ok(Instruction::Blt {
                label: label.to_string(),
            })
        }
        "b.le" => {
            let [label] = operands(&split_line)?;
            Ok(Instruction::Ble {
                label: label.to_string(),
            })
        }
        "b.gt" => {
            let [label] = operands(&split_line)?;
            Ok(Instruction::Bgt {
                label: label.to_string(),
            })
        }
        "b.ge" => {
            let [label] = operands(&split_line)?;
            Ok(Instruction::Bge {
                label: label.to_string(),
            })
        }
        "ret" => {
            let [] = operands(&split_line)?;
            Ok(Instruction::Ret)
        }
        "and" => {
            let [rd, rn, rm] = operands(&split_line)?;
            Ok(Instruction::And {
                rd: register(rd)?,
                rn: register(rn)?,
                rm: register(rm)?,
            })
        }
        "orr" => {
            let [rd, rn, rm] = operands(&split_line)?;
            Ok(Instruction::Orr {
                rd: register(rd)?,
                rn: register(rn)?,
                rm: register(rm)?,
            })
        }
        "eor" => {
            let [rd, rn, rm] = operands(&split_line)?;
            Ok(Instruction::Eor {
                rd: register(rd)?,
                rn: register(rn)?,
                rm: register(rm)?,
            })
        }
        "lsl" => {
            let [rd, rn, shift] = operands(&split_line)?;
            Ok(Instruction::Lsl {
                rd: register(rd)?,
                rn: register(rn)?,
                shift: immediate(shift)?,
            })
        }
        "lsr" => {
            let [rd, rn, shift] = operands(&split_line)?;
            Ok(Instruction::Lsr {
                rd: register(rd)?,
                rn: register(rn)?,
                shift: immediate(shift)?,
            })
        }
        _ => Err(ParseError::SyntaxError),
    }
}

/// Strips the commas and brackets from the words after the mnemonic, so
/// `ldr x0, [x1, #8]` gives `["x0", "x1", "#8"]`.
fn operands<'a, const N: usize>(split_line: &[&'a str]) -> Result<[&'a str; N], ParseError> {
    let operands: Vec<&str> = split_line[1..]
        .iter()
        .map(|word| word.trim_matches(|c| c == ',' || c == '[' || c == ']'))
        .filter(|word| !word.is_empty())
        .collect();

    operands.try_into().map_err(|_| ParseError::SyntaxError)
}

fn register(operand: &str) -> Result<Register, ParseError> {
    const REGISTERS: [Register; 32] = [
        Register::X0,
        Register::X1,
        Register::X2,
        Register::X3,
        Register::X4,
        Register::X5,
        Register::X6,
        Register::X7,
        Register::X8,
        Register::X9,
        Register::X10,
        Register::X11,
        Register::X12,
        Register::X13,
        Register::X14,
        Register::X15,
        Register::X16,
        Register::X17,
        Register::X18,
        Register::X19,
        Register::X20,
        Register::X21,
        Register::X22,
        Register::X23,
        Register::X24,
        Register::X25,
        Register::X26,
        Register::X27,
        Register::X28,
        Register::X29,
        Register::X30,
        Register::XZR,
    ];

    let name = operand.to_lowercase();
    if name == "xzr" {
        return Ok(Register::XZR);
    }

    name.strip_prefix('x')
        .and_then(|number| number.parse::<usize>().ok())
        .filter(|&index| index < 31)
        .map(|index| REGISTERS[index])
        .ok_or(ParseError::SyntaxError)
}

/// Parses `#42`, `#-8` or `#0x2a` into whatever integer type the field uses.
fn immediate<T: TryFrom<i64>>(operand: &str) -> Result<T, ParseError> {
    let literal = operand.strip_prefix('#').ok_or(ParseError::SyntaxError)?;
    let (negative, digits) = match literal.strip_prefix('-') {
        Some(digits) => (true, digits),
        None => (false, literal),
    };

    let magnitude = match digits.strip_prefix("0x") {
        Some(hex) => i64::from_str_radix(hex, 16),
        None => digits.parse(),
    }
    .map_err(|_| ParseError::SyntaxError)?;

    let value = if negative { -magnitude } else { magnitude };
    T::try_from(value).map_err(|_| ParseError::SyntaxError)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> Result<Instruction, ParseError> {
        parse_line(&line.to_string())
    }

    #[test]
    fn mov() {
        assert!(matches!(
            parse("mov x0, x1"),
            Ok(Instruction::Mov {
                rd: Register::X0,
                rm: Register::X1
            })
        ));
        assert!(matches!(
            parse("mov x0, #42"),
            Ok(Instruction::MovImmediate {
                rd: Register::X0,
                immediate: 42
            })
        ));
    }

    #[test]
    fn arithmetic() {
        assert!(matches!(
            parse("add x0, x1, x2"),
            Ok(Instruction::Add {
                rd: Register::X0,
                rn: Register::X1,
                rm: Register::X2
            })
        ));
        assert!(matches!(
            parse("add x0, x1, #0x10"),
            Ok(Instruction::AddImmediate {
                rd: Register::X0,
                rn: Register::X1,
                immediate: 16
            })
        ));
        assert!(matches!(
            parse("sub x3, x4, x5"),
            Ok(Instruction::Sub {
                rd: Register::X3,
                rn: Register::X4,
                rm: Register::X5
            })
        ));
        assert!(matches!(
            parse("sub x3, x4, #1"),
            Ok(Instruction::SubImmediate {
                rd: Register::X3,
                rn: Register::X4,
                immediate: 1
            })
        ));
        assert!(matches!(
            parse("mul x0, x1, x2"),
            Ok(Instruction::Mul {
                rd: Register::X0,
                rn: Register::X1,
                rm: Register::X2
            })
        ));
    }

    #[test]
    fn loads_and_stores() {
        assert!(matches!(
            parse("ldr x0, [x1]"),
            Ok(Instruction::Ldr {
                rt: Register::X0,
                rn: Register::X1
            })
        ));
        assert!(matches!(
            parse("ldr x0, [x1, #8]"),
            Ok(Instruction::LdrOffset {
                rt: Register::X0,
                rn: Register::X1,
                offset: 8
            })
        ));
        assert!(matches!(
            parse("str xzr, [x29]"),
            Ok(Instruction::Str {
                rt: Register::XZR,
                rn: Register::X29
            })
        ));
        assert!(matches!(
            parse("str x0, [x1, #-16]"),
            Ok(Instruction::StrOffset {
                rt: Register::X0,
                rn: Register::X1,
                offset: -16
            })
        ));
    }

    #[test]
    fn compare() {
        assert!(matches!(
            parse("cmp x0, x1"),
            Ok(Instruction::Cmp {
                rn: Register::X0,
                rm: Register::X1
            })
        ));
        assert!(matches!(
            parse("cmp x0, #5"),
            Ok(Instruction::CmpImmediate {
                rn: Register::X0,
                immediate: 5
            })
        ));
    }

    #[test]
    fn branches() {
        assert!(matches!(parse("b loop"), Ok(Instruction::B { label }) if label == "loop"));
        assert!(matches!(parse("bl func"), Ok(Instruction::Bl { label }) if label == "func"));
        assert!(matches!(parse("b.eq end"), Ok(Instruction::Beq { label }) if label == "end"));
        assert!(matches!(parse("b.lt end"), Ok(Instruction::Blt { label }) if label == "end"));
        assert!(matches!(parse("b.le end"), Ok(Instruction::Ble { label }) if label == "end"));
        assert!(matches!(parse("b.gt end"), Ok(Instruction::Bgt { label }) if label == "end"));
        assert!(matches!(parse("b.ge end"), Ok(Instruction::Bge { label }) if label == "end"));
    }

    #[test]
    fn ret() {
        assert!(matches!(parse("ret"), Ok(Instruction::Ret)));
        assert!(matches!(parse("ret x0"), Err(ParseError::SyntaxError)));
    }

    #[test]
    fn logic_and_shifts() {
        assert!(matches!(
            parse("and x0, x1, x2"),
            Ok(Instruction::And {
                rd: Register::X0,
                rn: Register::X1,
                rm: Register::X2
            })
        ));
        assert!(matches!(
            parse("orr x0, x1, x2"),
            Ok(Instruction::Orr {
                rd: Register::X0,
                rn: Register::X1,
                rm: Register::X2
            })
        ));
        assert!(matches!(
            parse("eor x0, x1, x2"),
            Ok(Instruction::Eor {
                rd: Register::X0,
                rn: Register::X1,
                rm: Register::X2
            })
        ));
        assert!(matches!(
            parse("lsl x0, x1, #4"),
            Ok(Instruction::Lsl {
                rd: Register::X0,
                rn: Register::X1,
                shift: 4
            })
        ));
        assert!(matches!(
            parse("lsr x0, x1, #63"),
            Ok(Instruction::Lsr {
                rd: Register::X0,
                rn: Register::X1,
                shift: 63
            })
        ));
    }

    #[test]
    fn mnemonic_is_case_insensitive() {
        assert!(matches!(
            parse("ADD X0, X1, X2"),
            Ok(Instruction::Add {
                rd: Register::X0,
                rn: Register::X1,
                rm: Register::X2
            })
        ));
    }

    #[test]
    fn rejects_bad_input() {
        let bad = [
            "",
            "nop",
            "add x0, x1",
            "add x0, x1, x2, x3",
            "add x0, x1, x31",
            "add x0, y1, x2",
            "mov x0, #70000",
            "mov x0, #abc",
            "lsl x0, x1, #256",
            "b",
            "b one two",
        ];

        for line in bad {
            assert!(
                matches!(parse(line), Err(ParseError::SyntaxError)),
                "expected {line:?} to be rejected"
            );
        }
    }
}
