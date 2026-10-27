use std::collections::{HashMap, HashSet};

use crate::CompileError;
use crate::ast::*;

pub const MEMORY_SIZE: usize = 0x40_0000;
pub const OUTPUT_ADDRESS: i64 = 0xFFFF_000C;
pub const INPUT_ADDRESS: i64 = 0xFFFF_0004;

const TEMPS: usize = 16;
const FIRST_TEMP: usize = 9;
const INTRINSICS: [&str; 5] = ["putchar", "getchar", "exit", "printf", "__heap_start"];

type Result<T> = std::result::Result<T, CompileError>;

struct Signature {
    label: String,
    ret: Type,
    params: Vec<Type>,
    defined: bool,
}

struct Global {
    label: String,
    ty: Type,
}

struct Local {
    offset: i64,
    ty: Type,
}

enum Slot<'a> {
    Zero,
    Value(i64),
    Expr(&'a Expr),
}

struct Generator {
    signatures: HashMap<String, Signature>,
    globals: HashMap<String, Global>,
    constants: Vec<(String, String)>,
    constant_labels: HashMap<String, String>,
    strings: Vec<(String, String)>,
    labels: usize,
    code: Vec<String>,
    scopes: Vec<HashMap<String, Local>>,
    frame: i64,
    loops: Vec<(String, String)>,
    return_label: String,
    calls: HashSet<String>,
    line: usize,
}

pub fn generate(program: &Program, prelude: &Program) -> std::result::Result<String, Vec<CompileError>> {
    let mut errors = Vec::new();
    let user_functions: HashSet<&str> = program
        .functions
        .iter()
        .map(|f| f.name.as_str())
        .collect();
    let user_globals: HashSet<&str> = program.globals.iter().map(|g| g.name.as_str()).collect();

    let functions: Vec<(&Function, bool)> = program
        .functions
        .iter()
        .map(|f| (f, true))
        .chain(
            prelude
                .functions
                .iter()
                .filter(|f| !user_functions.contains(f.name.as_str()))
                .map(|f| (f, false)),
        )
        .collect();
    let globals: Vec<&Variable> = program
        .globals
        .iter()
        .chain(
            prelude
                .globals
                .iter()
                .filter(|g| !user_globals.contains(g.name.as_str())),
        )
        .collect();

    let mut generator = Generator {
        signatures: HashMap::new(),
        globals: HashMap::new(),
        constants: Vec::new(),
        constant_labels: HashMap::new(),
        strings: Vec::new(),
        labels: 0,
        code: Vec::new(),
        scopes: Vec::new(),
        frame: 0,
        loops: Vec::new(),
        return_label: String::new(),
        calls: HashSet::new(),
        line: 0,
    };

    for (index, (function, _)) in functions.iter().enumerate() {
        let params: Vec<Type> = function.params.iter().map(|(_, ty)| ty.clone()).collect();
        match generator.signatures.get_mut(&function.name) {
            Some(existing) => {
                if existing.params.len() != params.len() || existing.ret != function.ret {
                    errors.push(CompileError {
                        line: function.line,
                        message: format!("Conflicting declarations of `{}`", function.name),
                    });
                } else if existing.defined && function.body.is_some() {
                    errors.push(CompileError {
                        line: function.line,
                        message: format!("Function `{}` is defined twice", function.name),
                    });
                }
                existing.defined |= function.body.is_some();
            }
            None => {
                generator.signatures.insert(
                    function.name.clone(),
                    Signature {
                        label: format!("F{}{}", index, mangle(&function.name)),
                        ret: function.ret.clone(),
                        params,
                        defined: function.body.is_some(),
                    },
                );
            }
        }
    }

    let mut data = Vec::new();
    for (index, global) in globals.iter().enumerate() {
        if generator.globals.contains_key(&global.name)
            || generator.signatures.contains_key(&global.name)
        {
            errors.push(CompileError {
                line: global.line,
                message: format!("`{}` is already declared", global.name),
            });
            continue;
        }
        let label = format!("G{}{}", index, mangle(&global.name));
        match generator.global_data(global) {
            Ok(values) => {
                data.push(format!("{}:", label));
                data.extend(values.into_iter().map(|v| format!("    .8byte {}", v)));
            }
            Err(error) => errors.push(error),
        }
        generator.globals.insert(
            global.name.clone(),
            Global {
                label,
                ty: global.ty.clone(),
            },
        );
    }

    match generator.signatures.get("main") {
        Some(main) if main.defined => {}
        _ => errors.push(CompileError {
            line: 1,
            message: String::from("The program has no `main` function"),
        }),
    }

    let mut bodies: HashMap<String, Vec<String>> = HashMap::new();
    let mut graph: HashMap<String, HashSet<String>> = HashMap::new();
    for (function, _) in &functions {
        if function.body.is_none() {
            continue;
        }
        match generator.function(function) {
            Ok(code) => {
                bodies.insert(function.name.clone(), code);
            }
            Err(error) => errors.push(error),
        }
        graph.insert(
            function.name.clone(),
            std::mem::take(&mut generator.calls),
        );
    }

    if !errors.is_empty() {
        errors.sort_by_key(|e| e.line);
        return Err(errors);
    }

    let mut reachable: HashSet<String> = HashSet::new();
    let mut pending: Vec<String> = functions
        .iter()
        .filter(|(_, user)| *user)
        .map(|(f, _)| f.name.clone())
        .collect();
    while let Some(name) = pending.pop() {
        if reachable.insert(name.clone()) {
            if let Some(calls) = graph.get(&name) {
                pending.extend(calls.iter().cloned());
            }
        }
    }

    let stack = generator.constant(MEMORY_SIZE.to_string());
    let eight = generator.constant(String::from("8"));
    let one = generator.constant(String::from("1"));
    let main = generator.constant(generator.signatures["main"].label.clone());

    let mut lines = vec![
        String::from("    add x25, x30, xzr"),
        format!("    ldr x28, {}", stack),
        format!("    ldr x27, {}", eight),
        format!("    ldr x26, {}", one),
        format!("    ldr x8, {}", main),
        String::from("    blr x8"),
        String::from("    br x25"),
    ];
    for (function, _) in &functions {
        if !reachable.contains(&function.name) {
            continue;
        }
        if let Some(code) = bodies.remove(&function.name) {
            lines.push(String::new());
            lines.extend(code);
        }
    }

    let used: HashSet<&str> = lines
        .iter()
        .filter_map(|line| line.trim_start().strip_prefix("ldr "))
        .filter_map(|operands| operands.rsplit(", ").next())
        .collect();
    let constants: Vec<&(String, String)> = generator
        .constants
        .iter()
        .filter(|(label, _)| used.contains(label.as_str()))
        .collect();
    let used: HashSet<&str> = constants
        .iter()
        .map(|(_, value)| value.as_str())
        .chain(data.iter().filter_map(|line| line.rsplit(' ').next()))
        .collect();

    lines.push(String::new());
    for (label, value) in &constants {
        lines.push(format!("{}: .8byte {}", label, value));
    }
    for (label, text) in generator
        .strings
        .iter()
        .filter(|(label, _)| used.contains(label.as_str()))
    {
        lines.push(format!("{}:", label));
        for c in text.chars() {
            lines.push(format!("    .8byte {}", c as u32));
        }
        lines.push(String::from("    .8byte 0"));
    }
    lines.extend(data);
    lines.push(String::from("End:"));

    Ok(lines.join("\n") + "\n")
}

fn mangle(name: &str) -> String {
    name.chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

fn condition(op: BinOp, when: bool) -> &'static str {
    match (op, when) {
        (BinOp::Eq, true) | (BinOp::Ne, false) => "eq",
        (BinOp::Ne, true) | (BinOp::Eq, false) => "ne",
        (BinOp::Lt, true) | (BinOp::Ge, false) => "lt",
        (BinOp::Ge, true) | (BinOp::Lt, false) => "ge",
        (BinOp::Gt, true) | (BinOp::Le, false) => "gt",
        (BinOp::Le, true) | (BinOp::Gt, false) => "le",
        _ => unreachable!(),
    }
}

fn helper(op: BinOp) -> Option<&'static str> {
    match op {
        BinOp::BitAnd => Some("__and"),
        BinOp::BitOr => Some("__or"),
        BinOp::BitXor => Some("__xor"),
        BinOp::Shl => Some("__shl"),
        BinOp::Shr => Some("__shr"),
        _ => None,
    }
}

fn arithmetic_type(a: &Type, b: &Type) -> Type {
    if *a == Type::Long || *b == Type::Long {
        Type::Long
    } else {
        Type::Int
    }
}

fn flatten<'a>(ty: &Type, init: &'a Init, out: &mut Vec<Slot<'a>>) -> Result<()> {
    match (ty, init) {
        (Type::Array(element, length), Init::List(items)) => {
            if items.len() > *length {
                return Err(CompileError {
                    line: 0,
                    message: String::from("Too many values in the initializer"),
                });
            }
            for item in items {
                flatten(element, item, out)?;
            }
            let words = (*length - items.len()) as i64 * element.size() / 8;
            out.extend((0..words).map(|_| Slot::Zero));
        }
        (Type::Array(element, length), Init::Expr(expr)) => {
            let ExprKind::Str(text) = &expr.kind else {
                return Err(CompileError {
                    line: expr.line,
                    message: String::from("An array must be initialized with `{ ... }`"),
                });
            };
            if !element.is_integer() {
                return Err(CompileError {
                    line: expr.line,
                    message: String::from("Only char arrays can be initialized with a string"),
                });
            }
            let chars: Vec<i64> = text.chars().map(|c| c as i64).collect();
            if chars.len() > *length {
                return Err(CompileError {
                    line: expr.line,
                    message: String::from("The string is too long for the array"),
                });
            }
            out.extend(chars.iter().map(|c| Slot::Value(*c)));
            out.extend((chars.len()..*length).map(|_| Slot::Zero));
        }
        (_, Init::List(items)) => match items.as_slice() {
            [item] => flatten(ty, item, out)?,
            _ => {
                return Err(CompileError {
                    line: 0,
                    message: String::from("A scalar must be initialized with a single value"),
                });
            }
        },
        (_, Init::Expr(expr)) => out.push(Slot::Expr(expr)),
    }
    Ok(())
}

impl Generator {
    fn error<T>(&self, message: String) -> Result<T> {
        Err(CompileError {
            line: self.line,
            message,
        })
    }

    fn emit(&mut self, text: String) {
        self.code.push(format!("    {}", text));
    }

    fn label(&mut self, label: &str) {
        self.code.push(format!("{}:", label));
    }

    fn new_label(&mut self) -> String {
        self.labels += 1;
        format!("L{}", self.labels)
    }

    fn temp(&self, depth: usize) -> Result<String> {
        if depth >= TEMPS {
            return self.error(String::from("Expression is too deeply nested"));
        }
        Ok(format!("x{}", FIRST_TEMP + depth))
    }

    fn constant(&mut self, value: String) -> String {
        if let Some(label) = self.constant_labels.get(&value) {
            return label.clone();
        }
        let label = format!("K{}", self.constants.len());
        self.constants.push((label.clone(), value.clone()));
        self.constant_labels.insert(value, label.clone());
        label
    }

    fn load_const(&mut self, register: &str, value: i64) {
        match value {
            0 => self.emit(format!("add {}, xzr, xzr", register)),
            1 => self.emit(format!("add {}, x26, xzr", register)),
            8 => self.emit(format!("add {}, x27, xzr", register)),
            _ => {
                let label = self.constant(value.to_string());
                self.emit(format!("ldr {}, {}", register, label));
            }
        }
    }

    fn load_address(&mut self, register: &str, label: &str) {
        let constant = self.constant(label.to_string());
        self.emit(format!("ldr {}, {}", register, constant));
    }

    fn memory(&mut self, op: &str, register: &str, base: &str, offset: i64) {
        if (-256..=255).contains(&offset) {
            self.emit(format!("{} {}, [{}, {}]", op, register, base, offset));
        } else {
            self.load_const("x8", offset);
            self.emit(format!("add x8, {}, x8", base));
            self.emit(format!("{} {}, [x8, 0]", op, register));
        }
    }

    fn push(&mut self, register: &str) {
        self.emit(String::from("sub x28, x28, x27"));
        self.emit(format!("stur {}, [x28, 0]", register));
    }

    fn drop_stack(&mut self, bytes: i64) {
        if bytes == 8 {
            self.emit(String::from("add x28, x28, x27"));
        } else if bytes > 0 {
            self.load_const("x8", bytes);
            self.emit(String::from("add x28, x28, x8"));
        }
    }

    fn set_if(&mut self, register: &str, condition: &str) {
        let done = self.new_label();
        self.emit(format!("add {}, x26, xzr", register));
        self.emit(format!("b.{} {}", condition, done));
        self.emit(format!("add {}, xzr, xzr", register));
        self.label(&done);
    }

    fn global_data(&mut self, global: &Variable) -> Result<Vec<String>> {
        self.line = global.line;
        let mut slots = Vec::new();
        match &global.init {
            Some(init) => flatten(&global.ty, init, &mut slots).map_err(|e| self.relocate(e))?,
            None => slots.extend((0..global.ty.size() / 8).map(|_| Slot::Zero)),
        }
        let mut values = Vec::new();
        for slot in slots {
            values.push(match slot {
                Slot::Zero => String::from("0"),
                Slot::Value(value) => value.to_string(),
                Slot::Expr(expr) => match (&expr.kind, const_eval(expr)) {
                    (_, Some(value)) => value.to_string(),
                    (ExprKind::Str(text), None) => self.string(text),
                    _ => {
                        return self.error(format!(
                            "The initializer of global `{}` must be a constant",
                            global.name
                        ));
                    }
                },
            });
        }
        Ok(values)
    }

    fn relocate(&self, error: CompileError) -> CompileError {
        CompileError {
            line: if error.line == 0 { self.line } else { error.line },
            message: error.message,
        }
    }

    fn string(&mut self, text: &str) -> String {
        let label = format!("S{}", self.strings.len());
        self.strings.push((label.clone(), text.to_string()));
        label
    }

    fn lookup(&self, name: &str) -> Option<(Option<i64>, Type, String)> {
        for scope in self.scopes.iter().rev() {
            if let Some(local) = scope.get(name) {
                return Some((Some(local.offset), local.ty.clone(), String::new()));
            }
        }
        self.globals
            .get(name)
            .map(|g| (None, g.ty.clone(), g.label.clone()))
    }

    fn function(&mut self, function: &Function) -> Result<Vec<String>> {
        self.line = function.line;
        self.code = Vec::new();
        self.frame = 0;
        self.loops = Vec::new();
        self.return_label = self.new_label();

        let count = function.params.len() as i64;
        let mut params = HashMap::new();
        for (i, (name, ty)) in function.params.iter().enumerate() {
            if *ty == Type::Void {
                return self.error(format!("Parameter `{}` cannot have type void", name));
            }
            let local = Local {
                offset: 16 + 8 * (count - 1 - i as i64),
                ty: ty.clone(),
            };
            if params.insert(name.clone(), local).is_some() {
                return self.error(format!("Parameter `{}` is declared twice", name));
            }
        }
        self.scopes = vec![params];

        for stmt in function.body.as_ref().unwrap() {
            self.statement(stmt)?;
        }
        self.emit(String::from("add x0, xzr, xzr"));
        let return_label = self.return_label.clone();
        self.label(&return_label);
        self.emit(String::from("add x28, x29, xzr"));
        self.emit(String::from("ldur x29, [x28, 0]"));
        self.emit(String::from("ldur x30, [x28, 8]"));
        self.drop_stack(16);
        self.emit(String::from("br x30"));

        let body = std::mem::take(&mut self.code);
        let frame = self.frame;
        self.label(&self.signatures[&function.name].label.clone());
        self.push("x30");
        self.push("x29");
        self.emit(String::from("add x29, x28, xzr"));
        if frame > 0 {
            self.load_const("x8", frame);
            self.emit(String::from("sub x28, x28, x8"));
        }
        let mut code = std::mem::take(&mut self.code);
        code.extend(body);
        Ok(code)
    }

    fn statement(&mut self, stmt: &Stmt) -> Result<()> {
        match stmt {
            Stmt::Decl(variables) => {
                for variable in variables {
                    self.declare(variable)?;
                }
            }
            Stmt::Expr(expr) => {
                self.expr(expr, 0)?;
            }
            Stmt::If(cond, then, otherwise) => {
                let else_label = self.new_label();
                self.jump(cond, 0, &else_label, false)?;
                self.scoped(then)?;
                match otherwise {
                    Some(otherwise) => {
                        let end = self.new_label();
                        self.emit(format!("b {}", end));
                        self.label(&else_label);
                        self.scoped(otherwise)?;
                        self.label(&end);
                    }
                    None => self.label(&else_label),
                }
            }
            Stmt::While(cond, body) => {
                let top = self.new_label();
                let end = self.new_label();
                self.label(&top);
                self.jump(cond, 0, &end, false)?;
                self.loop_body(body, &top, &end)?;
                self.emit(format!("b {}", top));
                self.label(&end);
            }
            Stmt::DoWhile(body, cond) => {
                let top = self.new_label();
                let next = self.new_label();
                let end = self.new_label();
                self.label(&top);
                self.loop_body(body, &next, &end)?;
                self.label(&next);
                self.jump(cond, 0, &top, true)?;
                self.label(&end);
            }
            Stmt::For(init, cond, step, body) => {
                self.scopes.push(HashMap::new());
                if let Some(init) = init {
                    self.statement(init)?;
                }
                let top = self.new_label();
                let next = self.new_label();
                let end = self.new_label();
                self.label(&top);
                if let Some(cond) = cond {
                    self.jump(cond, 0, &end, false)?;
                }
                self.loop_body(body, &next, &end)?;
                self.label(&next);
                if let Some(step) = step {
                    self.expr(step, 0)?;
                }
                self.emit(format!("b {}", top));
                self.label(&end);
                self.scopes.pop();
            }
            Stmt::Return(value, line) => {
                self.line = *line;
                if let Some(value) = value {
                    self.expr(value, 0)?;
                    self.emit(format!("add x0, {}, xzr", self.temp(0)?));
                }
                self.emit(format!("b {}", self.return_label));
            }
            Stmt::Break(line) | Stmt::Continue(line) => {
                self.line = *line;
                let Some((next, end)) = self.loops.last() else {
                    return self.error(String::from("`break` or `continue` outside of a loop"));
                };
                let target = if matches!(stmt, Stmt::Break(_)) { end } else { next };
                self.emit(format!("b {}", target.clone()));
            }
            Stmt::Block(stmts) => {
                self.scopes.push(HashMap::new());
                for stmt in stmts {
                    self.statement(stmt)?;
                }
                self.scopes.pop();
            }
            Stmt::Empty => {}
        }
        Ok(())
    }

    fn scoped(&mut self, stmt: &Stmt) -> Result<()> {
        self.scopes.push(HashMap::new());
        let result = self.statement(stmt);
        self.scopes.pop();
        result
    }

    fn loop_body(&mut self, body: &Stmt, next: &str, end: &str) -> Result<()> {
        self.loops.push((next.to_string(), end.to_string()));
        let result = self.scoped(body);
        self.loops.pop();
        result
    }

    fn declare(&mut self, variable: &Variable) -> Result<()> {
        self.line = variable.line;
        if self.scopes.last().unwrap().contains_key(&variable.name) {
            return self.error(format!(
                "`{}` is already declared in this scope",
                variable.name
            ));
        }
        self.frame += variable.ty.size();
        let offset = -self.frame;

        let mut slots = Vec::new();
        if let Some(init) = &variable.init {
            flatten(&variable.ty, init, &mut slots).map_err(|e| self.relocate(e))?;
        }
        for (i, slot) in slots.into_iter().enumerate() {
            let register = match slot {
                Slot::Zero => String::from("xzr"),
                Slot::Value(value) => {
                    let register = self.temp(0)?;
                    self.load_const(&register, value);
                    register
                }
                Slot::Expr(expr) => {
                    self.expr(expr, 0)?;
                    self.temp(0)?
                }
            };
            self.memory("stur", &register, "x29", offset + 8 * i as i64);
        }

        self.scopes.last_mut().unwrap().insert(
            variable.name.clone(),
            Local {
                offset,
                ty: variable.ty.clone(),
            },
        );
        Ok(())
    }

    fn jump(&mut self, expr: &Expr, depth: usize, target: &str, when: bool) -> Result<()> {
        self.line = expr.line;
        match &expr.kind {
            ExprKind::Binary(op, left, right) if op.is_comparison() => {
                self.expr(left, depth)?;
                self.expr(right, depth + 1)?;
                let (a, b) = (self.temp(depth)?, self.temp(depth + 1)?);
                self.emit(format!("cmp {}, {}", a, b));
                self.emit(format!("b.{} {}", condition(*op, when), target));
            }
            ExprKind::Binary(op @ (BinOp::LogAnd | BinOp::LogOr), left, right) => {
                let short = (*op == BinOp::LogOr) == when;
                if short {
                    self.jump(left, depth, target, when)?;
                    self.jump(right, depth, target, when)?;
                } else {
                    let skip = self.new_label();
                    self.jump(left, depth, &skip, !when)?;
                    self.jump(right, depth, target, when)?;
                    self.label(&skip);
                }
            }
            ExprKind::Unary(UnOp::Not, inner) => self.jump(inner, depth, target, !when)?,
            ExprKind::Num(value) => {
                if (*value != 0) == when {
                    self.emit(format!("b {}", target));
                }
            }
            _ => {
                self.expr(expr, depth)?;
                let register = self.temp(depth)?;
                self.emit(format!("cmp {}, xzr", register));
                let condition = if when { "ne" } else { "eq" };
                self.emit(format!("b.{} {}", condition, target));
            }
        }
        Ok(())
    }

    fn type_of(&mut self, expr: &Expr) -> Result<Type> {
        let code = self.code.len();
        let constants = self.constants.len();
        let strings = self.strings.len();
        let calls = self.calls.clone();
        let result = match &expr.kind {
            ExprKind::Var(_) | ExprKind::Deref(_) | ExprKind::Index(..) => self.address(expr, 0),
            ExprKind::Str(text) => Ok(Type::Array(
                Box::new(Type::Char),
                text.chars().count() + 1,
            )),
            _ => self.expr(expr, 0),
        };
        self.code.truncate(code);
        for (_, value) in self.constants.drain(constants..) {
            self.constant_labels.remove(&value);
        }
        self.strings.truncate(strings);
        self.calls = calls;
        result
    }

    fn address(&mut self, expr: &Expr, depth: usize) -> Result<Type> {
        self.line = expr.line;
        let register = self.temp(depth)?;
        match &expr.kind {
            ExprKind::Var(name) => match self.lookup(name) {
                Some((Some(offset), ty, _)) => {
                    self.load_const(&register, offset);
                    self.emit(format!("add {}, x29, {}", register, register));
                    Ok(ty)
                }
                Some((None, ty, label)) => {
                    self.load_address(&register, &label);
                    Ok(ty)
                }
                None => self.error(format!("Unknown variable `{}`", name)),
            },
            ExprKind::Deref(inner) => {
                let ty = self.expr(inner, depth)?;
                self.line = expr.line;
                self.target(ty)
            }
            ExprKind::Index(array, index) => {
                let ty = self.binary(BinOp::Add, array, index, depth)?;
                self.line = expr.line;
                self.target(ty)
            }
            _ => self.error(String::from("This expression is not assignable")),
        }
    }

    fn target(&self, ty: Type) -> Result<Type> {
        match ty.target() {
            Some(Type::Void) => self.error(String::from("Cannot dereference a void pointer")),
            Some(target) => Ok(target.clone()),
            None => self.error(format!("Cannot dereference a value of type {}", ty.name())),
        }
    }

    fn local(&self, expr: &Expr) -> Option<(i64, Type)> {
        let ExprKind::Var(name) = &expr.kind else {
            return None;
        };
        match self.lookup(name)? {
            (Some(offset), ty, _) if !matches!(ty, Type::Array(..)) => Some((offset, ty)),
            _ => None,
        }
    }

    fn load(&mut self, expr: &Expr, depth: usize) -> Result<Type> {
        if let Some((offset, ty)) = self.local(expr) {
            let register = self.temp(depth)?;
            self.memory("ldur", &register, "x29", offset);
            return Ok(ty);
        }
        let ty = self.address(expr, depth)?;
        if matches!(ty, Type::Array(..)) {
            return Ok(ty.decay());
        }
        let register = self.temp(depth)?;
        self.emit(format!("ldur {}, [{}, 0]", register, register));
        Ok(ty)
    }

    fn expr(&mut self, expr: &Expr, depth: usize) -> Result<Type> {
        self.line = expr.line;
        let register = self.temp(depth)?;
        match &expr.kind {
            ExprKind::Num(value) => {
                self.load_const(&register, *value);
                Ok(Type::Int)
            }
            ExprKind::Str(text) => {
                let label = self.string(text);
                self.load_address(&register, &label);
                Ok(Type::Char.pointer_to())
            }
            ExprKind::Var(_) | ExprKind::Deref(_) | ExprKind::Index(..) => self.load(expr, depth),
            ExprKind::Addr(inner) => Ok(self.address(inner, depth)?.pointer_to()),
            ExprKind::Unary(op, inner) => {
                let ty = self.expr(inner, depth)?;
                match op {
                    UnOp::Neg => {
                        self.emit(format!("sub {}, xzr, {}", register, register));
                        Ok(ty)
                    }
                    UnOp::Not => {
                        self.emit(format!("cmp {}, xzr", register));
                        self.set_if(&register, "eq");
                        Ok(Type::Int)
                    }
                    UnOp::BitNot => {
                        self.emit(format!("sub {}, xzr, {}", register, register));
                        self.emit(format!("sub {}, {}, x26", register, register));
                        Ok(ty)
                    }
                }
            }
            ExprKind::Binary(op, left, right) => self.binary(*op, left, right, depth),
            ExprKind::Assign(op, left, right) => self.assign(*op, left, right, depth),
            ExprKind::Cond(cond, then, otherwise) => {
                let else_label = self.new_label();
                let end = self.new_label();
                self.jump(cond, depth, &else_label, false)?;
                let ty = self.expr(then, depth)?;
                self.emit(format!("b {}", end));
                self.label(&else_label);
                self.expr(otherwise, depth)?;
                self.label(&end);
                Ok(ty)
            }
            ExprKind::Call(name, args) => self.call(name, args, depth),
            ExprKind::IncDec(prefix, delta, inner) => {
                let ty = self.address(inner, depth)?;
                if matches!(ty, Type::Array(..)) {
                    return self.error(String::from("Cannot increment an array"));
                }
                let value = self.temp(depth + 1)?;
                let updated = self.temp(depth + 2)?;
                let step = match ty.target() {
                    Some(target) => target.size(),
                    None => 1,
                };
                self.emit(format!("ldur {}, [{}, 0]", value, register));
                self.load_const(&updated, step);
                let op = if *delta > 0 { "add" } else { "sub" };
                self.emit(format!("{} {}, {}, {}", op, updated, value, updated));
                self.emit(format!("stur {}, [{}, 0]", updated, register));
                let result = if *prefix { updated } else { value };
                self.emit(format!("add {}, {}, xzr", register, result));
                Ok(ty)
            }
            ExprKind::SizeofType(ty) => {
                self.load_const(&register, ty.size());
                Ok(Type::Long)
            }
            ExprKind::SizeofExpr(inner) => {
                let ty = self.type_of(inner)?;
                self.load_const(&register, ty.size());
                Ok(Type::Long)
            }
            ExprKind::Cast(ty, inner) => {
                self.expr(inner, depth)?;
                Ok(ty.clone())
            }
            ExprKind::Comma(first, second) => {
                self.expr(first, depth)?;
                self.expr(second, depth)
            }
        }
    }

    fn binary(&mut self, op: BinOp, left: &Expr, right: &Expr, depth: usize) -> Result<Type> {
        let register = self.temp(depth)?;
        if op == BinOp::LogAnd || op == BinOp::LogOr {
            let short = self.new_label();
            let end = self.new_label();
            let when = op == BinOp::LogOr;
            self.jump(left, depth, &short, when)?;
            self.jump(right, depth, &short, when)?;
            self.load_const(&register, (!when) as i64);
            self.emit(format!("b {}", end));
            self.label(&short);
            self.load_const(&register, when as i64);
            self.label(&end);
            return Ok(Type::Int);
        }
        if op == BinOp::Shl {
            if let Some(shift @ 0..=62) = const_eval(right) {
                let ty = self.expr(left, depth)?;
                let factor = self.temp(depth + 1)?;
                self.load_const(&factor, 1 << shift);
                self.emit(format!("mul {}, {}, {}", register, register, factor));
                return Ok(ty);
            }
        }
        let line = self.line;
        let a = self.expr(left, depth)?;
        let b = self.expr(right, depth + 1)?;
        self.line = line;
        self.arithmetic(op, depth, a, b)
    }

    fn arithmetic(&mut self, op: BinOp, depth: usize, a: Type, b: Type) -> Result<Type> {
        let x = self.temp(depth)?;
        let y = self.temp(depth + 1)?;
        let invalid = |this: &Self| {
            this.error(format!(
                "Invalid operands to `{}`: {} and {}",
                op.symbol(),
                a.name(),
                b.name()
            ))
        };
        if a == Type::Void || b == Type::Void {
            return invalid(self);
        }
        let a_pointer = a.target().is_some();
        let b_pointer = b.target().is_some();
        if let Some(name) = helper(op) {
            if a_pointer || b_pointer {
                return invalid(self);
            }
            self.call_helper(name, depth)?;
            return Ok(arithmetic_type(&a, &b));
        }
        match op {
            BinOp::Add | BinOp::Sub => {
                let instruction = if op == BinOp::Add { "add" } else { "sub" };
                match (a_pointer, b_pointer) {
                    (false, false) => {
                        self.emit(format!("{} {}, {}, {}", instruction, x, x, y));
                        Ok(arithmetic_type(&a, &b))
                    }
                    (true, false) => {
                        self.scale(&y, a.target().unwrap().size(), depth + 2)?;
                        self.emit(format!("{} {}, {}, {}", instruction, x, x, y));
                        Ok(a.decay())
                    }
                    (false, true) if op == BinOp::Add => {
                        self.scale(&x, b.target().unwrap().size(), depth + 2)?;
                        self.emit(format!("add {}, {}, {}", x, x, y));
                        Ok(b.decay())
                    }
                    (true, true) if op == BinOp::Sub => {
                        self.emit(format!("sub {}, {}, {}", x, x, y));
                        let size = a.target().unwrap().size();
                        if size != 1 {
                            self.load_const(&y, size);
                            self.emit(format!("sdiv {}, {}, {}", x, x, y));
                        }
                        Ok(Type::Long)
                    }
                    _ => invalid(self),
                }
            }
            BinOp::Mul | BinOp::Div | BinOp::Mod => {
                if a_pointer || b_pointer {
                    return invalid(self);
                }
                match op {
                    BinOp::Mul => self.emit(format!("mul {}, {}, {}", x, x, y)),
                    BinOp::Div => self.emit(format!("sdiv {}, {}, {}", x, x, y)),
                    _ => {
                        let z = self.temp(depth + 2)?;
                        self.emit(format!("sdiv {}, {}, {}", z, x, y));
                        self.emit(format!("mul {}, {}, {}", z, z, y));
                        self.emit(format!("sub {}, {}, {}", x, x, z));
                    }
                }
                Ok(arithmetic_type(&a, &b))
            }
            _ if op.is_comparison() => {
                self.emit(format!("cmp {}, {}", x, y));
                self.set_if(&x, condition(op, true));
                Ok(Type::Int)
            }
            _ => invalid(self),
        }
    }

    fn scale(&mut self, register: &str, size: i64, scratch: usize) -> Result<()> {
        match size {
            1 => {}
            8 => self.emit(format!("mul {}, {}, x27", register, register)),
            _ => {
                let factor = self.temp(scratch)?;
                self.load_const(&factor, size);
                self.emit(format!("mul {}, {}, {}", register, register, factor));
            }
        }
        Ok(())
    }

    fn assign(&mut self, op: Option<BinOp>, left: &Expr, right: &Expr, depth: usize) -> Result<Type> {
        let register = self.temp(depth)?;
        match op {
            None => {
                self.expr(right, depth)?;
                if let Some((offset, ty)) = self.local(left) {
                    self.memory("stur", &register, "x29", offset);
                    return Ok(ty);
                }
                let ty = self.address(left, depth + 1)?;
                if matches!(ty, Type::Array(..)) {
                    return self.error(String::from("Cannot assign to an array"));
                }
                let address = self.temp(depth + 1)?;
                self.emit(format!("stur {}, [{}, 0]", register, address));
                Ok(ty)
            }
            Some(op) => {
                let ty = self.address(left, depth)?;
                if matches!(ty, Type::Array(..)) {
                    return self.error(String::from("Cannot assign to an array"));
                }
                let value = self.temp(depth + 1)?;
                self.emit(format!("ldur {}, [{}, 0]", value, register));
                let b = self.expr(right, depth + 2)?;
                self.arithmetic(op, depth + 1, ty.clone(), b)?;
                self.emit(format!("stur {}, [{}, 0]", value, register));
                self.emit(format!("add {}, {}, xzr", register, value));
                Ok(ty)
            }
        }
    }

    fn save(&mut self, depth: usize) -> Result<()> {
        if depth == 0 {
            return Ok(());
        }
        if depth == 1 {
            let register = self.temp(0)?;
            self.push(&register);
            return Ok(());
        }
        self.load_const("x8", depth as i64 * 8);
        self.emit(String::from("sub x28, x28, x8"));
        for i in 0..depth {
            let register = self.temp(i)?;
            self.emit(format!("stur {}, [x28, {}]", register, i * 8));
        }
        Ok(())
    }

    fn restore(&mut self, depth: usize) -> Result<()> {
        for i in 0..depth {
            let register = self.temp(i)?;
            self.emit(format!("ldur {}, [x28, {}]", register, i * 8));
        }
        self.drop_stack(depth as i64 * 8);
        Ok(())
    }

    fn call_helper(&mut self, name: &str, depth: usize) -> Result<()> {
        let label = match self.signatures.get(name) {
            Some(signature) => signature.label.clone(),
            None => return self.error(format!("Missing runtime function `{}`", name)),
        };
        self.calls.insert(name.to_string());
        let (x, y) = (self.temp(depth)?, self.temp(depth + 1)?);
        self.save(depth)?;
        self.push(&x);
        self.push(&y);
        self.load_address("x8", &label);
        self.emit(String::from("blr x8"));
        self.drop_stack(16);
        self.restore(depth)?;
        self.emit(format!("add {}, x0, xzr", x));
        Ok(())
    }

    fn call(&mut self, name: &str, args: &[Expr], depth: usize) -> Result<Type> {
        let register = self.temp(depth)?;
        let defined = self.signatures.get(name).is_some_and(|s| s.defined);
        if !defined && INTRINSICS.contains(&name) {
            return self.intrinsic(name, args, depth);
        }
        let Some(signature) = self.signatures.get(name) else {
            return self.error(format!("Unknown function `{}`", name));
        };
        if !signature.defined {
            return self.error(format!("Function `{}` is declared but never defined", name));
        }
        if signature.params.len() != args.len() {
            return self.error(format!(
                "`{}` takes {} argument{} but was given {}",
                name,
                signature.params.len(),
                if signature.params.len() == 1 { "" } else { "s" },
                args.len()
            ));
        }
        let label = signature.label.clone();
        let ret = signature.ret.clone();
        self.calls.insert(name.to_string());

        self.save(depth)?;
        for arg in args {
            if self.expr(arg, depth)? == Type::Void {
                return self.error(String::from("A void value cannot be used as an argument"));
            }
            self.push(&register);
        }
        self.load_address("x8", &label);
        self.emit(String::from("blr x8"));
        self.drop_stack(args.len() as i64 * 8);
        self.restore(depth)?;
        if ret != Type::Void {
            self.emit(format!("add {}, x0, xzr", register));
        }
        Ok(ret)
    }

    fn arity(&self, name: &str, args: &[Expr], count: usize) -> Result<()> {
        if args.len() != count {
            return self.error(format!(
                "`{}` takes {} argument{} but was given {}",
                name,
                count,
                if count == 1 { "" } else { "s" },
                args.len()
            ));
        }
        Ok(())
    }

    fn intrinsic(&mut self, name: &str, args: &[Expr], depth: usize) -> Result<Type> {
        let register = self.temp(depth)?;
        match name {
            "putchar" => {
                self.arity(name, args, 1)?;
                self.expr(&args[0], depth)?;
                self.load_const("x8", OUTPUT_ADDRESS);
                self.emit(format!("stur {}, [x8, 0]", register));
                Ok(Type::Int)
            }
            "getchar" => {
                self.arity(name, args, 0)?;
                self.load_const("x8", INPUT_ADDRESS);
                self.emit(format!("ldur {}, [x8, 0]", register));
                Ok(Type::Int)
            }
            "exit" => {
                self.arity(name, args, 1)?;
                self.expr(&args[0], depth)?;
                self.emit(format!("add x0, {}, xzr", register));
                self.emit(String::from("br x25"));
                Ok(Type::Void)
            }
            "__heap_start" => {
                self.arity(name, args, 0)?;
                self.load_address(&register, "End");
                Ok(Type::Void.pointer_to())
            }
            _ => self.printf(args, depth),
        }
    }

    fn printf(&mut self, args: &[Expr], depth: usize) -> Result<Type> {
        let line = self.line;
        let Some(Expr {
            kind: ExprKind::Str(format),
            ..
        }) = args.first()
        else {
            return self.error(String::from("printf needs a string literal as its format"));
        };
        let mut values = args[1..].iter();
        let mut text = String::new();
        let mut chars = format.chars();
        let mut pieces: Vec<(&str, Expr)> = Vec::new();
        let flush = |text: &mut String, pieces: &mut Vec<(&str, Expr)>| {
            match text.chars().count() {
                0 => {}
                1 => pieces.push((
                    "putchar",
                    Expr::new(ExprKind::Num(text.chars().next().unwrap() as i64), line),
                )),
                _ => pieces.push(("print_str", Expr::new(ExprKind::Str(text.clone()), line))),
            }
            text.clear();
        };
        while let Some(c) = chars.next() {
            if c != '%' {
                text.push(c);
                continue;
            }
            let mut spec = chars.next();
            while spec == Some('l') {
                spec = chars.next();
            }
            let function = match spec {
                Some('%') => {
                    text.push('%');
                    continue;
                }
                Some('d' | 'i' | 'u') => "print_int",
                Some('c') => "putchar",
                Some('s') => "print_str",
                Some('x' | 'p') => "__print_hex",
                Some(other) => return self.error(format!("Unsupported printf format `%{}`", other)),
                None => return self.error(String::from("printf format ends with `%`")),
            };
            let Some(value) = values.next() else {
                return self.error(String::from("printf has fewer arguments than formats"));
            };
            flush(&mut text, &mut pieces);
            pieces.push((function, value.clone()));
        }
        flush(&mut text, &mut pieces);
        if values.next().is_some() {
            return self.error(String::from("printf has more arguments than formats"));
        }
        for (function, arg) in pieces {
            self.call(function, std::slice::from_ref(&arg), depth)?;
        }
        let register = self.temp(depth)?;
        self.load_const(&register, 0);
        Ok(Type::Int)
    }
}
