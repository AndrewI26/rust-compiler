use crate::dfa::{State, transition};
use crate::token::Tokens;

/// Impliments a simplified "Maximal Munch" algorithm,
/// generating tokens using the longest valid sequences of charecters.
/// **Does not impliment backtracking.**
///
/// # Panics
///
/// Panics if the input is invalid ARM syntax.
pub fn munch(input: &String) -> Tokens {
    let mut i = 0;
    let mut state: State = State::Start;

    loop {
        let char = input.chars().nth(i).expect("Invalid i");

        let mut next: Result<State, ()> = Err(());
        if i < input.len() - 1 {
            next = transition(&state, char);
        }

        match next {
            Ok(next) => {
                i += 1;
                state = next;
            }
            Err(()) => {}
        }
    }

    todo!();
}
