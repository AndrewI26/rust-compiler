use crate::dfa::{State, to_token, transition};
use crate::token::Tokens;

/// Impliments a simplified "Maximal Munch" algorithm,
/// generating tokens using the longest valid sequences of charecters.
///
/// **Does not impliment backtracking.**
///
/// # Errors
///
/// Returns an error if the input is invalid ARM syntax.
pub fn munch(input: &String) -> Result<Tokens, String> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();

    let mut i = 0;
    let mut state: State = State::Start;
    let mut lexeme = String::from("");

    while i < chars.len() {
        let char = chars[i];

        match transition(&state, char) {
            Ok(next) => {
                lexeme.push(char);
                state = next;
                i += 1;
            }
            Err(()) => {
                // No transition: the current lexeme is as long as it can get.
                if lexeme.is_empty() {
                    return Err(format!("Unexpected character {:?}", char));
                }
                if !state.is_accepting() {
                    return Err(format!("Invalid token {:?} at {:?}", lexeme, char));
                }

                if let Some(token) = to_token(&state, lexeme) {
                    tokens.push(token);
                }

                // Don't advance i, so `char` starts the next lexeme.
                state = State::Start;
                lexeme = String::from("");
            }
        }
    }

    // Output whatever lexeme was in progress when the input ran out.
    if !lexeme.is_empty() {
        if !state.is_accepting() {
            return Err(format!("Invalid token {:?} at end of line", lexeme));
        }

        if let Some(token) = to_token(&state, lexeme) {
            tokens.push(token);
        }
    }

    Ok(Tokens(tokens))
}

#[cfg(test)]
mod tests {
    use super::munch;
    use crate::token::Token::{self, *};

    fn lex(input: &str) -> Vec<Token> {
        munch(&input.to_string()).unwrap().0
    }

    #[test]
    fn empty_input() {
        assert_eq!(lex(""), vec![]);
    }

    #[test]
    fn whitespace_is_skipped() {
        assert_eq!(lex("  \n add \n"), vec![Id("add".into())]);
    }

    #[test]
    fn ids_and_labels() {
        assert_eq!(
            lex("foo Bar9 loop:"),
            vec![Id("foo".into()), Id("Bar9".into()), Label("loop:".into())]
        );
    }

    #[test]
    fn registers() {
        assert_eq!(
            lex("x0 x9 x30"),
            vec![Reg("x0".into()), Reg("x9".into()), Reg("x30".into())]
        );
    }

    #[test]
    fn register_lookalikes_are_ids() {
        assert_eq!(
            lex("x xz s x123 x1a xzrr spx"),
            vec![
                Id("x".into()),
                Id("xz".into()),
                Id("s".into()),
                Id("x123".into()),
                Id("x1a".into()),
                Id("xzrr".into()),
                Id("spx".into()),
            ]
        );
    }

    #[test]
    fn special_registers() {
        assert_eq!(lex("xzr sp"), vec![Zreg, Sp]);
    }

    #[test]
    fn register_lookalike_labels() {
        assert_eq!(
            lex("x1: xzr: sp:"),
            vec![
                Label("x1:".into()),
                Label("xzr:".into()),
                Label("sp:".into())
            ]
        );
    }

    #[test]
    fn integers() {
        assert_eq!(
            lex("0 7 1004 -23"),
            vec![
                Int("0".into()),
                Int("7".into()),
                Int("1004".into()),
                Int("-23".into()),
            ]
        );
    }

    #[test]
    fn hex_integers() {
        assert_eq!(
            lex("0x0 0xff 0x1A2b"),
            vec![
                Hexint("0x0".into()),
                Hexint("0xff".into()),
                Hexint("0x1A2b".into()),
            ]
        );
    }

    #[test]
    fn dotids() {
        assert_eq!(
            lex(".word .8byte"),
            vec![Dotid(".word".into()), Dotid(".8byte".into())]
        );
    }

    #[test]
    fn punctuation_without_spaces() {
        assert_eq!(
            lex("[x1,x2]"),
            vec![Lbrack, Reg("x1".into()), Comma, Reg("x2".into()), Rbrack]
        );
    }

    #[test]
    fn leading_zero_splits_into_two_ints() {
        // No backtracking: `0` can't continue with a digit, so `01` is two tokens.
        assert_eq!(lex("01"), vec![Int("0".into()), Int("1".into())]);
    }

    #[test]
    fn full_line() {
        assert_eq!(
            lex("main: ldr x12, [sp, 0x1F]\n"),
            vec![
                Label("main:".into()),
                Id("ldr".into()),
                Reg("x12".into()),
                Comma,
                Lbrack,
                Sp,
                Comma,
                Hexint("0x1F".into()),
                Rbrack,
            ]
        );
    }

    #[test]
    fn lone_minus_is_error() {
        assert!(munch(&"-".to_string()).is_err());
    }

    #[test]
    fn minus_zero_is_error() {
        assert!(munch(&"-0".to_string()).is_err());
    }

    #[test]
    fn hex_prefix_without_digits_is_error() {
        assert!(munch(&"0x".to_string()).is_err());
    }

    #[test]
    fn lone_dot_is_error() {
        assert!(munch(&". word".to_string()).is_err());
    }

    #[test]
    fn invalid_character_is_error() {
        assert!(munch(&"add x1, #5".to_string()).is_err());
    }
}
