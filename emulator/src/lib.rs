pub const OUTPUT_ADDRESS: u64 = 0xFFFF_000C;
pub const INPUT_ADDRESS: u64 = 0xFFFF_0004;
pub const RETURN_ADDRESS: u64 = 0xFFFF_FFFF_0000_0000;

pub struct Outcome {
    pub output: String,
    pub exit_code: Option<i64>,
    pub error: Option<String>,
    pub steps: u64,
    pub registers: [u64; 32],
}

struct Flags {
    negative: bool,
    zero: bool,
    carry: bool,
    overflow: bool,
}

pub struct Machine {
    registers: [u64; 32],
    pc: u64,
    flags: Flags,
    memory: Vec<u8>,
    input: Vec<u8>,
    input_position: usize,
    output: Vec<u8>,
    steps: u64,
}

impl Machine {
    pub fn new(words: &[u32], memory_size: usize, input: &str) -> Result<Machine, String> {
        if words.len() * 4 > memory_size {
            return Err(String::from("Program does not fit in memory"));
        }
        let mut memory = vec![0; memory_size];
        for (i, word) in words.iter().enumerate() {
            memory[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        let mut registers = [0; 32];
        registers[30] = RETURN_ADDRESS;
        Ok(Machine {
            registers,
            pc: 0,
            flags: Flags {
                negative: false,
                zero: false,
                carry: false,
                overflow: false,
            },
            memory,
            input: input.as_bytes().to_vec(),
            input_position: 0,
            output: Vec::new(),
            steps: 0,
        })
    }

    pub fn run(mut self, max_steps: u64) -> Outcome {
        let mut error = None;
        let mut exit_code = None;
        loop {
            if self.pc == RETURN_ADDRESS {
                exit_code = Some(self.registers[0] as i64);
                break;
            }
            if self.steps >= max_steps {
                error = Some(format!(
                    "Stopped after {} steps (infinite loop?)",
                    max_steps
                ));
                break;
            }
            if let Err(message) = self.step() {
                error = Some(format!("{} (pc = 0x{:X})", message, self.pc));
                break;
            }
        }
        Outcome {
            output: String::from_utf8_lossy(&self.output).into_owned(),
            exit_code,
            error,
            steps: self.steps,
            registers: self.registers,
        }
    }

    fn read(&self, register: u32) -> u64 {
        if register == 31 {
            0
        } else {
            self.registers[register as usize]
        }
    }

    fn write(&mut self, register: u32, value: u64) {
        if register != 31 {
            self.registers[register as usize] = value;
        }
    }

    fn load(&mut self, address: u64) -> Result<u64, String> {
        if address == INPUT_ADDRESS {
            let value = match self.input.get(self.input_position) {
                Some(byte) => *byte as u64,
                None => u64::MAX,
            };
            self.input_position += 1;
            return Ok(value);
        }
        let start = self.check(address, 8)?;
        let mut bytes = [0; 8];
        bytes.copy_from_slice(&self.memory[start..start + 8]);
        Ok(u64::from_be_bytes(bytes))
    }

    fn store(&mut self, address: u64, value: u64) -> Result<(), String> {
        if address == OUTPUT_ADDRESS {
            self.output.push(value as u8);
            return Ok(());
        }
        let start = self.check(address, 8)?;
        self.memory[start..start + 8].copy_from_slice(&value.to_be_bytes());
        Ok(())
    }

    fn check(&self, address: u64, size: u64) -> Result<usize, String> {
        match address.checked_add(size) {
            Some(end) if end <= self.memory.len() as u64 => Ok(address as usize),
            _ => Err(format!("Memory access out of bounds at 0x{:X}", address)),
        }
    }

    fn condition(&self, code: u32) -> Result<bool, String> {
        let Flags {
            negative,
            zero,
            carry,
            overflow,
        } = self.flags;
        Ok(match code {
            0b00000 => zero,
            0b00001 => !zero,
            0b00010 => carry,
            0b00011 => !carry,
            0b01000 => carry && !zero,
            0b01001 => !carry || zero,
            0b01010 => negative == overflow,
            0b01011 => negative != overflow,
            0b01100 => !zero && negative == overflow,
            0b01101 => zero || negative != overflow,
            _ => return Err(format!("Unknown condition code {:05b}", code)),
        })
    }

    fn step(&mut self) -> Result<(), String> {
        let word = self.load_word(self.pc)? as u32;
        self.steps += 1;
        let mut next = self.pc.wrapping_add(4);

        let rd = word & 0x1F;
        let rn = (word >> 5) & 0x1F;
        let rm = (word >> 16) & 0x1F;
        let flags = (word >> 10) & 0x3F;
        let top11 = word >> 21;

        if word >> 26 == 0b000101 {
            let offset = sign_extend(word & 0x3FF_FFFF, 26);
            next = self.pc.wrapping_add((offset * 4) as u64);
        } else if word >> 24 == 0b01010100 {
            let offset = sign_extend((word >> 5) & 0x7FFFF, 19);
            if self.condition(rd)? {
                next = self.pc.wrapping_add((offset * 4) as u64);
            }
        } else if word >> 24 == 0b01011000 {
            let offset = sign_extend((word >> 5) & 0x7FFFF, 19);
            let value = self.load(self.pc.wrapping_add((offset * 4) as u64))?;
            self.write(rd, value);
        } else if top11 == 0b11111000_010 || top11 == 0b11111000_000 {
            let offset = sign_extend((word >> 12) & 0x1FF, 9);
            let address = self.read(rn).wrapping_add(offset as u64);
            if top11 == 0b11111000_010 {
                let value = self.load(address)?;
                self.write(rd, value);
            } else {
                self.store(address, self.read(rd))?;
            }
        } else {
            let a = self.read(rn);
            let b = self.read(rm);
            match (top11, flags) {
                (0b10001011_001, 0b011000) => self.write(rd, a.wrapping_add(b)),
                (0b11001011_001, 0b011000) => self.write(rd, a.wrapping_sub(b)),
                (0b10011011_000, 0b011111) => self.write(rd, a.wrapping_mul(b)),
                (0b10011011_010, 0b011111) => {
                    let product = (a as i64 as i128) * (b as i64 as i128);
                    self.write(rd, (product >> 64) as u64);
                }
                (0b10011011_110, 0b011111) => {
                    let product = (a as u128) * (b as u128);
                    self.write(rd, (product >> 64) as u64);
                }
                (0b10011010_110, 0b000011) => {
                    let value = if b == 0 {
                        0
                    } else {
                        (a as i64).wrapping_div(b as i64) as u64
                    };
                    self.write(rd, value);
                }
                (0b10011010_110, 0b000010) => {
                    self.write(rd, a.checked_div(b).unwrap_or(0));
                }
                (0b11101011_001, 0b011000) => {
                    let result = a.wrapping_sub(b);
                    self.flags = Flags {
                        negative: (result as i64) < 0,
                        zero: result == 0,
                        carry: a >= b,
                        overflow: (((a ^ b) & (a ^ result)) as i64) < 0,
                    };
                }
                (0b11010110_000, _) => next = a,
                (0b11010110_001, _) => {
                    next = a;
                    self.registers[30] = self.pc.wrapping_add(4);
                }
                _ => return Err(format!("Invalid instruction 0x{:08X}", word)),
            }
        }

        self.pc = next;
        Ok(())
    }

    fn load_word(&self, address: u64) -> Result<u64, String> {
        let start = self.check(address, 4)?;
        let mut bytes = [0; 4];
        bytes.copy_from_slice(&self.memory[start..start + 4]);
        Ok(u32::from_be_bytes(bytes) as u64)
    }
}

fn sign_extend(value: u32, bits: u32) -> i64 {
    let shift = 64 - bits;
    ((value as i64) << shift) >> shift
}

pub fn run(words: &[u32], memory_size: usize, input: &str, max_steps: u64) -> Outcome {
    match Machine::new(words, memory_size, input) {
        Ok(machine) => machine.run(max_steps),
        Err(error) => Outcome {
            output: String::new(),
            exit_code: None,
            error: Some(error),
            steps: 0,
            registers: [0; 32],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn execute(source: &str) -> Outcome {
        let words = assembler::assemble(source).unwrap();
        run(&words, 4096, "", 1000)
    }

    #[test]
    fn returns_x0() {
        let outcome = execute("ldr x0, 2\nbr x30\n.8byte 42");
        assert_eq!(outcome.exit_code, Some(42));
        assert!(outcome.error.is_none());
    }

    #[test]
    fn arithmetic_and_branches() {
        let program = "
       ldr x1, ten
       ldr x2, one
       add x0, xzr, xzr
loop:  cmp x1, xzr
       b.eq done
       add x0, x0, x1
       sub x1, x1, x2
       b loop
done:  br x30
ten:   .8byte 10
one:   .8byte 1
";
        assert_eq!(execute(program).exit_code, Some(55));
    }

    #[test]
    fn signed_division_and_comparison() {
        let program = "
       ldr x1, a
       ldr x2, b
       sdiv x0, x1, x2
       cmp x0, xzr
       b.lt neg
       add x0, xzr, xzr
neg:   br x30
a:     .8byte -7
b:     .8byte 2
";
        assert_eq!(execute(program).exit_code, Some(-3));
    }

    #[test]
    fn memory_mapped_output() {
        let program = "
       ldr x1, out
       ldr x2, ch
       stur x2, [x1, 0]
       add x0, xzr, xzr
       br x30
out:   .8byte 0xFFFF000C
ch:    .8byte 65
";
        assert_eq!(execute(program).output, "A");
    }

    #[test]
    fn stops_infinite_loops() {
        let outcome = execute("b 0");
        assert!(outcome.error.unwrap().contains("Stopped"));
    }

    #[test]
    fn out_of_bounds_is_error() {
        let outcome = execute("ldr x1, 2\nldur x0, [x1, 0]\n.8byte 0x100000");
        assert!(outcome.error.unwrap().contains("out of bounds"));
    }
}
