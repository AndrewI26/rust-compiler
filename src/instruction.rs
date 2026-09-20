pub enum Register {
    X0,
    X1,
    X2,
    X3,
    X4,
    X5,
    X6,
    X7,
    X8,
    X9,
    X10,
    X11,
    X12,
    X13,
    X14,
    X15,
    X16,
    X17,
    X18,
    X19,
    X20,
    X21,
    X22,
    X23,
    X24,
    X25,
    X26,
    X27,
    X28,
    X29,
    X30,
    XZR,
}

pub enum Instruction {
    Mov {
        rd: Register,
        rm: Register,
    },
    MovImmediate {
        rd: Register,
        immediate: u16,
    },
    Add {
        rd: Register,
        rn: Register,
        rm: Register,
    },
    AddImmediate {
        rd: Register,
        rn: Register,
        immediate: u16,
    },
    Sub {
        rd: Register,
        rn: Register,
        rm: Register,
    },
    SubImmediate {
        rd: Register,
        rn: Register,
        immediate: u16,
    },
    Mul {
        rd: Register,
        rn: Register,
        rm: Register,
    },
    Ldr {
        rt: Register,
        rn: Register,
    },
    LdrOffset {
        rt: Register,
        rn: Register,
        offset: i32,
    },
    Str {
        rt: Register,
        rn: Register,
    },
    StrOffset {
        rt: Register,
        rn: Register,
        offset: i32,
    },
    Cmp {
        rn: Register,
        rm: Register,
    },
    CmpImmediate {
        rn: Register,
        immediate: u16,
    },
    B {
        label: String,
    },
    Bl {
        label: String,
    },
    Beq {
        label: String,
    },
    Blt {
        label: String,
    },
    Ble {
        label: String,
    },
    Bgt {
        label: String,
    },
    Bge {
        label: String,
    },
    Ret,
    And {
        rd: Register,
        rn: Register,
        rm: Register,
    },
    Orr {
        rd: Register,
        rn: Register,
        rm: Register,
    },
    Eor {
        rd: Register,
        rn: Register,
        rm: Register,
    },
    Lsl {
        rd: Register,
        rn: Register,
        shift: u8,
    },
    Lsr {
        rd: Register,
        rn: Register,
        shift: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationError {
    InvalidImmediate,
    InvalidRegister,
    InvalidInstruction,
    InvalidOffset,
    InvalidShift,
    InvalidLabel,
}

/// Largest immediate encodable by the add/sub/cmp immediate forms (12 bits,
/// unshifted).
const MAX_ARITH_IMMEDIATE: u16 = 0xFFF;

/// Widest shift amount for a 64-bit register.
const MAX_SHIFT: u8 = 63;

/// Inclusive bounds of the signed 9-bit unscaled offset (LDUR/STUR).
const MIN_UNSCALED_OFFSET: i32 = -256;
const MAX_UNSCALED_OFFSET: i32 = 255;

/// Largest scaled offset for a 64-bit load/store (12 bits, scaled by 8).
const MAX_SCALED_OFFSET: i32 = 32760;

fn check_destination(rd: &Register) -> Result<(), ValidationError> {
    if matches!(rd, Register::XZR) {
        return Err(ValidationError::InvalidRegister);
    }
    Ok(())
}

fn check_base(rn: &Register) -> Result<(), ValidationError> {
    if matches!(rn, Register::XZR) {
        return Err(ValidationError::InvalidRegister);
    }
    Ok(())
}

fn check_arith_immediate(immediate: u16) -> Result<(), ValidationError> {
    if immediate > MAX_ARITH_IMMEDIATE {
        return Err(ValidationError::InvalidImmediate);
    }
    Ok(())
}

/// A load/store offset is encodable either unscaled (signed 9 bits) or
/// scaled (non-negative multiple of 8 up to 32760).
fn check_offset(offset: i32) -> Result<(), ValidationError> {
    let unscaled = (MIN_UNSCALED_OFFSET..=MAX_UNSCALED_OFFSET).contains(&offset);
    let scaled = offset >= 0 && offset % 8 == 0 && offset <= MAX_SCALED_OFFSET;

    if unscaled || scaled {
        Ok(())
    } else {
        Err(ValidationError::InvalidOffset)
    }
}

fn check_shift(shift: u8) -> Result<(), ValidationError> {
    if shift > MAX_SHIFT {
        return Err(ValidationError::InvalidShift);
    }
    Ok(())
}

fn check_label(label: &str) -> Result<(), ValidationError> {
    let mut chars = label.chars();

    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' || first == '.' => {}
        _ => return Err(ValidationError::InvalidLabel),
    }

    if chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') {
        Ok(())
    } else {
        Err(ValidationError::InvalidLabel)
    }
}

impl Instruction {
    pub fn validate(&self) -> Result<(), ValidationError> {
        match self {
            Instruction::Mov { rd, rm: _ } => check_destination(rd)?,

            Instruction::MovImmediate { rd, immediate: _ } => check_destination(rd)?,

            Instruction::Add { rd, rn: _, rm: _ }
            | Instruction::Sub { rd, rn: _, rm: _ }
            | Instruction::Mul { rd, rn: _, rm: _ }
            | Instruction::And { rd, rn: _, rm: _ }
            | Instruction::Orr { rd, rn: _, rm: _ }
            | Instruction::Eor { rd, rn: _, rm: _ } => check_destination(rd)?,

            Instruction::AddImmediate {
                rd,
                rn: _,
                immediate,
            }
            | Instruction::SubImmediate {
                rd,
                rn: _,
                immediate,
            } => {
                check_destination(rd)?;
                check_arith_immediate(*immediate)?;
            }

            Instruction::Ldr { rt, rn } => {
                check_destination(rt)?;
                check_base(rn)?;
            }

            Instruction::LdrOffset { rt, rn, offset } => {
                check_destination(rt)?;
                check_base(rn)?;
                check_offset(*offset)?;
            }

            Instruction::Str { rt: _, rn } => check_base(rn)?,

            Instruction::StrOffset { rt: _, rn, offset } => {
                check_base(rn)?;
                check_offset(*offset)?;
            }

            Instruction::Cmp { rn: _, rm: _ } => {}

            Instruction::CmpImmediate { rn: _, immediate } => check_arith_immediate(*immediate)?,

            Instruction::B { label }
            | Instruction::Bl { label }
            | Instruction::Beq { label }
            | Instruction::Blt { label }
            | Instruction::Ble { label }
            | Instruction::Bgt { label }
            | Instruction::Bge { label } => check_label(label)?,

            Instruction::Ret => {}

            Instruction::Lsl { rd, rn: _, shift } | Instruction::Lsr { rd, rn: _, shift } => {
                check_destination(rd)?;
                check_shift(*shift)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_xzr_destination() {
        let instruction = Instruction::Add {
            rd: Register::XZR,
            rn: Register::X1,
            rm: Register::X2,
        };
        assert_eq!(
            instruction.validate(),
            Err(ValidationError::InvalidRegister)
        );
    }

    #[test]
    fn accepts_str_from_xzr() {
        let instruction = Instruction::Str {
            rt: Register::XZR,
            rn: Register::X1,
        };
        assert_eq!(instruction.validate(), Ok(()));
    }

    #[test]
    fn rejects_wide_arith_immediate() {
        let instruction = Instruction::AddImmediate {
            rd: Register::X0,
            rn: Register::X1,
            immediate: 4096,
        };
        assert_eq!(
            instruction.validate(),
            Err(ValidationError::InvalidImmediate)
        );
    }

    #[test]
    fn accepts_full_width_mov_immediate() {
        let instruction = Instruction::MovImmediate {
            rd: Register::X0,
            immediate: u16::MAX,
        };
        assert_eq!(instruction.validate(), Ok(()));
    }

    #[test]
    fn offsets() {
        let offset = |offset| {
            Instruction::LdrOffset {
                rt: Register::X0,
                rn: Register::X1,
                offset,
            }
            .validate()
        };

        assert_eq!(offset(-16), Ok(()));
        assert_eq!(offset(255), Ok(()));
        assert_eq!(offset(32760), Ok(()));
        assert_eq!(offset(-257), Err(ValidationError::InvalidOffset));
        assert_eq!(offset(257), Err(ValidationError::InvalidOffset));
        assert_eq!(offset(32768), Err(ValidationError::InvalidOffset));
    }

    #[test]
    fn shifts() {
        let shift = |shift| {
            Instruction::Lsl {
                rd: Register::X0,
                rn: Register::X1,
                shift,
            }
            .validate()
        };

        assert_eq!(shift(63), Ok(()));
        assert_eq!(shift(64), Err(ValidationError::InvalidShift));
    }

    #[test]
    fn labels() {
        let label = |label: &str| {
            Instruction::B {
                label: label.to_string(),
            }
            .validate()
        };

        assert_eq!(label("loop_start"), Ok(()));
        assert_eq!(label(".L1"), Ok(()));
        assert_eq!(label(""), Err(ValidationError::InvalidLabel));
        assert_eq!(label("1loop"), Err(ValidationError::InvalidLabel));
        assert_eq!(label("bad label"), Err(ValidationError::InvalidLabel));
    }
}
