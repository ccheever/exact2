//! Source spans survive parameter substitution and whole-component overrides.
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(crate) struct Location {
    pub file: String,
    pub pointer: String,
    pub line: usize,
    pub column: usize,
}
impl Location {
    pub fn error(&self, message: impl std::fmt::Display) -> String {
        format!(
            "{}:{}:{} {}: {message}",
            self.file, self.line, self.column, self.pointer
        )
    }
    pub fn json(&self) -> Value {
        json!({"file":self.file,"field":self.pointer,"line":self.line,"column":self.column})
    }
}
#[derive(Clone)]
pub(crate) struct Node {
    pub cost: usize,
    pub value: Value,
    pub at: Location,
    pub children: BTreeMap<String, Node>,
}
impl Node {
    pub fn field(&self, key: &str) -> Result<&Self, String> {
        self.children
            .get(key)
            .ok_or_else(|| self.at.error(format!("missing field {key}")))
    }
    pub fn text(&self) -> Result<&str, String> {
        self.value
            .as_str()
            .ok_or_else(|| self.at.error("expected string"))
    }
    pub fn object(&self) -> Result<&BTreeMap<String, Node>, String> {
        if self.value.is_object() {
            Ok(&self.children)
        } else {
            Err(self.at.error("expected object"))
        }
    }
    pub fn array(&self) -> Result<Vec<&Node>, String> {
        let array = self
            .value
            .as_array()
            .ok_or_else(|| self.at.error("expected array"))?;
        Ok((0..array.len())
            .map(|i| &self.children[&i.to_string()])
            .collect())
    }
    pub fn keys(&self, keys: &[&str]) -> Result<(), String> {
        let unknown: Vec<_> = self
            .object()?
            .iter()
            .filter(|(k, _)| !keys.contains(&k.as_str()))
            .map(|(k, n)| n.at.error(format!("unknown field {k}")))
            .collect();
        if unknown.is_empty() {
            Ok(())
        } else {
            Err(unknown.join("\n"))
        }
    }
    pub fn error_at(&self, path: &str, message: &str) -> String {
        let mut node = self;
        for part in path.split('.').filter(|p| !p.is_empty()) {
            let Some(next) = node.children.get(part) else {
                break;
            };
            node = next;
        }
        node.at.error(message)
    }
}
pub(crate) fn parse(file: &str, text: &str) -> Result<Node, String> {
    if text.len() > crate::MAX_SCENE_BYTES {
        return Err(format!("{file}: scene exceeds load limit"));
    }
    // serde_json owns JSON lexical validity; the span walk also refuses duplicate keys.
    serde_json::from_str::<Value>(text)
        .map_err(|e| format!("{file}:{}:{}: {e}", e.line(), e.column()))?;
    Parser {
        file,
        text,
        pos: 0,
        line: 1,
        column: 1,
    }
    .node(String::new(), 0)
}
struct Parser<'a> {
    file: &'a str,
    text: &'a str,
    pos: usize,
    line: usize,
    column: usize,
}
impl Parser<'_> {
    fn bump(&mut self) {
        let byte = self.text.as_bytes()[self.pos];
        self.pos += 1;
        if byte == b'\n' {
            self.line += 1;
            self.column = 1;
        } else if byte & 0xc0 != 0x80 {
            self.column += 1;
        }
    }
    fn ws(&mut self) {
        while self
            .text
            .as_bytes()
            .get(self.pos)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.bump();
        }
    }
    fn string(&mut self) -> String {
        let start = self.pos;
        self.bump();
        while self.text.as_bytes()[self.pos] != b'"' {
            if self.text.as_bytes()[self.pos] == b'\\' {
                self.bump();
            }
            self.bump();
        }
        self.bump();
        serde_json::from_str(&self.text[start..self.pos]).unwrap()
    }
    fn node(&mut self, pointer: String, depth: usize) -> Result<Node, String> {
        self.ws();
        let start = self.pos;
        let at = Location {
            file: self.file.into(),
            pointer,
            line: self.line,
            column: self.column,
        };
        if depth > 64 {
            return Err(at.error("scene nesting exceeds 64"));
        }
        let mut children = BTreeMap::new();
        match self.text.as_bytes()[self.pos] {
            b'{' | b'[' => {
                let object = self.text.as_bytes()[self.pos] == b'{';
                let end = if object { b'}' } else { b']' };
                self.bump();
                self.ws();
                let mut index = 0;
                while self.text.as_bytes()[self.pos] != end {
                    let key = if object {
                        let key = self.string();
                        self.ws();
                        self.bump();
                        key
                    } else {
                        index.to_string()
                    };
                    let pointer = format!(
                        "{}/{}",
                        at.pointer,
                        key.replace('~', "~0").replace('/', "~1")
                    );
                    let node = self.node(pointer, depth + 1)?;
                    if children.contains_key(&key) {
                        return Err(node.at.error(format!("duplicate field {key}")));
                    }
                    children.insert(key, node);
                    index += 1;
                    self.ws();
                    if self.text.as_bytes()[self.pos] == b',' {
                        self.bump();
                        self.ws();
                    }
                }
                self.bump();
            }
            b'"' => {
                self.string();
            }
            _ => {
                while self
                    .text
                    .as_bytes()
                    .get(self.pos)
                    .is_some_and(|b| !b.is_ascii_whitespace() && !b",]}".contains(b))
                {
                    self.bump();
                }
            }
        }
        Ok(Node {
            cost: self.pos - start + children.values().map(|n: &Node| n.cost).sum::<usize>(),
            value: serde_json::from_str(&self.text[start..self.pos]).unwrap(),
            at,
            children,
        })
    }
}
