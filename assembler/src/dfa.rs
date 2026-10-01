pub enum State {
    Start,
    State1,
    State2,
}

pub fn transition(state: &State, char: char) -> Result<State, ()> {
    match (state, char) {
        (State::Start, _) => todo!(),
        (State::State1, _) => todo!(),
        (State::State2, _) => todo!(),
    }
}
