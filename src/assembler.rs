use std::collections::HashMap;

use crate::instruction::{self, Instruction, Register, ValidationError};

/// Condition codes used by B.cond.
const COND_EQ: u32 = 0x0;
const COND_GE: u32 = 0xA;
const COND_LT: u32 = 0xB;
const COND_GT: u32 = 0xC;
const COND_LE: u32 = 0xD;

/// Field widths for PC-relative branch offsets (in instructions).
const B_OFFSET_BITS: u32 = 26;
const B_COND_OFFSET_BITS: u32 = 19;

/// Register number as encoded in the instruction (XZR/SP is 31).
fn reg(register: Register) -> u32 {
    register as u32
}

/// In the immediate add/sub forms register 31 means SP, not XZR, so an XZR
/// operand there can't be encoded.
fn check_not_sp_slot(register: Register) -> Result<(), ValidationError> {
    if register == Register::XZR {
        return Err(ValidationError::InvalidRegister);
    }
    Ok(())
}

/// Resolves `label` to a word offset from `address` and checks it fits in a
/// signed field of `bits` bits. Returns the offset masked to that width.
fn branch_offset(
    label: &str,
    address: u64,
    labels: &HashMap<String, u64>,
    bits: u32,
) -> Result<u32, ValidationError> {
    let target = *labels.get(label).ok_or(ValidationError::InvalidLabel)?;
    let delta = target as i64 - address as i64;

    if delta % 4 != 0 {
        return Err(ValidationError::InvalidOffset);
    }

    let words = delta / 4;
    let limit = 1i64 << (bits - 1);
    if words < -limit || words >= limit {
        return Err(ValidationError::InvalidOffset);
    }

    Ok((words as u32) & ((1 << bits) - 1))
}

/// Encodes a load/store with an immediate offset, picking the scaled
/// unsigned-offset form (LDR/STR) when possible and falling back to the
/// unscaled signed form (LDUR/STUR).
fn load_store(
    scaled_opcode: u32,
    unscaled_opcode: u32,
    rt: Register,
    rn: Register,
    offset: i32,
) -> u32 {
    if offset >= 0 && offset % 8 == 0 && offset / 8 <= 0xFFF {
        scaled_opcode | ((offset as u32 / 8) << 10) | (reg(rn) << 5) | reg(rt)
    } else {
        unscaled_opcode | (((offset as u32) & 0x1FF) << 12) | (reg(rn) << 5) | reg(rt)
    }
}

/// Encodes a single instruction as a 32-bit AArch64 machine word.
///
/// `address` is the byte address this instruction will live at and `labels`
/// maps label names to byte addresses; both are only needed to resolve branch
/// targets. Use `u32::to_le_bytes` on the result to get the bytes as they
/// appear in memory.
pub fn compile_line(
    instruction: &Instruction,
    address: u64,
    labels: &HashMap<String, u64>,
) -> Result<u32, ValidationError> {
    instruction.validate()?;

    let word = match *instruction {
        Instruction::Mov { rd, rm } => 0xAA0003E0 | (reg(rm) << 16) | reg(rd),

        Instruction::MovImmediate { rd, immediate } => {
            0xD2800000 | ((immediate as u32) << 5) | reg(rd)
        }

        Instruction::Add { rd, rn, rm } => 0x8B000000 | (reg(rm) << 16) | (reg(rn) << 5) | reg(rd),

        Instruction::AddImmediate { rd, rn, immediate } => {
            check_not_sp_slot(rn)?;
            0x91000000 | ((immediate as u32) << 10) | (reg(rn) << 5) | reg(rd)
        }

        Instruction::Sub { rd, rn, rm } => 0xCB000000 | (reg(rm) << 16) | (reg(rn) << 5) | reg(rd),

        Instruction::SubImmediate { rd, rn, immediate } => {
            check_not_sp_slot(rn)?;
            0xD1000000 | ((immediate as u32) << 10) | (reg(rn) << 5) | reg(rd)
        }

        Instruction::Mul { rd, rn, rm } => 0x9B007C00 | (reg(rm) << 16) | (reg(rn) << 5) | reg(rd),

        Instruction::Ldr { rt, rn } => load_store(0xF9400000, 0xF8400000, rt, rn, 0),
        Instruction::LdrOffset { rt, rn, offset } => {
            load_store(0xF9400000, 0xF8400000, rt, rn, offset)
        }

        Instruction::Str { rt, rn } => load_store(0xF9000000, 0xF8000000, rt, rn, 0),
        Instruction::StrOffset { rt, rn, offset } => {
            load_store(0xF9000000, 0xF8000000, rt, rn, offset)
        }

        Instruction::Cmp { rn, rm } => 0xEB00001F | (reg(rm) << 16) | (reg(rn) << 5),

        Instruction::CmpImmediate { rn, immediate } => {
            check_not_sp_slot(rn)?;
            0xF100001F | ((immediate as u32) << 10) | (reg(rn) << 5)
        }

        Instruction::B { ref label } => {
            0x14000000 | branch_offset(label, address, labels, B_OFFSET_BITS)?
        }
        Instruction::Bl { ref label } => {
            0x94000000 | branch_offset(label, address, labels, B_OFFSET_BITS)?
        }

        Instruction::Beq { ref label } => b_cond(COND_EQ, label, address, labels)?,
        Instruction::Blt { ref label } => b_cond(COND_LT, label, address, labels)?,
        Instruction::Ble { ref label } => b_cond(COND_LE, label, address, labels)?,
        Instruction::Bgt { ref label } => b_cond(COND_GT, label, address, labels)?,
        Instruction::Bge { ref label } => b_cond(COND_GE, label, address, labels)?,

        Instruction::Ret => 0xD65F03C0,

        Instruction::And { rd, rn, rm } => 0x8A000000 | (reg(rm) << 16) | (reg(rn) << 5) | reg(rd),
        Instruction::Orr { rd, rn, rm } => 0xAA000000 | (reg(rm) << 16) | (reg(rn) << 5) | reg(rd),
        Instruction::Eor { rd, rn, rm } => 0xCA000000 | (reg(rm) << 16) | (reg(rn) << 5) | reg(rd),

        Instruction::Lsl { rd, rn, shift } => {
            let immr = (64 - shift as u32) % 64;
            let imms = 63 - shift as u32;
            ubfm(rd, rn, immr, imms)
        }

        Instruction::Lsr { rd, rn, shift } => ubfm(rd, rn, shift as u32, 63),
    };

    Ok(word)
}

fn b_cond(
    cond: u32,
    label: &str,
    address: u64,
    labels: &HashMap<String, u64>,
) -> Result<u32, ValidationError> {
    let offset = branch_offset(label, address, labels, B_COND_OFFSET_BITS)?;
    Ok(0x54000000 | (offset << 5) | cond)
}

fn ubfm(rd: Register, rn: Register, immr: u32, imms: u32) -> u32 {
    0xD3400000 | (immr << 16) | (imms << 10) | (reg(rn) << 5) | reg(rd)
}

fn parse_line(line: &String) -> Result<Instruction, String> {
    let (opcode, operands) = line
        .split_once(' ')
        .ok_or("Expected instruction operands")?;

    match opcode.to_lowercase() {
        "add" => Ok(Instruction::Add { rd: 1, rn: 2, rm: 3 })
        _ => Err(!fmt("Unknown opcode: "))
    }

    return Ok(Instruction::Ret);
}

#[cfg(test)]
mod tests {
    use super::*;
    use Register::*;

    fn encode(instruction: Instruction) -> u32 {
        compile_line(&instruction, 0, &HashMap::new()).unwrap()
    }

    fn encode_branch(instruction: Instruction, address: u64, target: u64) -> u32 {
        let labels = HashMap::from([("target".to_string(), target)]);
        compile_line(&instruction, address, &labels).unwrap()
    }

    fn target() -> String {
        "target".to_string()
    }

    // Expected values were checked against `as` / `objdump` output.
    #[test]
    fn data_processing() {
        assert_eq!(encode(Instruction::Mov { rd: X0, rm: X1 }), 0xAA0103E0);
        assert_eq!(
            encode(Instruction::MovImmediate {
                rd: X0,
                immediate: 42
            }),
            0xD2800540
        );
        assert_eq!(
            encode(Instruction::Add {
                rd: X0,
                rn: X1,
                rm: X2
            }),
            0x8B020020
        );
        assert_eq!(
            encode(Instruction::AddImmediate {
                rd: X0,
                rn: X1,
                immediate: 1
            }),
            0x91000420
        );
        assert_eq!(
            encode(Instruction::Sub {
                rd: X0,
                rn: X1,
                rm: X2
            }),
            0xCB020020
        );
        assert_eq!(
            encode(Instruction::SubImmediate {
                rd: X0,
                rn: X1,
                immediate: 1
            }),
            0xD1000420
        );
        assert_eq!(
            encode(Instruction::Mul {
                rd: X0,
                rn: X1,
                rm: X2
            }),
            0x9B027C20
        );
        assert_eq!(encode(Instruction::Cmp { rn: X0, rm: X1 }), 0xEB01001F);
        assert_eq!(
            encode(Instruction::CmpImmediate {
                rn: X0,
                immediate: 5
            }),
            0xF100141F
        );
        assert_eq!(
            encode(Instruction::And {
                rd: X0,
                rn: X1,
                rm: X2
            }),
            0x8A020020
        );
        assert_eq!(
            encode(Instruction::Orr {
                rd: X0,
                rn: X1,
                rm: X2
            }),
            0xAA020020
        );
        assert_eq!(
            encode(Instruction::Eor {
                rd: X0,
                rn: X1,
                rm: X2
            }),
            0xCA020020
        );
        assert_eq!(
            encode(Instruction::Lsl {
                rd: X0,
                rn: X1,
                shift: 4
            }),
            0xD37CEC20
        );
        assert_eq!(
            encode(Instruction::Lsr {
                rd: X0,
                rn: X1,
                shift: 4
            }),
            0xD344FC20
        );
        assert_eq!(encode(Instruction::Ret), 0xD65F03C0);
    }

    #[test]
    fn loads_and_stores() {
        assert_eq!(encode(Instruction::Ldr { rt: X0, rn: X1 }), 0xF9400020);
        assert_eq!(
            encode(Instruction::LdrOffset {
                rt: X0,
                rn: X1,
                offset: 8
            }),
            0xF9400420
        );
        assert_eq!(
            encode(Instruction::LdrOffset {
                rt: X0,
                rn: X1,
                offset: -8
            }),
            0xF85F8020
        );
        assert_eq!(encode(Instruction::Str { rt: X0, rn: X1 }), 0xF9000020);
        assert_eq!(
            encode(Instruction::StrOffset {
                rt: X0,
                rn: X1,
                offset: 16
            }),
            0xF9000820
        );
        assert_eq!(
            encode(Instruction::StrOffset {
                rt: XZR,
                rn: X1,
                offset: 3
            }),
            0xF800303F
        );
    }

    #[test]
    fn branches() {
        assert_eq!(
            encode_branch(Instruction::B { label: target() }, 0, 8),
            0x14000002
        );
        assert_eq!(
            encode_branch(Instruction::B { label: target() }, 4, 0),
            0x17FFFFFF
        );
        assert_eq!(
            encode_branch(Instruction::Bl { label: target() }, 0, 8),
            0x94000002
        );
        assert_eq!(
            encode_branch(Instruction::Beq { label: target() }, 0, 8),
            0x54000040
        );
        assert_eq!(
            encode_branch(Instruction::Blt { label: target() }, 0, 8),
            0x5400004B
        );
        assert_eq!(
            encode_branch(Instruction::Ble { label: target() }, 0, 8),
            0x5400004D
        );
        assert_eq!(
            encode_branch(Instruction::Bgt { label: target() }, 0, 8),
            0x5400004C
        );
        assert_eq!(
            encode_branch(Instruction::Bge { label: target() }, 8, 0),
            0x54FFFFCA
        );
    }

    #[test]
    fn rejects_undefined_label() {
        let result = compile_line(&Instruction::B { label: target() }, 0, &HashMap::new());
        assert_eq!(result, Err(ValidationError::InvalidLabel));
    }

    #[test]
    fn rejects_out_of_range_conditional_branch() {
        let labels = HashMap::from([("target".to_string(), 1 << 20)]);
        let result = compile_line(&Instruction::Beq { label: target() }, 0, &labels);
        assert_eq!(result, Err(ValidationError::InvalidOffset));
    }

    #[test]
    fn rejects_xzr_in_sp_slot() {
        let result = compile_line(
            &Instruction::AddImmediate {
                rd: X0,
                rn: XZR,
                immediate: 1,
            },
            0,
            &HashMap::new(),
        );
        assert_eq!(result, Err(ValidationError::InvalidRegister));
    }
}
