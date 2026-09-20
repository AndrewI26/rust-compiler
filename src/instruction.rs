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

pub enum ValidationError {
    InvalidImmediate,
    InvalidRegister,
    InvalidInstruction,
}

impl Instruction {
    pub fn validate(&self) -> Result<(), ValidationError> {
        match self {
            Instruction::Add { rd, rn, rm } => {
                if matches!(rd, Register::XZR) {
                    return Err(ValidationError::InvalidRegister);
                }
            }

            _ => return Err(ValidationError::InvalidInstruction),
        }
        Ok(())
    }
}
