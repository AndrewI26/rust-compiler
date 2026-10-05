use crate::token::Token;

/// States of the tokenizing DFA.
pub enum State {
    Start,
    Id,
    Label,
    XStart,
    X1,
    X2,
    Xz,
    Xzr,
    SStart,
    Sp,
    Zero,
    HexPrefix,
    HexDigits,
    PosInt,
    NegStart,
    NegInt,
    Comma,
    Lbrack,
    Rbrack,
    DotStart,
    Dotid,
    Ws,
}

impl State {
    /// Returns true if the DFA can stop in this state and emit a token.
    pub fn is_accepting(&self) -> bool {
        !matches!(
            self,
            State::Start | State::HexPrefix | State::NegStart | State::DotStart
        )
    }
}

/// Returns the next state, or `Err(())` if there is no transition on `char`.
pub fn transition(state: &State, char: char) -> Result<State, ()> {
    use State::*;

    let next = match (state, char) {
        (Start, 'x') => XStart,
        (Start, 's') => SStart,
        (Start, 'a'..='z' | 'A'..='Z') => Id,
        (Start, '0') => Zero,
        (Start, '1'..='9') => PosInt,
        (Start, '-') => NegStart,
        (Start, '.') => DotStart,
        (Start, ',') => Comma,
        (Start, '[') => Lbrack,
        (Start, ']') => Rbrack,
        (Start, ' ' | '\n') => Ws,

        (Id, 'a'..='z' | 'A'..='Z' | '0'..='9') => Id,
        (Id, ':') => Label,

        (XStart, 'z') => Xz,
        (XStart, '0'..='9') => X1,
        (XStart, 'a'..='z' | 'A'..='Z') => Id,
        (XStart, ':') => Label,

        (X1, '0'..='9') => X2,
        (X1, 'a'..='z' | 'A'..='Z') => Id,
        (X1, ':') => Label,

        (X2, 'a'..='z' | 'A'..='Z' | '0'..='9') => Id,
        (X2, ':') => Label,

        (Xz, 'r') => Xzr,
        (Xz, 'a'..='z' | 'A'..='Z' | '0'..='9') => Id,
        (Xz, ':') => Label,

        (Xzr, 'a'..='z' | 'A'..='Z' | '0'..='9') => Id,
        (Xzr, ':') => Label,

        (SStart, 'p') => Sp,
        (SStart, 'a'..='z' | 'A'..='Z' | '0'..='9') => Id,
        (SStart, ':') => Label,

        (Sp, 'a'..='z' | 'A'..='Z' | '0'..='9') => Id,
        (Sp, ':') => Label,

        (Zero, 'x') => HexPrefix,

        (HexPrefix | HexDigits, '0'..='9' | 'a'..='f' | 'A'..='F') => HexDigits,

        (PosInt, '0'..='9') => PosInt,

        (NegStart, '1'..='9') => NegInt,
        (NegInt, '0'..='9') => NegInt,

        (DotStart | Dotid, 'a'..='z' | 'A'..='Z' | '0'..='9') => Dotid,

        (Ws, ' ' | '\n') => Ws,

        _ => return Err(()),
    };

    Ok(next)
}

/// Converts a finished lexeme into a token based on the state it ended in.
/// Returns `None` for whitespace (skipped) and non-accepting states.
pub fn to_token(state: &State, lexeme: String) -> Option<Token> {
    use State::*;

    match state {
        Id | XStart | Xz | SStart => Some(Token::Id(lexeme)),
        Label => Some(Token::Label(lexeme)),
        X1 | X2 => Some(Token::Reg(lexeme)),
        Xzr => Some(Token::Zreg),
        Sp => Some(Token::Sp),
        Zero | PosInt | NegInt => Some(Token::Int(lexeme)),
        HexDigits => Some(Token::Hexint(lexeme)),
        Dotid => Some(Token::Dotid(lexeme)),
        Comma => Some(Token::Comma),
        Lbrack => Some(Token::Lbrack),
        Rbrack => Some(Token::Rbrack),
        Ws => None,
        Start | HexPrefix | NegStart | DotStart => None,
    }
}
