//! A bytecode assembler for the expression VM.
//!
//! @ref LLP 1004 D2 (the opcode table is the one authority)
//!
//! Emits opcodes with their declared operand layouts and resolves forward
//! jumps by label, so a compiler never writes a raw byte. The result of
//! [`Asm::finish`] is a code body ending in `Return`, the shape
//! `Plan::check_code` verifies.

use crate::bytes::Writer;
use crate::generated::{Opcode, Operand, Stdlib};
use crate::{DerivesId, MutationsId, ResourcesId, SlotsId, StrId, TypesId};

/// A forward-jump label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Label(usize);

/// One operand value, matching an [`Operand`] layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Arg {
    /// A `u8`.
    U8(u8),
    /// A `u16`.
    U16(u16),
    /// A `u32`.
    U32(u32),
    /// An `f64`.
    F64(f64),
    /// A string index.
    Str(StrId),
    /// An enum ordinal.
    Enum(u8),
    /// A row index.
    Idx(u32),
}

/// Assembles one code body.
#[derive(Debug, Default)]
pub struct Asm {
    w: Writer,
    labels: Vec<Option<u32>>,
    patches: Vec<(usize, Label)>,
}

impl Asm {
    /// Empty.
    pub fn new() -> Asm {
        Asm::default()
    }

    /// Current byte offset.
    pub fn position(&self) -> u32 {
        self.w.len() as u32
    }

    /// Emit `op` with `args`, which must match its declared operand layout.
    pub fn op(&mut self, op: Opcode, args: &[Arg]) -> &mut Self {
        let layout = op.operands();
        assert_eq!(
            layout.len(),
            args.len(),
            "{op:?} takes {} operands",
            layout.len()
        );
        self.w.u8(op as u8);
        for (operand, arg) in layout.iter().zip(args) {
            match (operand, arg) {
                (Operand::U8, Arg::U8(v)) => self.w.u8(*v),
                (Operand::U16, Arg::U16(v)) => self.w.u16(*v),
                (Operand::U32, Arg::U32(v)) => self.w.u32(*v),
                (Operand::F64, Arg::F64(v)) => self.w.f64(*v),
                (Operand::Str, Arg::Str(v)) => self.w.u32(v.0),
                (Operand::Enum(_), Arg::Enum(v)) => self.w.u8(*v),
                (Operand::Idx(_), Arg::Idx(v)) => self.w.u32(*v),
                (operand, arg) => panic!("{op:?}: operand {operand:?} given {arg:?}"),
            }
        }
        self
    }

    /// Push a number.
    pub fn number(&mut self, n: f64) -> &mut Self {
        self.op(Opcode::Number, &[Arg::F64(n)])
    }

    /// Push a bool.
    pub fn bool(&mut self, b: bool) -> &mut Self {
        self.op(Opcode::Bool, &[Arg::U8(b as u8)])
    }

    /// Push a string.
    pub fn str(&mut self, s: StrId) -> &mut Self {
        self.op(Opcode::Str, &[Arg::Str(s)])
    }

    /// Push a slot's value.
    pub fn load_slot(&mut self, s: SlotsId) -> &mut Self {
        self.op(Opcode::LoadSlot, &[Arg::Idx(s.0)])
    }

    /// Push a derive's value.
    pub fn load_derive(&mut self, d: DerivesId) -> &mut Self {
        self.op(Opcode::LoadDerive, &[Arg::Idx(d.0)])
    }

    /// Push a resource's value.
    pub fn load_resource(&mut self, r: ResourcesId) -> &mut Self {
        self.op(Opcode::LoadResource, &[Arg::Idx(r.0)])
    }

    /// Push an action parameter.
    pub fn load_param(&mut self, i: u16) -> &mut Self {
        self.op(Opcode::LoadParam, &[Arg::U16(i)])
    }

    /// Push the `each` item `depth` scopes out.
    pub fn load_item(&mut self, depth: u16) -> &mut Self {
        self.op(Opcode::LoadItem, &[Arg::U16(depth)])
    }

    /// Push the `match` binding `depth` scopes out.
    pub fn load_bound(&mut self, depth: u16) -> &mut Self {
        self.op(Opcode::LoadBound, &[Arg::U16(depth)])
    }

    /// Replace the record on top with its field `i`.
    pub fn field(&mut self, i: u16) -> &mut Self {
        self.op(Opcode::Field, &[Arg::U16(i)])
    }

    /// Build a record of type `ty` from the top `n` values (n = its field count).
    pub fn record(&mut self, ty: TypesId) -> &mut Self {
        self.op(Opcode::Record, &[Arg::Idx(ty.0)])
    }

    /// Build a list from the top `n` values.
    pub fn list(&mut self, n: u32) -> &mut Self {
        self.op(Opcode::List, &[Arg::U32(n)])
    }

    /// Call a roster function; its arity's worth of arguments are on the stack.
    pub fn call(&mut self, f: Stdlib) -> &mut Self {
        self.op(Opcode::Call, &[Arg::Enum(f as u8)])
    }

    /// Pop into a slot.
    pub fn store_slot(&mut self, s: SlotsId) -> &mut Self {
        self.op(Opcode::StoreSlot, &[Arg::Idx(s.0)])
    }

    /// Emit a command with `n` arguments from the stack.
    pub fn command(&mut self, name: StrId, n: u16) -> &mut Self {
        self.op(Opcode::Command, &[Arg::Str(name), Arg::U16(n)])
    }

    /// Send a mutation to `source` with `n` arguments from the stack (LLP 1016).
    pub fn send(&mut self, m: MutationsId, source: StrId, n: u16) -> &mut Self {
        self.op(
            Opcode::Send,
            &[Arg::Idx(m.0), Arg::Str(source), Arg::U16(n)],
        )
    }

    /// Re-request a resource with its current arguments.
    pub fn refresh(&mut self, r: ResourcesId) -> &mut Self {
        self.op(Opcode::Refresh, &[Arg::Idx(r.0)])
    }

    /// Push whether a resource has a request in flight.
    pub fn pending_resource(&mut self, r: ResourcesId) -> &mut Self {
        self.op(Opcode::PendingResource, &[Arg::Idx(r.0)])
    }

    /// Push whether a mutation has a request in flight.
    pub fn pending_mutation(&mut self, m: MutationsId) -> &mut Self {
        self.op(Opcode::PendingMutation, &[Arg::Idx(m.0)])
    }

    /// Pop into the locals stack.
    pub fn bind_local(&mut self) -> &mut Self {
        self.simple(Opcode::BindLocal)
    }

    /// Push a copy of local `i`.
    pub fn load_local(&mut self, i: u16) -> &mut Self {
        self.op(Opcode::LoadLocal, &[Arg::U16(i)])
    }

    /// Pop the locals stack.
    pub fn drop_local(&mut self) -> &mut Self {
        self.simple(Opcode::DropLocal)
    }

    /// A new unplaced label.
    pub fn label(&mut self) -> Label {
        self.labels.push(None);
        Label(self.labels.len() - 1)
    }

    /// Place `label` here.
    pub fn place(&mut self, label: Label) -> &mut Self {
        self.labels[label.0] = Some(self.position());
        self
    }

    fn jump_like(&mut self, op: Opcode, label: Label) -> &mut Self {
        self.w.u8(op as u8);
        self.patches.push((self.w.len(), label));
        self.w.u32(0);
        self
    }

    /// Unconditional jump.
    pub fn jump(&mut self, label: Label) -> &mut Self {
        self.jump_like(Opcode::Jump, label)
    }

    /// Pop a bool; jump when false.
    pub fn jump_if_false(&mut self, label: Label) -> &mut Self {
        self.jump_like(Opcode::JumpIfFalse, label)
    }

    /// Peek an option; jump when `none` (the value stays for `Unwrap`).
    pub fn jump_if_none(&mut self, label: Label) -> &mut Self {
        self.jump_like(Opcode::JumpIfNone, label)
    }

    /// Emit an operand-free opcode.
    pub fn simple(&mut self, op: Opcode) -> &mut Self {
        self.op(op, &[])
    }

    /// Terminate with `Return` and resolve every jump.
    pub fn finish(mut self) -> Vec<u8> {
        self.w.u8(Opcode::Return as u8);
        let mut bytes = self.w.into_vec();
        for (at, label) in self.patches {
            let target = self.labels[label.0].expect("every jumped-to label is placed");
            bytes[at..at + 4].copy_from_slice(&target.to_le_bytes());
        }
        bytes
    }

    /// Whether the label is a `u32` jump target (`Jump*` operands are absolute
    /// offsets within the body).
    pub fn is_placed(&self, label: Label) -> bool {
        self.labels[label.0].is_some()
    }
}
