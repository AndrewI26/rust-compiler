# C Compiler

A compiler for a subset of C that targets the ARM64 subset below, with an assembler, an emulator, and a WebAssembly web app that shows every stage.

```bash
cargo run -p compiler -- examples/fibonacci.c
cargo run -p compiler -- -S examples/fibonacci.c
cd web && trunk serve
```

## Supported C

- Types: `int`, `long`, `char`, `void`, pointers, and multi-dimensional arrays. Every scalar is 64 bits because the ISA only loads and stores 8-byte words, so `sizeof(char) == 8`.
- Functions with recursion and prototypes, globals, and block-scoped locals with initializers (`{...}` lists and string literals).
- Statements: `if`/`else`, `while`, `do`/`while`, `for`, `break`, `continue`, `return`.
- Every C operator except struct access: arithmetic, comparison, logical (short-circuit), bitwise, shifts, compound assignment, `++`/`--`, `?:`, `,`, `&`, `*`, `[]`, casts, `sizeof`.
- `#define NAME value`; `#include` lines are ignored.
- Runtime: `printf` (`%d %i %c %s %x %%`), `putchar`, `getchar`, `puts`, `print_int`, `print_str`, `read_int`, `malloc`, `calloc`, `free`, `strlen`, `strcmp`, `strcpy`, `abs`, `exit`.

## Conventions

- `x9`–`x24` hold expression temporaries, `x25` the return address to the loader, `x26` = 1, `x27` = 8, `x28` the stack pointer, `x29` the frame pointer.
- Arguments are pushed on the stack; the return value is in `x0`.
- Storing to `0xFFFF000C` writes a byte to stdout; loading from `0xFFFF0004` reads a byte from stdin (-1 at end of input).
- `.8byte label` stores a label's address, which is how code loads function and data addresses.
- The instruction set has no bitwise instructions, so `& | ^ << >>` call runtime helpers that are written in C.

## 3-Register Format

| Instruction | Assembly syntax | Behaviour | Opcode | xm | Flags | xn | xd |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Add | `add xd, xn, xm` | xd = xn + xm | `10001011 001` | `mmmmm` | `011000` | `nn nnn` | `ddddd` |
| Subtract | `sub xd, xn, xm` | xd = xn - xm | `11001011 001` | `mmmmm` | `011000` | `nn nnn` | `ddddd` |
| Multiply | `mul xd, xn, xm` | xd = (xn * xm) % 2^64 | `10011011 000` | `mmmmm` | `011111` | `nn nnn` | `ddddd` |
| Multiply Overflow Signed | `smulh xd, xn, xm` | xd = (xn * xm) / 2^64 | `10011011 010` | `mmmmm` | `011111` | `nn nnn` | `ddddd` |
| Multiply Overflow Unsigned | `umulh xd, xn, xm` | xd = (xn * xm) / 2^64 | `10011011 110` | `mmmmm` | `011111` | `nn nnn` | `ddddd` |
| Divide Signed | `sdiv xd, xn, xm` | xd = xn / xm | `10011010 110` | `mmmmm` | `000011` | `nn nnn` | `ddddd` |
| Divide Unsigned | `udiv xd, xn, xm` | xd = xn / xm | `10011010 110` | `mmmmm` | `000010` | `nn nnn` | `ddddd` |
| Compare | `cmp xn, xm` | compare xn and xm | `11101011 001` | `mmmmm` | `011000` | `nn nnn` | `11111` |
| Branch to Register | `br xn` | NPC = xn | `11010110 000` | `11111` | `000000` | `nn nnn` | `00000` |
| Branch & Link Register | `blr xn` | NPC = xn; x30 = PC + 4 | `11010110 001` | `11111` | `000000` | `nn nnn` | `00000` |

## 2-Register Format

| Instruction | Assembly syntax | Behaviour | Opcode | Immediate | Unused | xn | xd |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Load Register | `ldur xd, [xn, i]` | xd = MEM[xn + i] | `11111000 010` | `iiiii iiii` | `00` | `nn nnn` | `ddddd` |
| Store Register | `stur xd, [xn, i]` | MEM[xn + i] = xd | `11111000 000` | `iiiii iiii` | `00` | `nn nnn` | `ddddd` |

## 1-Register Format

| Instruction | Assembly syntax | Behaviour | Opcode | Immediate | xd |
| --- | --- | --- | --- | --- | --- |
| PC-Relative Load | `ldr xd, i` | xd = MEM[PC + i(*4)] | `01011000` | `iiiiiiii iiiiiiii iii` | `ddddd` |

## Branching Format

| Instruction | Assembly syntax | Behaviour | Opcode | Immediate (& Condition) |
| --- | --- | --- | --- | --- |
| Branch | `b i` | NPC = PC + i(*4) | `000101` | `ii iiiiiiii iiiiiiii iiiiiiii` |
| Conditional Branch | `b.cond i` | if (cond) NPC = PC + i(*4) | `01010100` | `iiiiiiii iiiiiiii iiiccccc` |

## Condition Codes

| Condition | Assembly syntax | Bits |
| --- | --- | --- |
| Equals | `eq` | `00000` |
| Not equals | `ne` | `00001` |
| Greater than or equals (unsigned) | `hs` | `00010` |
| Less than (unsigned) | `lo` | `00011` |
| Greater than (unsigned) | `hi` | `01000` |
| Less than or equals (unsigned) | `ls` | `01001` |
| Greater than or equals (signed) | `ge` | `01010` |
| Less than (signed) | `lt` | `01011` |
| Greater than (signed) | `gt` | `01100` |
| Less than or equals (signed) | `le` | `01101` |
