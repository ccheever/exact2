//! A surface's current public record, decoded against each reader's shape.
use exact_plan::{Opcode, Plan, ResourcesRow, StrId, TypeKind, TypesId, Value};
use std::collections::BTreeMap;

/// Reserved runner-owned resource source.
pub const SOURCE: &str = "exactSurface";
/// Maximum UTF-8 record size, including ignored fields.
pub const MAX_BYTES: usize = 64 * 1024;
const MAX_DEPTH: usize = 32;

/// Resource `i`'s value from its surface's published record: what the runner
/// answers for [`SOURCE`] when its host links surfaces (LLP 1047 D3).
pub fn answer(
    plan: &Plan,
    records: &exact_kernel::SortedMap<String, String>,
    i: usize,
) -> Result<Value, crate::runner::DataError> {
    let row = &plan.resources[i];
    let name = surface_name(plan, row).ok_or_else(|| {
        crate::runner::DataError::BadArguments(format!(
            "{SOURCE} takes exactly one string-literal surface name"
        ))
    })?;
    decode(plan, row.ty, records.get(name).map(String::as_str))
        .map_err(crate::runner::DataError::Unavailable)
}

/// The sole argument must be a string literal, not a computed string.
pub fn surface_name<'a>(plan: &'a Plan, row: &ResourcesRow) -> Option<&'a str> {
    if row.args.len != 1 {
        return None;
    }
    let code = plan.code(plan.arg(row.args.iter().next()?).expr);
    if code.len() != 6 || code[0] != Opcode::Str as u8 || code[5] != Opcode::Return as u8 {
        return None;
    }
    Some(plan.str(StrId(u32::from_le_bytes(code[1..5].try_into().ok()?))))
}

#[derive(Debug)]
enum Json {
    Null,
    Scalar(Value),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}

/// Decode one JSON object, filling absent fields and ignoring extra keys.
/// Errors name the field path; the runner adds the resource's name.
pub fn decode(plan: &Plan, ty: TypesId, text: Option<&str>) -> Result<Value, String> {
    if plan.type_(ty).kind != TypeKind::Record {
        return Err("expected a record shape".into());
    }
    let json = match text {
        None => None,
        Some(text) => {
            if text.len() > MAX_BYTES {
                return Err("record exceeds 64 KiB".into());
            }
            let mut reader = Reader { text, at: 0 };
            let value = reader.value(0)?;
            reader.space();
            if reader.at != text.len() {
                return Err(reader.error("trailing input"));
            }
            if !matches!(value, Json::Object(_)) {
                return Err("record: expected JSON object".into());
            }
            Some(value)
        }
    };
    shaped(plan, ty, json.as_ref(), "record", 0)
}

fn shaped(
    plan: &Plan,
    ty: TypesId,
    json: Option<&Json>,
    path: &str,
    depth: usize,
) -> Result<Value, String> {
    if depth > MAX_DEPTH {
        return Err(format!("{path}: shape exceeds depth 32"));
    }
    let row = plan.type_(ty);
    let fail = || format!("{path}: expected {:?}", row.kind);
    Ok(match (row.kind, json) {
        (TypeKind::Number, None) => Value::Number(0.0),
        (TypeKind::Bool, None) => Value::Bool(false),
        (TypeKind::String, None) => Value::str(""),
        (TypeKind::Unit, None | Some(Json::Null)) => Value::Unit,
        (TypeKind::Number, Some(Json::Scalar(v @ Value::Number(_))))
        | (TypeKind::Bool, Some(Json::Scalar(v @ Value::Bool(_))))
        | (TypeKind::String, Some(Json::Scalar(v @ exact_plan::str_value!()))) => v.clone(),
        (TypeKind::Option, None | Some(Json::Null)) => Value::NONE,
        (TypeKind::Option, Some(v)) => Value::some(shaped(
            plan,
            row.elem.ok_or_else(fail)?,
            Some(v),
            path,
            depth + 1,
        )?),
        (TypeKind::List, None) => Value::list(vec![]),
        (TypeKind::List, Some(Json::Array(items))) => Value::list(
            items
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    shaped(
                        plan,
                        row.elem.ok_or_else(fail)?,
                        Some(v),
                        &format!("{path}[{i}]"),
                        depth + 1,
                    )
                })
                .collect::<Result<_, _>>()?,
        ),
        (TypeKind::Record, None | Some(Json::Object(_))) => {
            let fields = row
                .fields
                .iter()
                .map(|f| {
                    let field = plan.field(f);
                    let name = plan.str(field.name);
                    let value = match json {
                        Some(Json::Object(fields)) => fields.get(name),
                        _ => None,
                    };
                    shaped(plan, field.ty, value, &format!("{path}.{name}"), depth + 1)
                })
                .collect::<Result<_, _>>()?;
            Value::record(fields)
        }
        _ => return Err(fail()),
    })
}

struct Reader<'a> {
    text: &'a str,
    at: usize,
}
impl Reader<'_> {
    fn error(&self, why: &str) -> String {
        format!("JSON byte {}: {why}", self.at)
    }
    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }
    fn eat(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.at += 1;
        }
    }
    fn expect(&mut self, byte: u8) -> Result<(), String> {
        self.space();
        if self.eat(byte) {
            Ok(())
        } else {
            Err(self.error(&format!("expected {}", byte as char)))
        }
    }
    fn word(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if !self.text[self.at..].starts_with(word) {
            return Err(self.error("invalid literal"));
        }
        self.at += word.len();
        Ok(value)
    }
    fn value(&mut self, depth: usize) -> Result<Json, String> {
        self.space();
        match self.peek() {
            Some(b'{' | b'[') if depth >= MAX_DEPTH => Err(self.error("depth exceeds 32")),
            Some(b'{') => {
                self.at += 1;
                let mut fields = BTreeMap::new();
                self.space();
                if self.eat(b'}') {
                    return Ok(Json::Object(fields));
                }
                loop {
                    let key = self.string()?;
                    self.expect(b':')?;
                    if fields.contains_key(&key) {
                        return Err(self.error(&format!("duplicate key `{key}`")));
                    }
                    fields.insert(key, self.value(depth + 1)?);
                    self.space();
                    if self.eat(b'}') {
                        return Ok(Json::Object(fields));
                    }
                    self.expect(b',')?;
                    self.space();
                }
            }
            Some(b'[') => {
                self.at += 1;
                let mut items = Vec::new();
                self.space();
                if self.eat(b']') {
                    return Ok(Json::Array(items));
                }
                loop {
                    items.push(self.value(depth + 1)?);
                    self.space();
                    if self.eat(b']') {
                        return Ok(Json::Array(items));
                    }
                    self.expect(b',')?;
                }
            }
            Some(b'"') => Ok(Json::Scalar(Value::str(&self.string()?))),
            Some(b't') => self.word("true", Json::Scalar(Value::Bool(true))),
            Some(b'f') => self.word("false", Json::Scalar(Value::Bool(false))),
            Some(b'n') => self.word("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(self.error("expected a value")),
        }
    }
    fn digits(&mut self) -> Result<(), String> {
        let start = self.at;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.at += 1;
        }
        if self.at == start {
            Err(self.error("expected digit"))
        } else {
            Ok(())
        }
    }
    fn number(&mut self) -> Result<Json, String> {
        let start = self.at;
        self.eat(b'-');
        if !self.eat(b'0') {
            self.digits()?;
        }
        if self.eat(b'.') {
            self.digits()?;
        }
        if self.eat(b'e') || self.eat(b'E') {
            if !self.eat(b'+') {
                self.eat(b'-');
            }
            self.digits()?;
        }
        let n = exact_num::parse_f64(&self.text[start..self.at])
            .map_err(|_| self.error("invalid number"))?;
        if !n.is_finite() {
            return Err(self.error("number is not finite"));
        }
        Ok(Json::Scalar(Value::Number(n)))
    }
    fn hex(&mut self) -> Result<u32, String> {
        let mut n = 0;
        for _ in 0..4 {
            let digit = self
                .peek()
                .and_then(|b| (b as char).to_digit(16))
                .ok_or_else(|| self.error("invalid Unicode escape"))?;
            self.at += 1;
            n = n * 16 + digit;
        }
        Ok(n)
    }
    fn string(&mut self) -> Result<String, String> {
        if !self.eat(b'"') {
            return Err(self.error("expected string"));
        }
        let mut out = String::new();
        loop {
            let c = self.text[self.at..]
                .chars()
                .next()
                .ok_or_else(|| self.error("unterminated string"))?;
            self.at += c.len_utf8();
            match c {
                '"' => return Ok(out),
                '\\' => {
                    let escape = self
                        .peek()
                        .ok_or_else(|| self.error("unterminated escape"))?;
                    self.at += 1;
                    out.push(match escape {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let mut n = self.hex()?;
                            if (0xd800..=0xdbff).contains(&n) {
                                if !self.eat(b'\\') || !self.eat(b'u') {
                                    return Err(self.error("missing low surrogate"));
                                }
                                let low = self.hex()?;
                                if !(0xdc00..=0xdfff).contains(&low) {
                                    return Err(self.error("invalid low surrogate"));
                                }
                                n = 0x10000 + ((n - 0xd800) << 10) + low - 0xdc00;
                            }
                            char::from_u32(n).ok_or_else(|| self.error("invalid Unicode scalar"))?
                        }
                        _ => return Err(self.error("invalid escape")),
                    });
                }
                '\u{0}'..='\u{1f}' => return Err(self.error("unescaped control character")),
                _ => out.push(c),
            }
        }
    }
}
