#[derive(Clone, Copy)]
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

impl Register {
    /// The register's 5-bit number in machine code (x0 = 0 ... x30 = 30, xzr = 31).
    pub fn number(&self) -> u32 {
        *self as u32
    }
}

/// Condition for `b.cond`.
pub enum Condition {
    Eq,
    Ne,
    Hs,
    Lo,
    Hi,
    Ls,
    Ge,
    Lt,
    Gt,
    Le,
}

impl Condition {
    /// The condition's 5-bit code in machine code.
    pub fn bits(&self) -> u32 {
        match self {
            Condition::Eq => 0b00000,
            Condition::Ne => 0b00001,
            Condition::Hs => 0b00010,
            Condition::Lo => 0b00011,
            Condition::Hi => 0b01000,
            Condition::Ls => 0b01001,
            Condition::Ge => 0b01010,
            Condition::Lt => 0b01011,
            Condition::Gt => 0b01100,
            Condition::Le => 0b01101,
        }
    }
}

/// One instruction from the README. Offsets for `ldr`, `b` and `b.cond` are
/// counted in instructions (multiplied by 4 to get bytes), so labels must be
/// resolved before building these.
pub enum Instruction {
    // 3-register format
    Add {
        rd: Register,
        rn: Register,
        rm: Register,
    },
    Sub {
        rd: Register,
        rn: Register,
        rm: Register,
    },
    Mul {
        rd: Register,
        rn: Register,
        rm: Register,
    },
    Smulh {
        rd: Register,
        rn: Register,
        rm: Register,
    },
    Umulh {
        rd: Register,
        rn: Register,
        rm: Register,
    },
    Sdiv {
        rd: Register,
        rn: Register,
        rm: Register,
    },
    Udiv {
        rd: Register,
        rn: Register,
        rm: Register,
    },
    Cmp {
        rn: Register,
        rm: Register,
    },
    Br {
        rn: Register,
    },
    Blr {
        rn: Register,
    },

    // 2-register format
    Ldur {
        rd: Register,
        rn: Register,
        offset: i32,
    },
    Stur {
        rd: Register,
        rn: Register,
        offset: i32,
    },

    // 1-register format
    Ldr {
        rd: Register,
        offset: i32,
    },

    // Branching format
    B {
        offset: i32,
    },
    BCond {
        cond: Condition,
        offset: i32,
    },
}

#[derive(Debug, PartialEq)]
pub enum ValidationError {
    InvalidRegister,
    InvalidOffset,
}

/// Signed 9-bit byte offset for ldur/stur.
const MIN_UNSCALED_OFFSET: i32 = -(1 << 8);
const MAX_UNSCALED_OFFSET: i32 = (1 << 8) - 1;

/// Signed 19-bit instruction offset for ldr and b.cond.
const MIN_OFFSET_19: i32 = -(1 << 18);
const MAX_OFFSET_19: i32 = (1 << 18) - 1;

/// Signed 26-bit instruction offset for b.
const MIN_OFFSET_26: i32 = -(1 << 25);
const MAX_OFFSET_26: i32 = (1 << 25) - 1;

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

fn check_offset(offset: i32, min: i32, max: i32) -> Result<(), ValidationError> {
    if !(min..=max).contains(&offset) {
        return Err(ValidationError::InvalidOffset);
    }
    Ok(())
}

impl Instruction {
    pub fn validate(&self) -> Result<(), ValidationError> {
        match self {
            Instruction::Add { rd, .. }
            | Instruction::Sub { rd, .. }
            | Instruction::Mul { rd, .. }
            | Instruction::Smulh { rd, .. }
            | Instruction::Umulh { rd, .. }
            | Instruction::Sdiv { rd, .. }
            | Instruction::Udiv { rd, .. } => check_destination(rd)?,

            Instruction::Cmp { .. } | Instruction::Br { .. } | Instruction::Blr { .. } => {}

            Instruction::Ldur { rd, rn, offset } => {
                check_destination(rd)?;
                check_base(rn)?;
                check_offset(*offset, MIN_UNSCALED_OFFSET, MAX_UNSCALED_OFFSET)?;
            }

            Instruction::Stur { rd: _, rn, offset } => {
                check_base(rn)?;
                check_offset(*offset, MIN_UNSCALED_OFFSET, MAX_UNSCALED_OFFSET)?;
            }

            Instruction::Ldr { rd, offset } => {
                check_destination(rd)?;
                check_offset(*offset, MIN_OFFSET_19, MAX_OFFSET_19)?;
            }

            Instruction::B { offset } => check_offset(*offset, MIN_OFFSET_26, MAX_OFFSET_26)?,

            Instruction::BCond { cond: _, offset } => {
                check_offset(*offset, MIN_OFFSET_19, MAX_OFFSET_19)?
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_numbers() {
        assert_eq!(Register::X0.number(), 0);
        assert_eq!(Register::X30.number(), 30);
        assert_eq!(Register::XZR.number(), 31);
    }

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
    fn accepts_stur_from_xzr() {
        let instruction = Instruction::Stur {
            rd: Register::XZR,
            rn: Register::X1,
            offset: 0,
        };
        assert_eq!(instruction.validate(), Ok(()));
    }

    #[test]
    fn unscaled_offsets() {
        let offset = |offset| {
            Instruction::Ldur {
                rd: Register::X0,
                rn: Register::X1,
                offset,
            }
            .validate()
        };

        assert_eq!(offset(-256), Ok(()));
        assert_eq!(offset(255), Ok(()));
        assert_eq!(offset(-257), Err(ValidationError::InvalidOffset));
        assert_eq!(offset(256), Err(ValidationError::InvalidOffset));
    }

    #[test]
    fn branch_offsets() {
        assert_eq!(
            Instruction::B {
                offset: MAX_OFFSET_26
            }
            .validate(),
            Ok(())
        );
        assert_eq!(
            Instruction::B {
                offset: MAX_OFFSET_26 + 1
            }
            .validate(),
            Err(ValidationError::InvalidOffset)
        );

        let b_cond = |offset| {
            Instruction::BCond {
                cond: Condition::Eq,
                offset,
            }
            .validate()
        };
        assert_eq!(b_cond(MIN_OFFSET_19), Ok(()));
        assert_eq!(
            b_cond(MIN_OFFSET_19 - 1),
            Err(ValidationError::InvalidOffset)
        );
    }
}
