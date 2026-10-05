use crate::instruction::{Instruction, Register};

/// Encodes a 3-register instruction: opcode (11) | xm (5) | flags (6) | xn (5) | xd (5).
fn three_reg(opcode: u32, rm: u32, flags: u32, rn: u32, rd: u32) -> u32 {
    (opcode << 21) | (rm << 16) | (flags << 10) | (rn << 5) | rd
}

/// Encodes a 2-register instruction: opcode (11) | immediate (9) | 00 | xn (5) | xd (5).
fn two_reg(opcode: u32, offset: i32, rn: &Register, rd: &Register) -> u32 {
    let immediate = (offset as u32) & 0x1FF;
    (opcode << 21) | (immediate << 12) | (rn.number() << 5) | rd.number()
}

/// Encodes an opcode (8) | 19-bit immediate | 5-bit field, used by ldr and b.cond.
fn imm19(opcode: u32, offset: i32, low: u32) -> u32 {
    let immediate = (offset as u32) & 0x7FFFF;
    (opcode << 24) | (immediate << 5) | low
}

/// Compiles an instruction into machine code!
///
/// Assumes the instruction has already passed `validate()`; out-of-range
/// offsets are truncated to fit their field.
pub fn compile_line(instruction: &Instruction) -> u32 {
    match instruction {
        Instruction::Add { rd, rn, rm } => three_reg(
            0b10001011_001,
            rm.number(),
            0b011000,
            rn.number(),
            rd.number(),
        ),
        Instruction::Sub { rd, rn, rm } => three_reg(
            0b11001011_001,
            rm.number(),
            0b011000,
            rn.number(),
            rd.number(),
        ),
        Instruction::Mul { rd, rn, rm } => three_reg(
            0b10011011_000,
            rm.number(),
            0b011111,
            rn.number(),
            rd.number(),
        ),
        Instruction::Smulh { rd, rn, rm } => three_reg(
            0b10011011_010,
            rm.number(),
            0b011111,
            rn.number(),
            rd.number(),
        ),
        Instruction::Umulh { rd, rn, rm } => three_reg(
            0b10011011_110,
            rm.number(),
            0b011111,
            rn.number(),
            rd.number(),
        ),
        Instruction::Sdiv { rd, rn, rm } => three_reg(
            0b10011010_110,
            rm.number(),
            0b000011,
            rn.number(),
            rd.number(),
        ),
        Instruction::Udiv { rd, rn, rm } => three_reg(
            0b10011010_110,
            rm.number(),
            0b000010,
            rn.number(),
            rd.number(),
        ),
        Instruction::Cmp { rn, rm } => {
            three_reg(0b11101011_001, rm.number(), 0b011000, rn.number(), 0b11111)
        }
        Instruction::Br { rn } => three_reg(0b11010110_000, 0b11111, 0, rn.number(), 0),
        Instruction::Blr { rn } => three_reg(0b11010110_001, 0b11111, 0, rn.number(), 0),

        Instruction::Ldur { rd, rn, offset } => two_reg(0b11111000_010, *offset, rn, rd),
        Instruction::Stur { rd, rn, offset } => two_reg(0b11111000_000, *offset, rn, rd),

        Instruction::Ldr { rd, offset } => imm19(0b01011000, *offset, rd.number()),

        Instruction::B { offset } => (0b000101 << 26) | ((*offset as u32) & 0x3FFFFFF),
        Instruction::BCond { cond, offset } => imm19(0b01010100, *offset, cond.bits()),
    }
}

/// Bit widths of each field in an instruction's encoding, from the high bits
/// down, matching the formats in the README.
pub fn fields(instruction: &Instruction) -> &'static [u32] {
    match instruction {
        Instruction::Add { .. }
        | Instruction::Sub { .. }
        | Instruction::Mul { .. }
        | Instruction::Smulh { .. }
        | Instruction::Umulh { .. }
        | Instruction::Sdiv { .. }
        | Instruction::Udiv { .. }
        | Instruction::Cmp { .. }
        | Instruction::Br { .. }
        | Instruction::Blr { .. } => &[11, 5, 6, 5, 5],
        Instruction::Ldur { .. } | Instruction::Stur { .. } => &[11, 9, 2, 5, 5],
        Instruction::Ldr { .. } | Instruction::BCond { .. } => &[8, 19, 5],
        Instruction::B { .. } => &[6, 26],
    }
}

#[cfg(test)]
mod tests {
    use super::{compile_line, fields};
    use crate::instruction::Condition;
    use crate::instruction::Instruction::*;
    use crate::instruction::Register::*;

    #[test]
    fn three_register() {
        assert_eq!(
            compile_line(&Add {
                rd: X0,
                rn: X1,
                rm: X2
            }),
            0x8B226020
        );
        assert_eq!(
            compile_line(&Sub {
                rd: X3,
                rn: X4,
                rm: X5
            }),
            0xCB256083
        );
        assert_eq!(
            compile_line(&Mul {
                rd: X0,
                rn: X1,
                rm: X2
            }),
            0x9B027C20
        );
        assert_eq!(
            compile_line(&Smulh {
                rd: X0,
                rn: X1,
                rm: X2
            }),
            0x9B427C20
        );
        assert_eq!(
            compile_line(&Umulh {
                rd: X0,
                rn: X1,
                rm: X2
            }),
            0x9BC27C20
        );
        assert_eq!(
            compile_line(&Sdiv {
                rd: X0,
                rn: X1,
                rm: X2
            }),
            0x9AC20C20
        );
        assert_eq!(
            compile_line(&Udiv {
                rd: X0,
                rn: X1,
                rm: X2
            }),
            0x9AC20820
        );
    }

    #[test]
    fn compare_and_register_branches() {
        assert_eq!(compile_line(&Cmp { rn: X1, rm: X2 }), 0xEB22603F);
        assert_eq!(compile_line(&Br { rn: X30 }), 0xD61F03C0);
        assert_eq!(compile_line(&Blr { rn: X1 }), 0xD63F0020);
    }

    #[test]
    fn loads_and_stores() {
        assert_eq!(
            compile_line(&Ldur {
                rd: X0,
                rn: X1,
                offset: 8
            }),
            0xF8408020
        );
        assert_eq!(
            compile_line(&Ldur {
                rd: X0,
                rn: X1,
                offset: -8
            }),
            0xF85F8020
        );
        assert_eq!(
            compile_line(&Stur {
                rd: X0,
                rn: X1,
                offset: 8
            }),
            0xF8008020
        );
        assert_eq!(compile_line(&Ldr { rd: X0, offset: 2 }), 0x58000040);
    }

    #[test]
    fn branches() {
        assert_eq!(compile_line(&B { offset: 1 }), 0x14000001);
        assert_eq!(compile_line(&B { offset: -1 }), 0x17FFFFFF);
        assert_eq!(
            compile_line(&BCond {
                cond: Condition::Eq,
                offset: 2
            }),
            0x54000040
        );
        assert_eq!(
            compile_line(&BCond {
                cond: Condition::Lt,
                offset: -1
            }),
            0x54FFFFEB
        );
    }

    #[test]
    fn fields_add_up_to_32_bits() {
        let instructions = [
            Add {
                rd: X0,
                rn: X1,
                rm: X2,
            },
            Ldur {
                rd: X0,
                rn: X1,
                offset: 0,
            },
            Ldr { rd: X0, offset: 0 },
            B { offset: 0 },
        ];
        for instruction in &instructions {
            assert_eq!(fields(instruction).iter().sum::<u32>(), 32);
        }
    }
}
