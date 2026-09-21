use std::fmt;

pub enum Token {
    Int,
    Identifier(String),
    Integer(i64),
    Equals,
    Plus,
    Minus,
    Semicolon,
    LParen,
    RParen,
}
pub struct Tokens(Vec<Token>);

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Int => write!(f, "INT"),
            Token::Identifier(name) => write!(f, "IDENTIFIER({})", name),
            Token::Integer(value) => write!(f, "INTEGER({})", value),
            Token::Equals => write!(f, "EQUALS"),
            Token::Plus => write!(f, "PLUS"),
            Token::Minus => write!(f, "MINUS"),
            Token::Semicolon => write!(f, "SEMICOLON"),
            Token::LParen => write!(f, "LPAREN"),
            Token::RParen => write!(f, "RPAREN"),
        }
    }
}

impl fmt::Display for Tokens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for token in &self.0 {
            writeln!(f, "{}", token)?;
        }

        Ok(())
    }
}

pub fn tokenize(code: &String) -> Result<Tokens, String> {
    let tokens: Vec<Token> = code
        .trim()
        .split(" ")
        .map(|token| match token {
            "int" => Ok(Token::Int),
            _ => Err(format!("Unknown token: {}", token)),
        })
        .collect()?;

    Ok(Tokens(tokens))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let code = String::from("int x = 4;");
        println!("tokens: {}", tokenize(&code));
    }
}
