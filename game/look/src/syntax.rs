//! A look's syntax: Contract's habits (indented blocks, `fn … =`, `match { case
//! … => … }`, `name=value` arguments), not its grammar. Logical lines continue
//! while a bracket is open; a block is the lines indented under its head.

/// A source position, 1-based.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub line: u32,
    pub col: u32,
}

/// A refusal with its position.
#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    pub at: Span,
    pub message: String,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.at.line, self.at.col, self.message)
    }
}
pub(crate) fn error(at: Span, message: impl Into<String>) -> Error {
    Error {
        at,
        message: message.into(),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Num(f32),
    Str(String),
    Id(String),
    Sym(&'static str),
}

#[derive(Clone, Debug)]
struct Line {
    indent: usize,
    toks: Vec<(Tok, Span)>,
}

const SYMS: [&str; 26] = [
    "=>", "==", "!=", "<=", ">=", "&&", "||", "+", "-", "*", "/", "%", "<", ">", "!", "?", ":",
    "=", ",", ".", "(", ")", "{", "}", "[", "]",
];

fn lines(src: &str) -> Result<Vec<Line>, Error> {
    let mut out: Vec<Line> = Vec::new();
    let mut depth = 0usize;
    for (n, text) in src.lines().enumerate() {
        let line = n as u32 + 1;
        // A bracket left open on an earlier line continues that logical line.
        let continuing = depth > 0;
        let chars: Vec<char> = text.chars().collect();
        let indent = chars.iter().take_while(|c| **c == ' ').count();
        let mut toks = Vec::new();
        let mut i = indent;
        while i < chars.len() {
            let c = chars[i];
            let at = Span {
                line,
                col: i as u32 + 1,
            };
            if c == ' ' {
                i += 1;
            } else if c == '/' && chars.get(i + 1) == Some(&'/') {
                break;
            } else if c.is_ascii_digit() {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_ascii_digit()
                        || chars[i] == '.'
                        || chars[i] == 'e'
                        || ((chars[i] == '-' || chars[i] == '+') && chars[i - 1] == 'e'))
                {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                let v = s
                    .parse::<f32>()
                    .map_err(|_| error(at, format!("`{s}` is not a number")))?;
                toks.push((Tok::Num(v), at));
            } else if c.is_alphabetic() || c == '_' {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                toks.push((Tok::Id(chars[start..i].iter().collect()), at));
            } else if c == '"' {
                let start = i + 1;
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    i += 1;
                }
                if i == chars.len() {
                    return Err(error(at, "unterminated string"));
                }
                toks.push((Tok::Str(chars[start..i].iter().collect()), at));
                i += 1;
            } else {
                let rest: String = chars[i..chars.len().min(i + 2)].iter().collect();
                let Some(sym) = SYMS.iter().find(|s| rest.starts_with(**s)) else {
                    return Err(error(at, format!("unexpected `{c}`")));
                };
                match *sym {
                    "(" | "{" | "[" => depth += 1,
                    ")" | "}" | "]" => depth = depth.saturating_sub(1),
                    _ => {}
                }
                toks.push((Tok::Sym(sym), at));
                i += sym.len();
            }
        }
        if toks.is_empty() {
            continue;
        }
        match out.last_mut() {
            Some(open) if continuing => open.toks.extend(toks),
            _ => out.push(Line { indent, toks }),
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------- the tree

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Neg,
    Not,
}

#[derive(Clone, Debug)]
pub enum Expr {
    Num(f32, Span),
    Str(String, Span),
    Name(String, Span),
    Field(Box<Expr>, String, Span),
    Index(Box<Expr>, Box<Expr>, Span),
    Call(String, Vec<Expr>, Span),
    Unary(Op, Box<Expr>, Span),
    Binary(Op, Box<Expr>, Box<Expr>, Span),
    Cond(Box<Expr>, Box<Expr>, Box<Expr>, Span),
    Is(Box<Expr>, String, Span),
    Record(Vec<(String, Expr)>, Span),
    Match(Box<Expr>, Vec<Arm>, Span),
}
impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Num(_, s)
            | Expr::Str(_, s)
            | Expr::Name(_, s)
            | Expr::Field(_, _, s)
            | Expr::Index(_, _, s)
            | Expr::Call(_, _, s)
            | Expr::Unary(_, _, s)
            | Expr::Binary(_, _, _, s)
            | Expr::Cond(_, _, _, s)
            | Expr::Is(_, _, s)
            | Expr::Record(_, s)
            | Expr::Match(_, _, s) => *s,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Arm {
    /// None is `_`; otherwise a name and an optional binder (`case Child c`).
    pub pat: Option<(String, Option<String>)>,
    pub body: Expr,
    pub at: Span,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Let(String, Expr, Span),
    Guard(Expr, Span),
    If(Expr, Vec<Stmt>, Vec<Stmt>, Span),
    Each {
        entity: String,
        value: Option<String>,
        component: String,
        body: Vec<Stmt>,
        at: Span,
    },
    For {
        index: Option<String>,
        item: String,
        list: Expr,
        body: Vec<Stmt>,
        at: Span,
    },
    Emit {
        row: String,
        entity: Expr,
        args: Vec<(Option<String>, Expr)>,
        at: Span,
    },
}

/// A fn body's `let` lines.
pub type Lets = Vec<(String, Expr, Span)>;

#[derive(Clone, Debug)]
pub struct FnDecl {
    pub name: String,
    pub params: Vec<(String, String)>,
    pub lets: Lets,
    pub body: Expr,
    pub at: Span,
}

#[derive(Clone, Debug, Default)]
pub struct File {
    pub consts: Vec<(String, Expr, Span)>,
    pub fns: Vec<FnDecl>,
    /// Top-level statements, in order, interleaved with declarations by line.
    pub body: Vec<Stmt>,
    /// For each fn, how many top-level statements precede it.
    pub fn_after: Vec<usize>,
}

// ---------------------------------------------------------------- parsing

struct P<'a> {
    toks: &'a [(Tok, Span)],
    i: usize,
    end: Span,
}
impl<'a> P<'a> {
    fn peek(&self) -> Option<&'a Tok> {
        self.toks.get(self.i).map(|(t, _)| t)
    }
    fn at(&self) -> Span {
        self.toks.get(self.i).map_or(self.end, |(_, s)| *s)
    }
    fn sym(&self, s: &str) -> bool {
        matches!(self.peek(), Some(Tok::Sym(x)) if *x == s)
    }
    fn word(&self, w: &str) -> bool {
        matches!(self.peek(), Some(Tok::Id(x)) if x == w)
    }
    fn eat(&mut self, s: &str) -> bool {
        let hit = self.sym(s) || self.word(s);
        if hit {
            self.i += 1;
        }
        hit
    }
    fn expect(&mut self, s: &str) -> Result<(), Error> {
        if self.eat(s) {
            Ok(())
        } else {
            Err(error(self.at(), format!("expected `{s}`")))
        }
    }
    fn ident(&mut self) -> Result<(String, Span), Error> {
        match self.peek() {
            Some(Tok::Id(x)) => {
                let at = self.at();
                self.i += 1;
                Ok((x.clone(), at))
            }
            _ => Err(error(self.at(), "expected a name")),
        }
    }
    fn done(&self) -> Result<(), Error> {
        if self.i < self.toks.len() {
            Err(error(self.at(), "unexpected text after the end"))
        } else {
            Ok(())
        }
    }

    fn expr(&mut self) -> Result<Expr, Error> {
        let c = self.binary(0)?;
        if self.sym("?") {
            let at = self.at();
            self.i += 1;
            let a = self.expr()?;
            self.expect(":")?;
            let b = self.expr()?;
            return Ok(Expr::Cond(Box::new(c), Box::new(a), Box::new(b), at));
        }
        Ok(c)
    }
    fn binary(&mut self, min: u8) -> Result<Expr, Error> {
        let mut left = self.unary()?;
        loop {
            let at = self.at();
            if min <= 3 && self.word("is") {
                self.i += 1;
                let (arm, _) = self.ident()?;
                left = Expr::Is(Box::new(left), arm, at);
                continue;
            }
            let (op, level) = match self.peek() {
                Some(Tok::Sym("||")) => (Op::Or, 1),
                Some(Tok::Sym("&&")) => (Op::And, 2),
                Some(Tok::Sym("==")) => (Op::Eq, 3),
                Some(Tok::Sym("!=")) => (Op::Ne, 3),
                Some(Tok::Sym("<")) => (Op::Lt, 3),
                Some(Tok::Sym("<=")) => (Op::Le, 3),
                Some(Tok::Sym(">")) => (Op::Gt, 3),
                Some(Tok::Sym(">=")) => (Op::Ge, 3),
                Some(Tok::Sym("+")) => (Op::Add, 4),
                Some(Tok::Sym("-")) => (Op::Sub, 4),
                Some(Tok::Sym("*")) => (Op::Mul, 5),
                Some(Tok::Sym("/")) => (Op::Div, 5),
                Some(Tok::Sym("%")) => (Op::Rem, 5),
                _ => return Ok(left),
            };
            if level < min {
                return Ok(left);
            }
            self.i += 1;
            let right = self.binary(level + 1)?;
            left = Expr::Binary(op, Box::new(left), Box::new(right), at);
        }
    }
    fn unary(&mut self) -> Result<Expr, Error> {
        let at = self.at();
        if self.eat("-") {
            return Ok(Expr::Unary(Op::Neg, Box::new(self.unary()?), at));
        }
        if self.eat("!") {
            return Ok(Expr::Unary(Op::Not, Box::new(self.unary()?), at));
        }
        self.postfix()
    }
    fn postfix(&mut self) -> Result<Expr, Error> {
        let mut e = self.primary()?;
        loop {
            let at = self.at();
            if self.eat(".") {
                let (name, _) = self.ident()?;
                e = Expr::Field(Box::new(e), name, at);
            } else if self.eat("[") {
                let i = self.expr()?;
                self.expect("]")?;
                e = Expr::Index(Box::new(e), Box::new(i), at);
            } else {
                return Ok(e);
            }
        }
    }
    fn args(&mut self) -> Result<Vec<Expr>, Error> {
        let mut args = Vec::new();
        if !self.eat(")") {
            loop {
                args.push(self.expr()?);
                if self.eat(")") {
                    break;
                }
                self.expect(",")?;
            }
        }
        Ok(args)
    }
    fn primary(&mut self) -> Result<Expr, Error> {
        let at = self.at();
        match self.peek().cloned() {
            Some(Tok::Num(v)) => {
                self.i += 1;
                Ok(Expr::Num(v, at))
            }
            Some(Tok::Str(s)) => {
                self.i += 1;
                Ok(Expr::Str(s, at))
            }
            Some(Tok::Sym("(")) => {
                self.i += 1;
                let e = self.expr()?;
                self.expect(")")?;
                Ok(e)
            }
            Some(Tok::Sym("{")) => {
                self.i += 1;
                let mut fields = Vec::new();
                while !self.eat("}") {
                    let (name, _) = self.ident()?;
                    self.expect(":")?;
                    fields.push((name, self.expr()?));
                    self.eat(",");
                }
                Ok(Expr::Record(fields, at))
            }
            Some(Tok::Id(w)) if w == "match" => {
                self.i += 1;
                let scrutinee = self.expr()?;
                self.expect("{")?;
                let mut arms = Vec::new();
                while !self.eat("}") {
                    let at = self.at();
                    self.expect("case")?;
                    let pat = if self.eat("_") {
                        None
                    } else {
                        let (name, _) = self.ident()?;
                        let binder = match self.peek() {
                            Some(Tok::Id(b)) => {
                                let b = b.clone();
                                self.i += 1;
                                Some(b)
                            }
                            _ => None,
                        };
                        Some((name, binder))
                    };
                    self.expect("=>")?;
                    arms.push(Arm {
                        pat,
                        body: self.expr()?,
                        at,
                    });
                    self.eat(",");
                }
                Ok(Expr::Match(Box::new(scrutinee), arms, at))
            }
            Some(Tok::Id(w)) => {
                self.i += 1;
                if self.eat("(") {
                    return Ok(Expr::Call(w, self.args()?, at));
                }
                Ok(Expr::Name(w, at))
            }
            _ => Err(error(at, "expected an expression")),
        }
    }
}

fn parser(line: &Line) -> P<'_> {
    let end = line.toks.last().map_or(Span::default(), |(_, s)| Span {
        line: s.line,
        col: s.col + 1,
    });
    P {
        toks: &line.toks,
        i: 0,
        end,
    }
}

/// Parse a look.
pub fn parse(src: &str) -> Result<File, Error> {
    let lines = lines(src)?;
    let mut file = File::default();
    let mut i = 0;
    while i < lines.len() {
        let line = &lines[i];
        if line.indent != 0 {
            return Err(error(line.toks[0].1, "indented line outside a block"));
        }
        let mut p = parser(line);
        if p.eat("const") {
            let (name, at) = p.ident()?;
            p.expect("=")?;
            let e = p.expr()?;
            p.done()?;
            file.consts.push((name, e, at));
            i += 1;
        } else if p.eat("fn") {
            let (name, at) = p.ident()?;
            p.expect("(")?;
            let mut params = Vec::new();
            if !p.eat(")") {
                loop {
                    let (param, _) = p.ident()?;
                    p.expect(":")?;
                    let (ty, _) = p.ident()?;
                    params.push((param, ty));
                    if p.eat(")") {
                        break;
                    }
                    p.expect(",")?;
                }
            }
            p.expect("=")?;
            i += 1;
            let (lets, body) = if p.i < p.toks.len() {
                let e = p.expr()?;
                p.done()?;
                (Vec::new(), e)
            } else {
                fn_block(&lines, &mut i, at)?
            };
            file.fn_after.push(file.body.len());
            file.fns.push(FnDecl {
                name,
                params,
                lets,
                body,
                at,
            });
        } else {
            file.body.push(statement(&lines, &mut i)?);
        }
    }
    Ok(file)
}

fn fn_block(lines: &[Line], i: &mut usize, at: Span) -> Result<(Lets, Expr), Error> {
    let Some(indent) = lines.get(*i).map(|l| l.indent).filter(|n| *n > 0) else {
        return Err(error(
            at,
            "`fn … =` needs a body: an expression, or indented lines under it",
        ));
    };
    let mut lets = Vec::new();
    while let Some(line) = lines.get(*i).filter(|l| l.indent == indent) {
        let mut p = parser(line);
        *i += 1;
        if p.eat("let") {
            let (name, at) = p.ident()?;
            p.expect("=")?;
            let e = p.expr()?;
            p.done()?;
            lets.push((name, e, at));
        } else {
            let e = p.expr()?;
            p.done()?;
            if lines.get(*i).is_some_and(|l| l.indent > 0) {
                return Err(error(e.span(), "a fn's result is its last line"));
            }
            return Ok((lets, e));
        }
    }
    Err(error(at, "a fn's body ends with its result expression"))
}

fn block(lines: &[Line], i: &mut usize, head: Span, outer: usize) -> Result<Vec<Stmt>, Error> {
    let Some(indent) = lines.get(*i).map(|l| l.indent).filter(|n| *n > outer) else {
        return Err(error(head, "expected an indented block"));
    };
    let mut out = Vec::new();
    while lines.get(*i).is_some_and(|l| l.indent == indent) {
        out.push(statement(lines, i)?);
    }
    if let Some(l) = lines
        .get(*i)
        .filter(|l| l.indent > outer && l.indent != indent)
    {
        return Err(error(l.toks[0].1, "inconsistent indentation"));
    }
    Ok(out)
}

fn statement(lines: &[Line], i: &mut usize) -> Result<Stmt, Error> {
    let line = &lines[*i];
    let indent = line.indent;
    let mut p = parser(line);
    let at = p.at();
    *i += 1;
    if p.eat("let") {
        let (name, _) = p.ident()?;
        p.expect("=")?;
        let e = p.expr()?;
        p.done()?;
        return Ok(Stmt::Let(name, e, at));
    }
    if p.eat("guard") {
        let e = p.expr()?;
        p.done()?;
        return Ok(Stmt::Guard(e, at));
    }
    if p.eat("if") {
        let c = p.expr()?;
        p.done()?;
        let then = block(lines, i, at, indent)?;
        let mut otherwise = Vec::new();
        if let Some(next) = lines.get(*i).filter(|l| l.indent == indent) {
            let mut q = parser(next);
            if q.eat("else") {
                q.done()?;
                *i += 1;
                otherwise = block(lines, i, at, indent)?;
            }
        }
        return Ok(Stmt::If(c, then, otherwise, at));
    }
    if p.eat("each") {
        let (entity, _) = p.ident()?;
        let value = if p.eat(",") { Some(p.ident()?.0) } else { None };
        p.expect("in")?;
        let (component, _) = p.ident()?;
        p.done()?;
        let body = block(lines, i, at, indent)?;
        return Ok(Stmt::Each {
            entity,
            value,
            component,
            body,
            at,
        });
    }
    if p.eat("for") {
        let (first, _) = p.ident()?;
        let (index, item) = if p.eat(",") {
            (Some(first), p.ident()?.0)
        } else {
            (None, first)
        };
        p.expect("in")?;
        let list = p.expr()?;
        p.done()?;
        let body = block(lines, i, at, indent)?;
        return Ok(Stmt::For {
            index,
            item,
            list,
            body,
            at,
        });
    }
    if let Some(Tok::Id(row)) = p.peek().cloned() {
        if row.starts_with(char::is_uppercase) {
            p.i += 1;
            p.expect("(")?;
            let entity = p.expr()?;
            let mut args = Vec::new();
            while p.eat(",") {
                let named = match (p.peek(), p.toks.get(p.i + 1).map(|t| &t.0)) {
                    (Some(Tok::Id(n)), Some(Tok::Sym("="))) => Some(n.clone()),
                    _ => None,
                };
                if named.is_some() {
                    p.i += 2;
                }
                args.push((named, p.expr()?));
            }
            p.expect(")")?;
            p.done()?;
            return Ok(Stmt::Emit {
                row,
                entity,
                args,
                at,
            });
        }
    }
    Err(error(
        at,
        "expected `let`, `guard`, `if`, `each`, `for` or a row such as `Offset(e, rotation=…)`",
    ))
}
