#[derive(Clone, PartialEq)]
pub enum Type {
    Void,
    Char,
    Int,
    Long,
    Pointer(Box<Type>),
    Array(Box<Type>, usize),
}

impl Type {
    pub fn size(&self) -> i64 {
        match self {
            Type::Void => 1,
            Type::Array(element, length) => element.size() * *length as i64,
            _ => 8,
        }
    }

    pub fn pointer_to(self) -> Type {
        Type::Pointer(Box::new(self))
    }

    pub fn target(&self) -> Option<&Type> {
        match self {
            Type::Pointer(target) | Type::Array(target, _) => Some(target),
            _ => None,
        }
    }

    pub fn is_integer(&self) -> bool {
        matches!(self, Type::Char | Type::Int | Type::Long)
    }

    pub fn decay(self) -> Type {
        match self {
            Type::Array(element, _) => Type::Pointer(element),
            other => other,
        }
    }

    pub fn name(&self) -> String {
        match self {
            Type::Void => String::from("void"),
            Type::Char => String::from("char"),
            Type::Int => String::from("int"),
            Type::Long => String::from("long"),
            Type::Pointer(target) => format!("{}*", target.name()),
            Type::Array(..) => {
                let mut dimensions = String::new();
                let mut ty = self;
                while let Type::Array(element, length) = ty {
                    dimensions.push_str(&format!("[{}]", length));
                    ty = element;
                }
                format!("{}{}", ty.name(), dimensions)
            }
        }
    }
}

pub struct Program {
    pub globals: Vec<Variable>,
    pub functions: Vec<Function>,
}

pub struct Variable {
    pub name: String,
    pub ty: Type,
    pub init: Option<Init>,
    pub line: usize,
}

pub enum Init {
    Expr(Expr),
    List(Vec<Init>),
}

pub struct Function {
    pub name: String,
    pub ret: Type,
    pub params: Vec<(String, Type)>,
    pub body: Option<Vec<Stmt>>,
    pub line: usize,
}

pub enum Stmt {
    Decl(Vec<Variable>),
    Expr(Expr),
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    While(Expr, Box<Stmt>),
    DoWhile(Box<Stmt>, Expr),
    For(
        Option<Box<Stmt>>,
        Option<Expr>,
        Option<Expr>,
        Box<Stmt>,
    ),
    Return(Option<Expr>, usize),
    Break(usize),
    Continue(usize),
    Block(Vec<Stmt>),
    Empty,
}

#[derive(Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    LogAnd,
    LogOr,
}

impl BinOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            BinOp::BitXor => "^",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::LogAnd => "&&",
            BinOp::LogOr => "||",
        }
    }

    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
        )
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum UnOp {
    Neg,
    Not,
    BitNot,
}

impl UnOp {
    pub fn symbol(self) -> &'static str {
        match self {
            UnOp::Neg => "-",
            UnOp::Not => "!",
            UnOp::BitNot => "~",
        }
    }
}

#[derive(Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub line: usize,
}

#[derive(Clone)]
pub enum ExprKind {
    Num(i64),
    Str(String),
    Var(String),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Assign(Option<BinOp>, Box<Expr>, Box<Expr>),
    Cond(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Deref(Box<Expr>),
    Addr(Box<Expr>),
    IncDec(bool, i64, Box<Expr>),
    SizeofType(Type),
    SizeofExpr(Box<Expr>),
    Cast(Type, Box<Expr>),
    Comma(Box<Expr>, Box<Expr>),
}

impl Expr {
    pub fn new(kind: ExprKind, line: usize) -> Expr {
        Expr { kind, line }
    }
}

pub fn const_eval(expr: &Expr) -> Option<i64> {
    Some(match &expr.kind {
        ExprKind::Num(value) => *value,
        ExprKind::SizeofType(ty) => ty.size(),
        ExprKind::Cast(_, inner) => const_eval(inner)?,
        ExprKind::Unary(op, inner) => {
            let value = const_eval(inner)?;
            match op {
                UnOp::Neg => value.wrapping_neg(),
                UnOp::Not => (value == 0) as i64,
                UnOp::BitNot => !value,
            }
        }
        ExprKind::Binary(op, left, right) => {
            let a = const_eval(left)?;
            let b = const_eval(right)?;
            match op {
                BinOp::Add => a.wrapping_add(b),
                BinOp::Sub => a.wrapping_sub(b),
                BinOp::Mul => a.wrapping_mul(b),
                BinOp::Div => a.checked_div(b)?,
                BinOp::Mod => a.checked_rem(b)?,
                BinOp::BitAnd => a & b,
                BinOp::BitOr => a | b,
                BinOp::BitXor => a ^ b,
                BinOp::Shl => a.wrapping_shl(b as u32),
                BinOp::Shr => a.wrapping_shr(b as u32),
                BinOp::Eq => (a == b) as i64,
                BinOp::Ne => (a != b) as i64,
                BinOp::Lt => (a < b) as i64,
                BinOp::Le => (a <= b) as i64,
                BinOp::Gt => (a > b) as i64,
                BinOp::Ge => (a >= b) as i64,
                BinOp::LogAnd => (a != 0 && b != 0) as i64,
                BinOp::LogOr => (a != 0 || b != 0) as i64,
            }
        }
        ExprKind::Cond(c, a, b) => {
            if const_eval(c)? != 0 {
                const_eval(a)?
            } else {
                const_eval(b)?
            }
        }
        _ => return None,
    })
}

pub fn render(program: &Program) -> String {
    let mut out = Tree::default();
    for global in &program.globals {
        out.variable("Global", global, 0);
    }
    for function in &program.functions {
        let params: Vec<String> = function
            .params
            .iter()
            .map(|(name, ty)| format!("{} {}", ty.name(), name))
            .collect();
        let kind = if function.body.is_some() {
            "Function"
        } else {
            "Prototype"
        };
        out.line(
            0,
            format!(
                "{} {}({}) -> {}",
                kind,
                function.name,
                params.join(", "),
                function.ret.name()
            ),
        );
        if let Some(body) = &function.body {
            for stmt in body {
                out.stmt(stmt, 1);
            }
        }
    }
    out.text
}

#[derive(Default)]
struct Tree {
    text: String,
}

impl Tree {
    fn line(&mut self, depth: usize, text: String) {
        for _ in 0..depth {
            self.text.push_str("  ");
        }
        self.text.push_str(&text);
        self.text.push('\n');
    }

    fn variable(&mut self, kind: &str, variable: &Variable, depth: usize) {
        self.line(
            depth,
            format!("{} {}: {}", kind, variable.name, variable.ty.name()),
        );
        if let Some(init) = &variable.init {
            self.init(init, depth + 1);
        }
    }

    fn init(&mut self, init: &Init, depth: usize) {
        match init {
            Init::Expr(expr) => self.expr(expr, depth),
            Init::List(items) => {
                self.line(depth, String::from("InitList"));
                for item in items {
                    self.init(item, depth + 1);
                }
            }
        }
    }

    fn stmt(&mut self, stmt: &Stmt, depth: usize) {
        match stmt {
            Stmt::Decl(variables) => {
                for variable in variables {
                    self.variable("Decl", variable, depth);
                }
            }
            Stmt::Expr(expr) => self.expr(expr, depth),
            Stmt::If(cond, then, otherwise) => {
                self.line(depth, String::from("If"));
                self.expr(cond, depth + 1);
                self.stmt(then, depth + 1);
                if let Some(otherwise) = otherwise {
                    self.line(depth, String::from("Else"));
                    self.stmt(otherwise, depth + 1);
                }
            }
            Stmt::While(cond, body) => {
                self.line(depth, String::from("While"));
                self.expr(cond, depth + 1);
                self.stmt(body, depth + 1);
            }
            Stmt::DoWhile(body, cond) => {
                self.line(depth, String::from("DoWhile"));
                self.stmt(body, depth + 1);
                self.expr(cond, depth + 1);
            }
            Stmt::For(init, cond, step, body) => {
                self.line(depth, String::from("For"));
                match init {
                    Some(init) => self.stmt(init, depth + 1),
                    None => self.line(depth + 1, String::from("(no init)")),
                }
                match cond {
                    Some(cond) => self.expr(cond, depth + 1),
                    None => self.line(depth + 1, String::from("(no condition)")),
                }
                match step {
                    Some(step) => self.expr(step, depth + 1),
                    None => self.line(depth + 1, String::from("(no step)")),
                }
                self.stmt(body, depth + 1);
            }
            Stmt::Return(value, _) => {
                self.line(depth, String::from("Return"));
                if let Some(value) = value {
                    self.expr(value, depth + 1);
                }
            }
            Stmt::Break(_) => self.line(depth, String::from("Break")),
            Stmt::Continue(_) => self.line(depth, String::from("Continue")),
            Stmt::Block(stmts) => {
                self.line(depth, String::from("Block"));
                for stmt in stmts {
                    self.stmt(stmt, depth + 1);
                }
            }
            Stmt::Empty => self.line(depth, String::from("Empty")),
        }
    }

    fn expr(&mut self, expr: &Expr, depth: usize) {
        let children: Vec<&Expr> = match &expr.kind {
            ExprKind::Num(value) => {
                self.line(depth, format!("Num {}", value));
                vec![]
            }
            ExprKind::Str(text) => {
                self.line(depth, format!("Str {:?}", text));
                vec![]
            }
            ExprKind::Var(name) => {
                self.line(depth, format!("Var {}", name));
                vec![]
            }
            ExprKind::Unary(op, inner) => {
                self.line(depth, format!("Unary {}", op.symbol()));
                vec![inner]
            }
            ExprKind::Binary(op, left, right) => {
                self.line(depth, format!("Binary {}", op.symbol()));
                vec![left, right]
            }
            ExprKind::Assign(op, left, right) => {
                let symbol = op.map_or("", |op| op.symbol());
                self.line(depth, format!("Assign {}=", symbol));
                vec![left, right]
            }
            ExprKind::Cond(c, a, b) => {
                self.line(depth, String::from("Ternary"));
                vec![c, a, b]
            }
            ExprKind::Call(name, args) => {
                self.line(depth, format!("Call {}", name));
                args.iter().collect()
            }
            ExprKind::Index(array, index) => {
                self.line(depth, String::from("Index"));
                vec![array, index]
            }
            ExprKind::Deref(inner) => {
                self.line(depth, String::from("Deref *"));
                vec![inner]
            }
            ExprKind::Addr(inner) => {
                self.line(depth, String::from("AddressOf &"));
                vec![inner]
            }
            ExprKind::IncDec(prefix, delta, inner) => {
                let symbol = if *delta > 0 { "++" } else { "--" };
                let position = if *prefix { "Pre" } else { "Post" };
                self.line(depth, format!("{}{}", position, symbol));
                vec![inner]
            }
            ExprKind::SizeofType(ty) => {
                self.line(depth, format!("Sizeof {}", ty.name()));
                vec![]
            }
            ExprKind::SizeofExpr(inner) => {
                self.line(depth, String::from("Sizeof"));
                vec![inner]
            }
            ExprKind::Cast(ty, inner) => {
                self.line(depth, format!("Cast ({})", ty.name()));
                vec![inner]
            }
            ExprKind::Comma(a, b) => {
                self.line(depth, String::from("Comma"));
                vec![a, b]
            }
        };
        for child in children {
            self.expr(child, depth + 1);
        }
    }
}
