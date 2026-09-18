//! Reload-only Data projection. Ordinary wire/hash codecs never see these tags.
use crate::data::{f32_bits, f64_bits, BulkKind};
use crate::{Data, DataError, Number, World, Writer};
use std::collections::BTreeMap;

#[derive(Clone, Default, PartialEq, Eq, Data)]
pub(super) enum Node {
    #[default]
    Empty,
    Bool(bool),
    Unsigned(u64),
    Signed(i64),
    Float(u32),
    Double(u64),
    Text(String),
    Bytes(u32, Vec<u8>),
    Record(BTreeMap<String, Node>),
    Seq(Vec<Node>),
    Variant(String, u32, Box<Node>),
    Option(bool, Box<Node>),
    Reference(String),
}
impl Node {
    pub fn emit(&self, w: &mut dyn Writer, world: &World) -> Result<(), DataError> {
        match self {
            Self::Empty => return Err(DataError::new("invalid empty reload value")),
            Self::Bool(v) => w.boolean(*v),
            Self::Unsigned(v) => w.number(Number::Unsigned(*v)),
            Self::Signed(v) => w.number(Number::Signed(*v)),
            Self::Float(v) => w.number(Number::F32(f32::from_bits(*v))),
            Self::Double(v) => w.number(Number::F64(f64::from_bits(*v))),
            Self::Text(v) => w.string(v),
            Self::Bytes(k, v) => w.bytes(
                match k {
                    0 => BulkKind::U8,
                    1 => BulkKind::U16,
                    2 => BulkKind::U32,
                    3 => BulkKind::F32,
                    _ => return Err(DataError::new("invalid reload bulk kind")),
                },
                v,
            ),
            Self::Record(fields) => {
                w.begin_struct();
                for (name, value) in fields {
                    w.field(name);
                    value.emit(w, world)?;
                }
                w.end_struct();
            }
            Self::Seq(values) => {
                w.begin_seq(values.len());
                for value in values {
                    w.item();
                    value.emit(w, world)?;
                }
                w.end_seq();
            }
            Self::Variant(name, index, value) => {
                w.variant(name, *index);
                value.emit(w, world)?;
                w.end_variant();
            }
            Self::Option(some, value) => {
                w.option(*some);
                if *some {
                    value.emit(w, world)?;
                }
                w.end_option();
            }
            Self::Reference(key) => {
                let entity = if key == "null" {
                    crate::Entity::default()
                } else {
                    super::reload::resolve(world, key)
                        .ok_or_else(|| DataError::new("entity reference needs restart"))?
                };
                w.entity(entity.index(), entity.generation());
            }
        }
        Ok(())
    }
    pub fn display(&self, world: &World) -> String {
        if let Self::Reference(name) = self {
            return crate::values::quote(name.strip_prefix("n:").unwrap_or(name));
        }
        // Avoid expanding large container values just to report an initializer.
        if self.weight() > 256 {
            return "\"<value exceeds 256-byte report preview>\"".into();
        }
        let mut w = crate::json::Encoder::default();
        if self.emit(&mut w, world).is_err() {
            return "\"<unresolved reference>\"".into();
        }
        w.finish().unwrap_or_else(|_| "\"<non-JSON value>\"".into())
    }
    fn weight(&self) -> usize {
        match self {
            Self::Text(s) | Self::Reference(s) => s.len(),
            Self::Bytes(_, b) => b.len(),
            Self::Record(v) => v
                .iter()
                .map(|(k, v)| k.len().saturating_add(v.weight()))
                .fold(0usize, usize::saturating_add),
            Self::Seq(v) => v
                .iter()
                .map(Self::weight)
                .fold(0usize, usize::saturating_add),
            Self::Variant(_, _, v) | Self::Option(_, v) => v.weight(),
            _ => 16,
        }
    }
}

// One allowance for an entire projection, including empty rows and absent-column probes.
#[derive(Default)]
pub(super) struct Work {
    pub units: usize,
    pub bytes: usize,
    failed: bool,
}
impl Work {
    pub fn charge(&mut self, bytes: usize) -> bool {
        self.units = self.units.saturating_add(1);
        self.bytes = self.bytes.saturating_add(bytes);
        self.failed |= self.units > 16_000_000 || self.bytes > 512 * 1024 * 1024;
        !self.failed
    }
    pub fn check(&self) -> Result<(), DataError> {
        if self.failed {
            Err(DataError::new(
                "reload projection exceeds work limit (16M visits / 512 MiB)",
            ))
        } else {
            Ok(())
        }
    }
}
pub(super) struct Collect<'a> {
    world: &'a World,
    work: &'a mut Work,
    stack: Vec<(String, Node)>,
    value: Node,
}
impl<'a> Collect<'a> {
    pub fn new(world: &'a World, work: &'a mut Work) -> Self {
        Self {
            world,
            work,
            stack: vec![],
            value: Node::Empty,
        }
    }
    pub fn finish(self) -> Result<Node, DataError> {
        self.work.check()?;
        Ok(self.value)
    }
    fn add(&mut self, value: Node) {
        if self.work.failed {
            return;
        }
        match self.stack.last_mut() {
            Some((key, Node::Record(fields))) => {
                fields.insert(std::mem::take(key), value);
            }
            Some((_, Node::Seq(values))) => values.push(value),
            Some((_, Node::Variant(_, _, slot) | Node::Option(_, slot))) => **slot = value,
            _ => self.value = value,
        }
    }
    fn start(&mut self, value: Node) {
        if self.work.failed {
            return;
        }
        if self.stack.len() >= 64 {
            self.work.failed = true;
            return;
        }
        self.stack.push((String::new(), value));
    }
    fn end(&mut self) {
        if !self.work.failed {
            let (_, value) = self.stack.pop().unwrap();
            self.add(value);
        }
    }
}
impl Writer for Collect<'_> {
    fn entity(&mut self, index: u32, generation: u32) {
        if self.work.charge(64) {
            let entity = crate::world::Entity { index, generation };
            let key = if entity == crate::Entity::default() {
                "null".into()
            } else {
                let key = super::reload::key(self.world, entity);
                if key.starts_with("u:") {
                    format!("r:{index}:{generation}")
                } else {
                    key
                }
            };
            self.add(Node::Reference(key));
        }
    }
    fn boolean(&mut self, value: bool) {
        if self.work.charge(32) {
            self.add(Node::Bool(value));
        }
    }
    fn number(&mut self, value: Number) {
        if self.work.charge(32) {
            self.add(match value {
                Number::Unsigned(v) => Node::Unsigned(v),
                Number::Signed(v) => Node::Signed(v),
                Number::F32(v) => Node::Float(f32_bits(v)),
                Number::F64(v) => Node::Double(f64_bits(v)),
            });
        }
    }
    fn string(&mut self, value: &str) {
        if self.work.charge(32 + value.len()) {
            self.add(Node::Text(value.into()));
        }
    }
    fn bytes(&mut self, kind: BulkKind, value: &[u8]) {
        if self.work.charge(32 + value.len()) {
            self.add(Node::Bytes(kind as u32, value.into()));
        }
    }
    fn begin_seq(&mut self, _: usize) {
        if self.work.charge(32) {
            self.start(Node::Seq(vec![]));
        }
    }
    fn item(&mut self) {}
    fn end_seq(&mut self) {
        self.end();
    }
    fn begin_struct(&mut self) {
        if self.work.charge(32) {
            self.start(Node::Record(BTreeMap::new()));
        }
    }
    fn field(&mut self, name: &str) {
        if self.work.charge(64 + name.len()) {
            self.stack.last_mut().unwrap().0 = name.into();
        }
    }
    fn end_struct(&mut self) {
        self.end();
    }
    fn variant(&mut self, name: &str, _index: u32) {
        if self.work.charge(32 + name.len()) {
            self.start(Node::Variant(name.into(), 0, Box::default()));
        }
    }
    fn end_variant(&mut self) {
        self.end();
    }
    fn option(&mut self, some: bool) {
        if self.work.charge(32) {
            self.start(Node::Option(some, Box::default()));
        }
    }
    fn end_option(&mut self) {
        self.end();
    }
}
