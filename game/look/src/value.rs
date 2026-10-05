//! Values a look computes, and the two `Data` sinks that make them: `Build`
//! turns a row into a positional value at present time, `Tree` turns a type's
//! `Default` (or a probe) into a named shape for the checker.
use exact_game::data::{Bulk, Number};
use exact_game::{Entity, Quat, Vec3, Writer};
use std::rc::Rc;

/// A value. Records are positional: the checker resolved every field name to
/// its index in the type's declaration order, which `Data` writes in.
#[derive(Clone, Debug, Default)]
pub enum Val {
    #[default]
    None,
    Num(f32),
    Bool(bool),
    Str(Rc<str>),
    Ent(Entity),
    V3(Vec3),
    Q(Quat),
    /// An enum arm, by index.
    Arm(u32),
    List(Rc<Vec<Val>>),
    Col(Rc<Column>),
    Rec(Rc<Vec<Val>>),
}

/// A numeric column (`Vec<f32>`, `Vec<u8>`, …), as `Data` writes it in bulk.
#[derive(Debug)]
pub enum Column {
    U8(Vec<u8>),
    U16(Vec<u16>),
    U32(Vec<u32>),
    F32(Vec<f32>),
}
impl Column {
    pub fn len(&self) -> usize {
        match self {
            Self::U8(v) => v.len(),
            Self::U16(v) => v.len(),
            Self::U32(v) => v.len(),
            Self::F32(v) => v.len(),
        }
    }
    pub fn get(&self, i: usize) -> Option<f32> {
        Some(match self {
            Self::U8(v) => *v.get(i)? as f32,
            Self::U16(v) => *v.get(i)? as f32,
            Self::U32(v) => *v.get(i)? as f32,
            Self::F32(v) => *v.get(i)?,
        })
    }
}

impl Val {
    /// Bit-for-bit equality, as the engine compares rows.
    pub fn same(&self, other: &Val) -> bool {
        match (self, other) {
            (Val::None, Val::None) => true,
            (Val::Num(a), Val::Num(b)) => a.to_bits() == b.to_bits(),
            (Val::Bool(a), Val::Bool(b)) => a == b,
            (Val::Str(a), Val::Str(b)) => a == b,
            (Val::Ent(a), Val::Ent(b)) => a == b,
            (Val::V3(a), Val::V3(b)) => {
                a.to_array().map(f32::to_bits) == b.to_array().map(f32::to_bits)
            }
            (Val::Q(a), Val::Q(b)) => {
                a.to_array().map(f32::to_bits) == b.to_array().map(f32::to_bits)
            }
            (Val::Arm(a), Val::Arm(b)) => a == b,
            (Val::List(a), Val::List(b)) | (Val::Rec(a), Val::Rec(b)) => {
                Rc::ptr_eq(a, b)
                    || (a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.same(y)))
            }
            (Val::Col(a), Val::Col(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

fn number(n: Number) -> f32 {
    match n {
        Number::Unsigned(n) => n as f32,
        Number::Signed(n) => n as f32,
        Number::F32(n) => n,
        Number::F64(n) => n as f32,
    }
}

enum Frame {
    Seq(Vec<Val>),
    // Fields so far, and whether they are an Entity's `index` then `generation`.
    Rec(Vec<Val>, bool),
    Arm(u32),
    Opt(Option<Val>),
}

/// Build a positional value from a row's `Data::write`.
#[derive(Default)]
pub struct Build {
    stack: Vec<Frame>,
    out: Option<Val>,
    raw: (u32, u32),
}
impl Build {
    pub fn finish(mut self) -> Val {
        self.out.take().unwrap_or_default()
    }
    fn push(&mut self, v: Val) {
        match self.stack.last_mut() {
            None => self.out = Some(v),
            Some(Frame::Seq(items)) | Some(Frame::Rec(items, _)) => items.push(v),
            Some(Frame::Arm(_)) => {}
            Some(Frame::Opt(slot)) => *slot = Some(v),
        }
    }
}
impl Writer for Build {
    fn boolean(&mut self, value: bool) {
        self.push(Val::Bool(value));
    }
    fn number(&mut self, value: Number) {
        if let (Some(Frame::Rec(fields, true)), Number::Unsigned(n)) = (self.stack.last(), value) {
            match fields.len() {
                0 => self.raw.0 = n as u32,
                _ => self.raw.1 = n as u32,
            }
        }
        self.push(Val::Num(number(value)));
    }
    fn bytes(&mut self, value: Bulk<'_>) {
        self.push(Val::Col(Rc::new(match value {
            Bulk::U8(v) => Column::U8(v.to_vec()),
            Bulk::U16(v) => Column::U16(v.to_vec()),
            Bulk::U32(v) => Column::U32(v.to_vec()),
            Bulk::F32(v) => Column::F32(v.to_vec()),
        })));
    }
    fn string(&mut self, value: &str) {
        self.push(Val::Str(value.into()));
    }
    fn begin_seq(&mut self, len: usize) {
        self.stack.push(Frame::Seq(Vec::with_capacity(len)));
    }
    fn item(&mut self) {}
    fn end_seq(&mut self) {
        let Some(Frame::Seq(items)) = self.stack.pop() else {
            return;
        };
        let nums: Option<Vec<f32>> = items
            .iter()
            .map(|v| if let Val::Num(n) = v { Some(*n) } else { None })
            .collect();
        let v = match (items.len(), nums) {
            (3, Some(n)) => Val::V3(Vec3::new(n[0], n[1], n[2])),
            (4, Some(n)) => Val::Q(Quat::from_xyzw(n[0], n[1], n[2], n[3])),
            _ => Val::List(Rc::new(items)),
        };
        self.push(v);
    }
    fn begin_struct(&mut self) {
        self.stack.push(Frame::Rec(Vec::new(), true));
    }
    fn field(&mut self, name: &str) {
        if let Some(Frame::Rec(fields, entity)) = self.stack.last_mut() {
            let expected = ["index", "generation"].get(fields.len()).copied();
            *entity &= expected == Some(name);
        }
    }
    fn end_struct(&mut self) {
        let Some(Frame::Rec(fields, entity)) = self.stack.pop() else {
            return;
        };
        let v = if entity && fields.len() == 2 {
            entity_from(self.raw)
        } else {
            Val::Rec(Rc::new(fields))
        };
        self.push(v);
    }
    fn variant(&mut self, _name: &str, index: u32) {
        self.stack.push(Frame::Arm(index));
    }
    fn end_variant(&mut self) {
        if let Some(Frame::Arm(index)) = self.stack.pop() {
            self.push(Val::Arm(index));
        }
    }
    fn option(&mut self, _some: bool) {
        self.stack.push(Frame::Opt(None));
    }
    fn end_option(&mut self) {
        if let Some(Frame::Opt(v)) = self.stack.pop() {
            self.push(v.unwrap_or_default());
        }
    }
}

/// An `Entity` from the two numbers `Data` writes for it. `Entity` keeps its
/// fields private; reading its own encoding back is the one public door.
fn entity_from(raw: (u32, u32)) -> Val {
    let mut e = Entity::default();
    let mut r = EntityReader { raw, step: 0 };
    match exact_game::Data::read(&mut e, &mut r) {
        Ok(()) => Val::Ent(e),
        Err(_) => Val::None,
    }
}

/// Reads `{index, generation}` and nothing else.
struct EntityReader {
    raw: (u32, u32),
    step: u8,
}
impl exact_game::Reader for EntityReader {
    fn boolean(&mut self) -> Result<bool, exact_game::DataError> {
        Err(exact_game::DataError::new("entity"))
    }
    fn number(&mut self) -> Result<Number, exact_game::DataError> {
        Ok(Number::Unsigned(u64::from(match self.step {
            1 => self.raw.0,
            _ => self.raw.1,
        })))
    }
    fn bytes(
        &mut self,
        _: exact_game::data::BulkKind,
    ) -> Result<Option<&[u8]>, exact_game::DataError> {
        Ok(None)
    }
    fn string(&mut self) -> Result<String, exact_game::DataError> {
        Err(exact_game::DataError::new("entity"))
    }
    fn begin_seq(&mut self) -> Result<(), exact_game::DataError> {
        Err(exact_game::DataError::new("entity"))
    }
    fn item(&mut self) -> Result<bool, exact_game::DataError> {
        Ok(false)
    }
    fn begin_struct(&mut self) -> Result<(), exact_game::DataError> {
        Ok(())
    }
    fn field(&mut self) -> Result<Option<String>, exact_game::DataError> {
        self.step += 1;
        Ok(match self.step {
            1 => Some("index".into()),
            2 => Some("generation".into()),
            _ => None,
        })
    }
    fn variant(&mut self) -> Result<String, exact_game::DataError> {
        Err(exact_game::DataError::new("entity"))
    }
    fn end_variant(&mut self) -> Result<(), exact_game::DataError> {
        Ok(())
    }
    fn option(&mut self) -> Result<bool, exact_game::DataError> {
        Ok(false)
    }
    fn end_option(&mut self) -> Result<(), exact_game::DataError> {
        Ok(())
    }
    fn skip(&mut self) -> Result<(), exact_game::DataError> {
        Ok(())
    }
}

/// A type's shape: named fields, as the checker needs them.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Num,
    Bool,
    Str,
    Ent,
    V3,
    Quat,
    Col,
    /// A list; its element shape when the default had an element.
    List(Option<Box<Shape>>),
    Rec(Vec<(String, Shape)>),
    /// An enum: its default arm's name and index.
    Arm(String, u32),
    /// An option (its shape unknown when the default is none).
    Opt(Option<Box<Shape>>),
}

enum TreeFrame {
    Seq(Vec<Shape>),
    Rec(Vec<(String, Shape)>, Option<String>),
    Arm(String, u32),
    Opt(Option<Shape>),
}

/// Build a named shape from a value's `Data::write`.
#[derive(Default)]
pub struct Tree {
    stack: Vec<TreeFrame>,
    out: Option<Shape>,
}
impl Tree {
    pub fn finish(mut self) -> Option<Shape> {
        self.out.take()
    }
    fn push(&mut self, s: Shape) {
        match self.stack.last_mut() {
            None => self.out = Some(s),
            Some(TreeFrame::Seq(items)) => items.push(s),
            Some(TreeFrame::Rec(fields, name)) => {
                fields.push((name.take().unwrap_or_default(), s));
            }
            Some(TreeFrame::Arm(..)) => {}
            Some(TreeFrame::Opt(slot)) => *slot = Some(s),
        }
    }
}
impl Writer for Tree {
    fn boolean(&mut self, _: bool) {
        self.push(Shape::Bool);
    }
    fn number(&mut self, _: Number) {
        self.push(Shape::Num);
    }
    fn bytes(&mut self, _: Bulk<'_>) {
        self.push(Shape::Col);
    }
    fn string(&mut self, _: &str) {
        self.push(Shape::Str);
    }
    fn begin_seq(&mut self, _: usize) {
        self.stack.push(TreeFrame::Seq(Vec::new()));
    }
    fn item(&mut self) {}
    fn end_seq(&mut self) {
        let Some(TreeFrame::Seq(items)) = self.stack.pop() else {
            return;
        };
        let all_num = items.iter().all(|s| *s == Shape::Num);
        let s = match items.len() {
            3 if all_num => Shape::V3,
            4 if all_num => Shape::Quat,
            _ => Shape::List(items.into_iter().next().map(Box::new)),
        };
        self.push(s);
    }
    fn begin_struct(&mut self) {
        self.stack.push(TreeFrame::Rec(Vec::new(), None));
    }
    fn field(&mut self, name: &str) {
        if let Some(TreeFrame::Rec(_, slot)) = self.stack.last_mut() {
            *slot = Some(name.to_string());
        }
    }
    fn end_struct(&mut self) {
        let Some(TreeFrame::Rec(fields, _)) = self.stack.pop() else {
            return;
        };
        let entity = fields.len() == 2 && fields[0].0 == "index" && fields[1].0 == "generation";
        self.push(if entity {
            Shape::Ent
        } else {
            Shape::Rec(fields)
        });
    }
    fn variant(&mut self, name: &str, index: u32) {
        self.stack.push(TreeFrame::Arm(name.to_string(), index));
    }
    fn end_variant(&mut self) {
        if let Some(TreeFrame::Arm(name, index)) = self.stack.pop() {
            self.push(Shape::Arm(name, index));
        }
    }
    fn option(&mut self, _: bool) {
        self.stack.push(TreeFrame::Opt(None));
    }
    fn end_option(&mut self) {
        if let Some(TreeFrame::Opt(s)) = self.stack.pop() {
            self.push(Shape::Opt(s.map(Box::new)));
        }
    }
}
