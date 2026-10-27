use crate::CompileError;
use crate::ast::*;
use crate::lexer::{Token, TokenKind};

type Result<T> = std::result::Result<T, CompileError>;

pub fn parse(tokens: Vec<Token>) -> Result<Program> {
    let mut parser = Parser {
        tokens,
        position: 0,
    };
    parser.program()
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

const BINARY_LEVELS: [&[(&str, BinOp)]; 10] = [
    &[("||", BinOp::LogOr)],
    &[("&&", BinOp::LogAnd)],
    &[("|", BinOp::BitOr)],
    &[("^", BinOp::BitXor)],
    &[("&", BinOp::BitAnd)],
    &[("==", BinOp::Eq), ("!=", BinOp::Ne)],
    &[
        ("<", BinOp::Lt),
        ("<=", BinOp::Le),
        (">", BinOp::Gt),
        (">=", BinOp::Ge),
    ],
    &[("<<", BinOp::Shl), (">>", BinOp::Shr)],
    &[("+", BinOp::Add), ("-", BinOp::Sub)],
    &[("*", BinOp::Mul), ("/", BinOp::Div), ("%", BinOp::Mod)],
];

const ASSIGNMENTS: [(&str, Option<BinOp>); 11] = [
    ("=", None),
    ("+=", Some(BinOp::Add)),
    ("-=", Some(BinOp::Sub)),
    ("*=", Some(BinOp::Mul)),
    ("/=", Some(BinOp::Div)),
    ("%=", Some(BinOp::Mod)),
    ("&=", Some(BinOp::BitAnd)),
    ("|=", Some(BinOp::BitOr)),
    ("^=", Some(BinOp::BitXor)),
    ("<<=", Some(BinOp::Shl)),
    (">>=", Some(BinOp::Shr)),
];

enum Dimension {
    Known(usize),
    Inferred,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.position]
    }

    fn peek_at(&self, offset: usize) -> &TokenKind {
        let index = (self.position + offset).min(self.tokens.len() - 1);
        &self.tokens[index].kind
    }

    fn line(&self) -> usize {
        self.peek().line
    }

    fn advance(&mut self) -> Token {
        let token = self.tokens[self.position].clone();
        if self.position + 1 < self.tokens.len() {
            self.position += 1;
        }
        token
    }

    fn error<T>(&self, message: String) -> Result<T> {
        Err(CompileError {
            line: self.line(),
            message,
        })
    }

    fn found(&self) -> String {
        match &self.peek().kind {
            TokenKind::Eof => String::from("end of file"),
            _ => format!("`{}`", self.peek().describe().1),
        }
    }

    fn is_punct(&self, symbol: &str) -> bool {
        matches!(&self.peek().kind, TokenKind::Punct(p) if *p == symbol)
    }

    fn is_keyword(&self, word: &str) -> bool {
        matches!(&self.peek().kind, TokenKind::Keyword(k) if *k == word)
    }

    fn eat_punct(&mut self, symbol: &str) -> bool {
        if self.is_punct(symbol) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn eat_keyword(&mut self, word: &str) -> bool {
        if self.is_keyword(word) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_punct(&mut self, symbol: &str) -> Result<()> {
        if self.eat_punct(symbol) {
            Ok(())
        } else {
            self.error(format!("Expected `{}` but found {}", symbol, self.found()))
        }
    }

    fn expect_ident(&mut self) -> Result<String> {
        match &self.peek().kind {
            TokenKind::Ident(name) => {
                let name = name.clone();
                self.advance();
                Ok(name)
            }
            _ => self.error(format!("Expected a name but found {}", self.found())),
        }
    }

    fn starts_type(&self) -> bool {
        matches!(
            &self.peek().kind,
            TokenKind::Keyword("int" | "long" | "char" | "void" | "const" | "unsigned")
        )
    }

    fn base_type(&mut self) -> Result<Type> {
        let mut ty = None;
        loop {
            if self.eat_keyword("const") || self.eat_keyword("unsigned") {
                continue;
            }
            let next = match &self.peek().kind {
                TokenKind::Keyword("int") => Type::Int,
                TokenKind::Keyword("long") => Type::Long,
                TokenKind::Keyword("char") => Type::Char,
                TokenKind::Keyword("void") => Type::Void,
                _ => break,
            };
            self.advance();
            ty = Some(match (ty, next) {
                (None, next) => next,
                (Some(Type::Long), Type::Int | Type::Long) => Type::Long,
                (Some(Type::Int), Type::Long) => Type::Long,
                _ => return self.error(String::from("Invalid combination of type keywords")),
            });
        }
        match ty {
            Some(ty) => Ok(ty),
            None => self.error(format!("Expected a type but found {}", self.found())),
        }
    }

    fn pointers(&mut self, mut ty: Type) -> Type {
        loop {
            if self.eat_punct("*") {
                ty = ty.pointer_to();
            } else if !self.eat_keyword("const") {
                return ty;
            }
        }
    }

    fn dimensions(&mut self) -> Result<Vec<Dimension>> {
        let mut dimensions = Vec::new();
        while self.eat_punct("[") {
            if self.eat_punct("]") {
                dimensions.push(Dimension::Inferred);
                continue;
            }
            let line = self.line();
            let size = self.conditional()?;
            match const_eval(&size) {
                Some(size) if size > 0 => dimensions.push(Dimension::Known(size as usize)),
                _ => {
                    return Err(CompileError {
                        line,
                        message: String::from("Array size must be a positive constant"),
                    });
                }
            }
            self.expect_punct("]")?;
        }
        Ok(dimensions)
    }

    fn apply_dimensions(&self, ty: Type, dimensions: &[Dimension], inferred: usize) -> Type {
        dimensions.iter().rev().fold(ty, |ty, dimension| {
            let length = match dimension {
                Dimension::Known(length) => *length,
                Dimension::Inferred => inferred,
            };
            Type::Array(Box::new(ty), length)
        })
    }

    fn abstract_type(&mut self) -> Result<Type> {
        let base = self.base_type()?;
        let ty = self.pointers(base);
        let dimensions = self.dimensions()?;
        Ok(self.apply_dimensions(ty, &dimensions, 1))
    }

    fn program(&mut self) -> Result<Program> {
        let mut program = Program {
            globals: Vec::new(),
            functions: Vec::new(),
        };
        while self.peek().kind != TokenKind::Eof {
            let line = self.line();
            let base = self.base_type()?;
            let ty = self.pointers(base.clone());
            let name = self.expect_ident()?;
            if self.eat_punct("(") {
                let params = self.params()?;
                let body = if self.eat_punct(";") {
                    None
                } else {
                    Some(self.block()?)
                };
                program.functions.push(Function {
                    name,
                    ret: ty,
                    params,
                    body,
                    line,
                });
            } else {
                let mut variables = self.declarators(base, ty, name, line)?;
                program.globals.append(&mut variables);
            }
        }
        Ok(program)
    }

    fn params(&mut self) -> Result<Vec<(String, Type)>> {
        let mut params = Vec::new();
        if self.eat_punct(")") {
            return Ok(params);
        }
        if self.is_keyword("void") && *self.peek_at(1) == TokenKind::Punct(")") {
            self.advance();
            self.advance();
            return Ok(params);
        }
        loop {
            let base = self.base_type()?;
            let ty = self.pointers(base);
            let name = self.expect_ident()?;
            let dimensions = self.dimensions()?;
            let ty = self.apply_dimensions(ty, &dimensions, 1).decay();
            params.push((name, ty));
            if !self.eat_punct(",") {
                break;
            }
        }
        self.expect_punct(")")?;
        Ok(params)
    }

    fn declarators(
        &mut self,
        base: Type,
        first_type: Type,
        first_name: String,
        line: usize,
    ) -> Result<Vec<Variable>> {
        let mut variables = Vec::new();
        let mut ty = first_type;
        let mut name = first_name;
        loop {
            let dimensions = self.dimensions()?;
            let init = if self.eat_punct("=") {
                Some(self.initializer()?)
            } else {
                None
            };
            let inferred = match &init {
                Some(Init::List(items)) => items.len(),
                Some(Init::Expr(Expr {
                    kind: ExprKind::Str(text),
                    ..
                })) => text.chars().count() + 1,
                _ => 0,
            };
            if matches!(dimensions.first(), Some(Dimension::Inferred)) && inferred == 0 {
                return self.error(format!("Cannot infer the size of array `{}`", name));
            }
            let full = self.apply_dimensions(ty, &dimensions, inferred);
            if full == Type::Void {
                return self.error(format!("Variable `{}` cannot have type void", name));
            }
            variables.push(Variable {
                name,
                ty: full,
                init,
                line,
            });
            if !self.eat_punct(",") {
                break;
            }
            ty = self.pointers(base.clone());
            name = self.expect_ident()?;
        }
        self.expect_punct(";")?;
        Ok(variables)
    }

    fn initializer(&mut self) -> Result<Init> {
        if self.eat_punct("{") {
            let mut items = Vec::new();
            while !self.is_punct("}") {
                items.push(self.initializer()?);
                if !self.eat_punct(",") {
                    break;
                }
            }
            self.expect_punct("}")?;
            Ok(Init::List(items))
        } else {
            Ok(Init::Expr(self.assignment()?))
        }
    }

    fn block(&mut self) -> Result<Vec<Stmt>> {
        self.expect_punct("{")?;
        let mut stmts = Vec::new();
        while !self.eat_punct("}") {
            if self.peek().kind == TokenKind::Eof {
                return self.error(String::from("Missing `}` at end of file"));
            }
            stmts.push(self.statement()?);
        }
        Ok(stmts)
    }

    fn declaration(&mut self) -> Result<Stmt> {
        let line = self.line();
        let base = self.base_type()?;
        let ty = self.pointers(base.clone());
        let name = self.expect_ident()?;
        Ok(Stmt::Decl(self.declarators(base, ty, name, line)?))
    }

    fn statement(&mut self) -> Result<Stmt> {
        let line = self.line();
        if self.starts_type() {
            return self.declaration();
        }
        if self.is_punct("{") {
            return Ok(Stmt::Block(self.block()?));
        }
        if self.eat_punct(";") {
            return Ok(Stmt::Empty);
        }
        if self.eat_keyword("if") {
            self.expect_punct("(")?;
            let cond = self.expression()?;
            self.expect_punct(")")?;
            let then = Box::new(self.statement()?);
            let otherwise = if self.eat_keyword("else") {
                Some(Box::new(self.statement()?))
            } else {
                None
            };
            return Ok(Stmt::If(cond, then, otherwise));
        }
        if self.eat_keyword("while") {
            self.expect_punct("(")?;
            let cond = self.expression()?;
            self.expect_punct(")")?;
            return Ok(Stmt::While(cond, Box::new(self.statement()?)));
        }
        if self.eat_keyword("do") {
            let body = Box::new(self.statement()?);
            if !self.eat_keyword("while") {
                return self.error(format!("Expected `while` but found {}", self.found()));
            }
            self.expect_punct("(")?;
            let cond = self.expression()?;
            self.expect_punct(")")?;
            self.expect_punct(";")?;
            return Ok(Stmt::DoWhile(body, cond));
        }
        if self.eat_keyword("for") {
            self.expect_punct("(")?;
            let init = if self.eat_punct(";") {
                None
            } else if self.starts_type() {
                Some(Box::new(self.declaration()?))
            } else {
                let expr = self.expression()?;
                self.expect_punct(";")?;
                Some(Box::new(Stmt::Expr(expr)))
            };
            let cond = if self.is_punct(";") {
                None
            } else {
                Some(self.expression()?)
            };
            self.expect_punct(";")?;
            let step = if self.is_punct(")") {
                None
            } else {
                Some(self.expression()?)
            };
            self.expect_punct(")")?;
            let body = Box::new(self.statement()?);
            return Ok(Stmt::For(init, cond, step, body));
        }
        if self.eat_keyword("return") {
            let value = if self.is_punct(";") {
                None
            } else {
                Some(self.expression()?)
            };
            self.expect_punct(";")?;
            return Ok(Stmt::Return(value, line));
        }
        if self.eat_keyword("break") {
            self.expect_punct(";")?;
            return Ok(Stmt::Break(line));
        }
        if self.eat_keyword("continue") {
            self.expect_punct(";")?;
            return Ok(Stmt::Continue(line));
        }
        let expr = self.expression()?;
        self.expect_punct(";")?;
        Ok(Stmt::Expr(expr))
    }

    fn expression(&mut self) -> Result<Expr> {
        let mut expr = self.assignment()?;
        while self.is_punct(",") {
            let line = self.line();
            self.advance();
            let right = self.assignment()?;
            expr = Expr::new(ExprKind::Comma(Box::new(expr), Box::new(right)), line);
        }
        Ok(expr)
    }

    fn assignment(&mut self) -> Result<Expr> {
        let left = self.conditional()?;
        for (symbol, op) in ASSIGNMENTS {
            if self.is_punct(symbol) {
                let line = self.line();
                self.advance();
                let right = self.assignment()?;
                return Ok(Expr::new(
                    ExprKind::Assign(op, Box::new(left), Box::new(right)),
                    line,
                ));
            }
        }
        Ok(left)
    }

    fn conditional(&mut self) -> Result<Expr> {
        let cond = self.binary(0)?;
        if !self.is_punct("?") {
            return Ok(cond);
        }
        let line = self.line();
        self.advance();
        let then = self.expression()?;
        self.expect_punct(":")?;
        let otherwise = self.conditional()?;
        Ok(Expr::new(
            ExprKind::Cond(Box::new(cond), Box::new(then), Box::new(otherwise)),
            line,
        ))
    }

    fn binary(&mut self, level: usize) -> Result<Expr> {
        if level == BINARY_LEVELS.len() {
            return self.unary();
        }
        let mut left = self.binary(level + 1)?;
        'outer: loop {
            for (symbol, op) in BINARY_LEVELS[level] {
                if self.is_punct(symbol) {
                    let line = self.line();
                    self.advance();
                    let right = self.binary(level + 1)?;
                    left = Expr::new(ExprKind::Binary(*op, Box::new(left), Box::new(right)), line);
                    continue 'outer;
                }
            }
            return Ok(left);
        }
    }

    fn is_cast(&self) -> bool {
        self.is_punct("(")
            && matches!(
                self.peek_at(1),
                TokenKind::Keyword("int" | "long" | "char" | "void" | "const" | "unsigned")
            )
    }

    fn unary(&mut self) -> Result<Expr> {
        let line = self.line();
        let unary = |kind| Ok(Expr::new(kind, line));
        if self.is_cast() {
            self.advance();
            let ty = self.abstract_type()?;
            self.expect_punct(")")?;
            let inner = self.unary()?;
            return unary(ExprKind::Cast(ty, Box::new(inner)));
        }
        let symbol = match &self.peek().kind {
            TokenKind::Punct(symbol) => *symbol,
            TokenKind::Keyword("sizeof") => "sizeof",
            _ => return self.postfix(),
        };
        match symbol {
            "++" | "--" => {
                self.advance();
                let delta = if symbol == "++" { 1 } else { -1 };
                let inner = self.unary()?;
                unary(ExprKind::IncDec(true, delta, Box::new(inner)))
            }
            "-" | "!" | "~" => {
                self.advance();
                let op = match symbol {
                    "-" => UnOp::Neg,
                    "!" => UnOp::Not,
                    _ => UnOp::BitNot,
                };
                let inner = self.unary()?;
                unary(ExprKind::Unary(op, Box::new(inner)))
            }
            "+" => {
                self.advance();
                self.unary()
            }
            "*" => {
                self.advance();
                let inner = self.unary()?;
                unary(ExprKind::Deref(Box::new(inner)))
            }
            "&" => {
                self.advance();
                let inner = self.unary()?;
                unary(ExprKind::Addr(Box::new(inner)))
            }
            "sizeof" => {
                self.advance();
                if self.is_cast() {
                    self.advance();
                    let ty = self.abstract_type()?;
                    self.expect_punct(")")?;
                    return unary(ExprKind::SizeofType(ty));
                }
                let inner = self.unary()?;
                unary(ExprKind::SizeofExpr(Box::new(inner)))
            }
            _ => self.postfix(),
        }
    }

    fn postfix(&mut self) -> Result<Expr> {
        let mut expr = self.primary()?;
        loop {
            let line = self.line();
            if self.eat_punct("[") {
                let index = self.expression()?;
                self.expect_punct("]")?;
                expr = Expr::new(ExprKind::Index(Box::new(expr), Box::new(index)), line);
            } else if self.is_punct("(") {
                let ExprKind::Var(name) = &expr.kind else {
                    return self.error(String::from("Only named functions can be called"));
                };
                let name = name.clone();
                self.advance();
                let mut args = Vec::new();
                if !self.eat_punct(")") {
                    loop {
                        args.push(self.assignment()?);
                        if !self.eat_punct(",") {
                            break;
                        }
                    }
                    self.expect_punct(")")?;
                }
                expr = Expr::new(ExprKind::Call(name, args), expr.line);
            } else if self.eat_punct("++") {
                expr = Expr::new(ExprKind::IncDec(false, 1, Box::new(expr)), line);
            } else if self.eat_punct("--") {
                expr = Expr::new(ExprKind::IncDec(false, -1, Box::new(expr)), line);
            } else if self.is_punct(".") || self.is_punct("->") {
                return self.error(String::from("Structs are not supported"));
            } else {
                return Ok(expr);
            }
        }
    }

    fn primary(&mut self) -> Result<Expr> {
        let line = self.line();
        let token = self.peek().clone();
        let kind = match token.kind {
            TokenKind::Int(value) | TokenKind::Char(value) => {
                self.advance();
                ExprKind::Num(value)
            }
            TokenKind::Str(text) => {
                self.advance();
                let mut text = text;
                while let TokenKind::Str(more) = &self.peek().kind {
                    text.push_str(more);
                    self.advance();
                }
                ExprKind::Str(text)
            }
            TokenKind::Ident(name) => {
                self.advance();
                ExprKind::Var(name)
            }
            TokenKind::Punct("(") => {
                self.advance();
                let inner = self.expression()?;
                self.expect_punct(")")?;
                return Ok(inner);
            }
            _ => return self.error(format!("Expected an expression but found {}", self.found())),
        };
        Ok(Expr::new(kind, line))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    fn tree(source: &str) -> String {
        render(&parse(tokenize(source).unwrap()).unwrap())
    }

    fn error(source: &str) -> CompileError {
        parse(tokenize(source).unwrap()).err().unwrap()
    }

    #[test]
    fn precedence() {
        assert_eq!(
            tree("int main() { return 1 + 2 * 3 == 7; }"),
            "Function main() -> int\n  Return\n    Binary ==\n      Binary +\n        Num 1\n        Binary *\n          Num 2\n          Num 3\n      Num 7\n"
        );
    }

    #[test]
    fn declarations_and_arrays() {
        assert_eq!(
            tree("int a[2][3], *p; char s[] = \"hi\";"),
            "Global a: int[2][3]\nGlobal p: int*\nGlobal s: char[3]\n  Str \"hi\"\n"
        );
    }

    #[test]
    fn right_associative_assignment() {
        assert_eq!(
            tree("void f() { a = b += 1; }"),
            "Function f() -> void\n  Assign =\n    Var a\n    Assign +=\n      Var b\n      Num 1\n"
        );
    }

    #[test]
    fn casts_and_sizeof() {
        assert_eq!(
            tree("long f() { return sizeof(int*) + (long)x; }"),
            "Function f() -> long\n  Return\n    Binary +\n      Sizeof int*\n      Cast (long)\n        Var x\n"
        );
    }

    #[test]
    fn missing_semicolon_reports_line() {
        let error = error("int main() {\n  return 0\n}");
        assert_eq!(error.line, 3);
        assert_eq!(error.message, "Expected `;` but found `}`");
    }
}
