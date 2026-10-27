use std::collections::HashMap;

use crate::CompileError;

#[derive(Clone, PartialEq)]
pub enum TokenKind {
    Int(i64),
    Char(i64),
    Str(String),
    Ident(String),
    Keyword(&'static str),
    Punct(&'static str),
    Eof,
}

#[derive(Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
}

impl Token {
    pub fn describe(&self) -> (&'static str, String) {
        match &self.kind {
            TokenKind::Int(value) => ("INT", value.to_string()),
            TokenKind::Char(value) => ("CHAR", quote_char(*value)),
            TokenKind::Str(text) => ("STR", format!("{:?}", text)),
            TokenKind::Ident(name) => ("ID", name.clone()),
            TokenKind::Keyword(word) => ("KW", word.to_string()),
            TokenKind::Punct(symbol) => ("OP", symbol.to_string()),
            TokenKind::Eof => ("EOF", String::new()),
        }
    }
}

fn quote_char(value: i64) -> String {
    match char::from_u32(value as u32) {
        Some(c) => format!("{:?}", c),
        None => value.to_string(),
    }
}

const KEYWORDS: [&str; 15] = [
    "int", "long", "char", "void", "if", "else", "while", "do", "for", "return", "break",
    "continue", "sizeof", "const", "unsigned",
];

const PUNCTUATION: [&str; 47] = [
    "<<=", ">>=", "...", "++", "--", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<", ">>",
    "<=", ">=", "==", "!=", "&&", "||", "->", "+", "-", "*", "/", "%", "&", "|", "^", "~", "!",
    "<", ">", "=", "?", ":", ";", ",", "(", ")", "{", "}", "[", "]", ".", "#",
];

struct Lexer<'a> {
    chars: Vec<char>,
    position: usize,
    line: usize,
    at_line_start: bool,
    macros: &'a mut HashMap<String, Vec<TokenKind>>,
}

pub fn tokenize(source: &str) -> Result<Vec<Token>, CompileError> {
    let mut macros = HashMap::new();
    let mut tokens = lex(source, 1, &mut macros)?;
    let line = tokens.last().map_or(1, |t| t.line);
    tokens.push(Token {
        kind: TokenKind::Eof,
        line,
    });
    Ok(tokens)
}

fn lex(
    source: &str,
    line: usize,
    macros: &mut HashMap<String, Vec<TokenKind>>,
) -> Result<Vec<Token>, CompileError> {
    let mut lexer = Lexer {
        chars: source.chars().collect(),
        position: 0,
        line,
        at_line_start: true,
        macros,
    };
    let mut tokens = Vec::new();
    while let Some(token) = lexer.next_token()? {
        match &token.kind {
            TokenKind::Ident(name) if lexer.macros.contains_key(name) => {
                for kind in &lexer.macros[name] {
                    tokens.push(Token {
                        kind: kind.clone(),
                        line: token.line,
                    });
                }
            }
            _ => tokens.push(token),
        }
    }
    Ok(tokens)
}

impl Lexer<'_> {
    fn peek(&self, offset: usize) -> char {
        *self.chars.get(self.position + offset).unwrap_or(&'\0')
    }

    fn bump(&mut self) -> char {
        let c = self.peek(0);
        self.position += 1;
        if c == '\n' {
            self.line += 1;
            self.at_line_start = true;
        }
        c
    }

    fn error(&self, message: String) -> CompileError {
        CompileError {
            line: self.line,
            message,
        }
    }

    fn skip_trivia(&mut self) -> Result<(), CompileError> {
        loop {
            let c = self.peek(0);
            if c == '\n' || c.is_whitespace() {
                self.bump();
            } else if c == '/' && self.peek(1) == '/' {
                while self.peek(0) != '\n' && self.peek(0) != '\0' {
                    self.bump();
                }
            } else if c == '/' && self.peek(1) == '*' {
                let line = self.line;
                self.bump();
                self.bump();
                while !(self.peek(0) == '*' && self.peek(1) == '/') {
                    if self.position >= self.chars.len() {
                        return Err(CompileError {
                            line,
                            message: String::from("Unterminated comment"),
                        });
                    }
                    self.bump();
                }
                self.bump();
                self.bump();
            } else if c == '#' && self.at_line_start {
                self.directive()?;
            } else {
                return Ok(());
            }
        }
    }

    fn directive(&mut self) -> Result<(), CompileError> {
        let line = self.line;
        let mut text = String::new();
        while self.peek(0) != '\n' && self.peek(0) != '\0' {
            text.push(self.bump());
        }
        let text = text[1..].trim_start();
        if let Some(rest) = text.strip_prefix("define") {
            let rest = rest.trim_start();
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if name.is_empty() {
                return Err(CompileError {
                    line,
                    message: String::from("Expected a macro name after #define"),
                });
            }
            if rest[name.len()..].starts_with('(') {
                return Err(CompileError {
                    line,
                    message: String::from("Function-like macros are not supported"),
                });
            }
            let body = lex(&rest[name.len()..], line, self.macros)?;
            self.macros
                .insert(name, body.into_iter().map(|t| t.kind).collect());
        } else if !text.starts_with("include") && !text.starts_with("pragma") {
            return Err(CompileError {
                line,
                message: format!("Unsupported directive #{}", text),
            });
        }
        Ok(())
    }

    fn next_token(&mut self) -> Result<Option<Token>, CompileError> {
        self.skip_trivia()?;
        self.at_line_start = false;
        let line = self.line;
        let c = self.peek(0);
        if c == '\0' && self.position >= self.chars.len() {
            return Ok(None);
        }

        let kind = if c.is_ascii_digit() {
            self.number()?
        } else if c.is_alphabetic() || c == '_' {
            let mut word = String::new();
            while self.peek(0).is_alphanumeric() || self.peek(0) == '_' {
                word.push(self.bump());
            }
            match KEYWORDS.iter().find(|k| **k == word) {
                Some(keyword) => TokenKind::Keyword(keyword),
                None => TokenKind::Ident(word),
            }
        } else if c == '\'' {
            self.bump();
            if self.peek(0) == '\'' {
                return Err(self.error(String::from("Empty character literal")));
            }
            let value = self.character()?;
            if self.bump() != '\'' {
                return Err(self.error(String::from("Unterminated character literal")));
            }
            TokenKind::Char(value)
        } else if c == '"' {
            self.bump();
            let mut text = String::new();
            while self.peek(0) != '"' {
                if self.peek(0) == '\n' || self.position >= self.chars.len() {
                    return Err(self.error(String::from("Unterminated string literal")));
                }
                let value = self.character()?;
                text.push(char::from_u32(value as u32).unwrap_or('?'));
            }
            self.bump();
            TokenKind::Str(text)
        } else {
            let rest: String = self.chars[self.position..].iter().take(3).collect();
            match PUNCTUATION.iter().find(|p| rest.starts_with(**p)) {
                Some(symbol) => {
                    for _ in 0..symbol.len() {
                        self.bump();
                    }
                    TokenKind::Punct(symbol)
                }
                None => return Err(self.error(format!("Unexpected character {:?}", c))),
            }
        };

        Ok(Some(Token { kind, line }))
    }

    fn number(&mut self) -> Result<TokenKind, CompileError> {
        let mut digits = String::new();
        let radix = if self.peek(0) == '0' && matches!(self.peek(1), 'x' | 'X') {
            self.bump();
            self.bump();
            16
        } else {
            10
        };
        while self.peek(0).is_ascii_alphanumeric() {
            digits.push(self.bump());
        }
        let digits = digits.trim_end_matches(['l', 'L', 'u', 'U']);
        match u64::from_str_radix(digits, radix) {
            Ok(value) => Ok(TokenKind::Int(value as i64)),
            Err(_) => Err(self.error(format!("Invalid number {:?}", digits))),
        }
    }

    fn character(&mut self) -> Result<i64, CompileError> {
        let c = self.bump();
        if c != '\\' {
            return Ok(c as i64);
        }
        let escaped = self.bump();
        Ok(match escaped {
            'n' => 10,
            't' => 9,
            'r' => 13,
            '0' => 0,
            'a' => 7,
            'b' => 8,
            'f' => 12,
            'v' => 11,
            'e' => 27,
            '\\' | '\'' | '"' | '?' => escaped as i64,
            _ => return Err(self.error(format!("Unknown escape sequence \\{}", escaped))),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<TokenKind> {
        tokenize(source).unwrap().into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn basic_tokens() {
        assert!(
            kinds("int x = 0x1F + 'a';")
                == vec![
                    TokenKind::Keyword("int"),
                    TokenKind::Ident(String::from("x")),
                    TokenKind::Punct("="),
                    TokenKind::Int(31),
                    TokenKind::Punct("+"),
                    TokenKind::Char(97),
                    TokenKind::Punct(";"),
                    TokenKind::Eof,
                ]
        );
    }

    #[test]
    fn comments_and_directives() {
        let source = "#include <stdio.h>\n#define N 10\n// hi\n/* a\nb */ N";
        let tokens = tokenize(source).unwrap();
        assert!(tokens[0].kind == TokenKind::Int(10));
        assert_eq!(tokens[0].line, 5);
    }

    #[test]
    fn longest_punctuation_wins() {
        assert!(
            kinds("a<<=b")
                == vec![
                    TokenKind::Ident(String::from("a")),
                    TokenKind::Punct("<<="),
                    TokenKind::Ident(String::from("b")),
                    TokenKind::Eof,
                ]
        );
    }

    #[test]
    fn string_escapes() {
        assert!(kinds("\"a\\n\"")[0] == TokenKind::Str(String::from("a\n")));
    }

    #[test]
    fn unterminated_string_is_error() {
        assert_eq!(tokenize("\"abc").err().unwrap().line, 1);
    }
}
