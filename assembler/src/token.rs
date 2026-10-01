/// Each varient represents a token kind, with an optional lexeme attached to the enum.
#[derive(Debug)]
pub enum Token {
    /// A period followed by one or more alphanumeric chars.
    Dotid(String),

    /// A letter (alphabetical character) followed by zero or more alphanumeric characters, ending off with a colon.
    Label(String),

    /// A letter followed by zero or more alphanumeric characters.
    Id(String),

    /// The characters 0x followed by 1 or more characters between 0-9, a-f, or A-F.
    Hexint(String),

    /// The letter x followed by one or two decimal digits. If a lexeme matches both REG and ID, it should be tokenized as REG. For example, x53 should be tokenized as a single REG, not as an ID.
    Reg(String),

    /// The string xzr. Takes precedence over ID.
    Zreg,

    /// The string sp. Takes precedence over ID.
    Sp,

    /// Must be one of:
    /// The number `0`.
    /// A digit from 1-9, optionally preceded by `a` - and optionally followed by one or more digits from 0-9.
    ///
    /// Examples: `0`, `-23`, `1004`
    Int(String),

    /// The , character.
    Comma,

    /// The [ character.
    Lbrack,

    /// The ] character.
    Rbrack,
}

#[derive(Debug)]
pub struct Tokens(Vec<Token>);
