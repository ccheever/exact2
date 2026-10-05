//! Records written relative to their type's `Default`: a top-level field whose
//! value equals the default's is left out of saves and hashes, so a field added
//! with a default (an engine resource's new setting, a game's new argument)
//! leaves existing saves and pins unchanged. Readers patch records over
//! `Default` ("records keep fields the input lacks"), so a field left out reads
//! back as the value it had. Values that are not records are written whole.
use super::{hash::Hasher, Bulk, Data, Number, Writer};

/// Each top-level field's digest (its name and value), or None for a value that
/// is not a record.
pub(crate) fn fields<T: Data>(value: &T) -> Option<Vec<u64>> {
    let mut f = Fields::default();
    value.write(&mut f);
    f.record.then_some(f.done)
}

/// Write `value` without the top-level fields whose digests `defaults` holds
/// (`fields(&T::default())`). A hash receives each written field's digest in
/// place of its content; the digest covers the field's name.
pub(crate) fn write_changed<T: Data>(value: &T, defaults: Option<&[u64]>, w: &mut dyn Writer) {
    let (Some(defaults), Some(mine)) = (defaults, fields(value)) else {
        value.write(w);
        return;
    };
    if w.digests() {
        let kept: Vec<_> = mine.into_iter().filter(|h| !defaults.contains(h)).collect();
        w.begin_seq(kept.len());
        for h in kept {
            w.item();
            w.number(Number::Unsigned(h));
        }
        w.end_seq();
        return;
    }
    let keep = mine.iter().map(|h| !defaults.contains(h)).collect();
    value.write(&mut Only {
        out: w,
        keep,
        depth: 0,
        next: 0,
        on: false,
    });
}

// Hashes each top-level field of a record separately.
#[derive(Default)]
struct Fields {
    depth: usize,
    started: bool,
    record: bool,
    current: Option<Hasher>,
    done: Vec<u64>,
}
impl Fields {
    // A value event: at depth zero the value is not a record.
    fn value(&mut self) -> Option<&mut Hasher> {
        if self.depth == 0 {
            self.started = true;
            self.record = false;
        }
        self.current.as_mut()
    }
    fn close(&mut self) {
        if let Some(h) = self.current.take() {
            self.done.push(h.finish());
        }
    }
}
impl Writer for Fields {
    fn digests(&self) -> bool {
        true
    }
    fn unit(&mut self) {
        if let Some(h) = self.value() {
            h.unit();
        }
    }
    fn boolean(&mut self, value: bool) {
        if let Some(h) = self.value() {
            h.boolean(value);
        }
    }
    fn number(&mut self, value: Number) {
        if let Some(h) = self.value() {
            h.number(value);
        }
    }
    fn bytes(&mut self, value: Bulk<'_>) {
        if let Some(h) = self.value() {
            h.bytes(value);
        }
    }
    fn string(&mut self, value: &str) {
        if let Some(h) = self.value() {
            h.string(value);
        }
    }
    fn begin_seq(&mut self, len: usize) {
        if let Some(h) = self.value() {
            h.begin_seq(len);
        }
        self.depth += 1;
    }
    fn item(&mut self) {
        if let Some(h) = self.current.as_mut() {
            h.item();
        }
    }
    fn end_seq(&mut self) {
        self.depth -= 1;
        if let Some(h) = self.current.as_mut() {
            h.end_seq();
        }
    }
    fn begin_struct(&mut self) {
        if self.depth == 0 {
            self.record = !self.started;
            self.started = true;
        } else if let Some(h) = self.current.as_mut() {
            h.begin_struct();
        }
        self.depth += 1;
    }
    fn field(&mut self, name: &str) {
        if self.depth == 1 {
            self.close();
            let mut h = Hasher::default();
            h.string(name);
            self.current = Some(h);
        } else if let Some(h) = self.current.as_mut() {
            h.field(name);
        }
    }
    fn key(&mut self, name: &str) {
        if self.depth == 1 {
            // A map, replaced whole when read: not a record.
            self.record = false;
        } else if let Some(h) = self.current.as_mut() {
            h.key(name);
        }
    }
    fn end_struct(&mut self) {
        self.depth -= 1;
        if self.depth == 0 {
            self.close();
        } else if let Some(h) = self.current.as_mut() {
            h.end_struct();
        }
    }
    fn variant(&mut self, name: &str, index: u32) {
        if let Some(h) = self.value() {
            h.variant(name, index);
        }
        self.depth += 1;
    }
    fn end_variant(&mut self) {
        self.depth -= 1;
        if let Some(h) = self.current.as_mut() {
            h.end_variant();
        }
    }
    fn option(&mut self, some: bool) {
        if let Some(h) = self.value() {
            h.option(some);
        }
        self.depth += 1;
    }
    fn end_option(&mut self) {
        self.depth -= 1;
        if let Some(h) = self.current.as_mut() {
            h.end_option();
        }
    }
}

// Forwards a record, keeping only the top-level fields `keep` marks.
struct Only<'w> {
    out: &'w mut dyn Writer,
    keep: Vec<bool>,
    depth: usize,
    next: usize,
    on: bool,
}
impl Only<'_> {
    fn out(&mut self) -> Option<&mut dyn Writer> {
        self.on.then_some(&mut *self.out)
    }
}
impl Writer for Only<'_> {
    fn digests(&self) -> bool {
        self.out.digests()
    }
    fn unit(&mut self) {
        if let Some(w) = self.out() {
            w.unit();
        }
    }
    fn boolean(&mut self, value: bool) {
        if let Some(w) = self.out() {
            w.boolean(value);
        }
    }
    fn number(&mut self, value: Number) {
        if let Some(w) = self.out() {
            w.number(value);
        }
    }
    fn bytes(&mut self, value: Bulk<'_>) {
        if let Some(w) = self.out() {
            w.bytes(value);
        }
    }
    fn string(&mut self, value: &str) {
        if let Some(w) = self.out() {
            w.string(value);
        }
    }
    fn begin_seq(&mut self, len: usize) {
        self.depth += 1;
        if let Some(w) = self.out() {
            w.begin_seq(len);
        }
    }
    fn item(&mut self) {
        if let Some(w) = self.out() {
            w.item();
        }
    }
    fn end_seq(&mut self) {
        self.depth -= 1;
        if let Some(w) = self.out() {
            w.end_seq();
        }
    }
    fn begin_struct(&mut self) {
        self.depth += 1;
        if self.depth == 1 {
            self.out.begin_struct();
        } else if let Some(w) = self.out() {
            w.begin_struct();
        }
    }
    fn field(&mut self, name: &str) {
        if self.depth == 1 {
            self.on = self.keep.get(self.next).copied().unwrap_or(true);
            self.next += 1;
        }
        if let Some(w) = self.out() {
            w.field(name);
        }
    }
    fn key(&mut self, name: &str) {
        if let Some(w) = self.out() {
            w.key(name);
        }
    }
    fn end_struct(&mut self) {
        self.depth -= 1;
        if self.depth == 0 {
            self.on = false;
            self.out.end_struct();
        } else if let Some(w) = self.out() {
            w.end_struct();
        }
    }
    fn variant(&mut self, name: &str, index: u32) {
        self.depth += 1;
        if let Some(w) = self.out() {
            w.variant(name, index);
        }
    }
    fn end_variant(&mut self) {
        self.depth -= 1;
        if let Some(w) = self.out() {
            w.end_variant();
        }
    }
    fn option(&mut self, some: bool) {
        self.depth += 1;
        if let Some(w) = self.out() {
            w.option(some);
        }
    }
    fn end_option(&mut self) {
        self.depth -= 1;
        if let Some(w) = self.out() {
            w.end_option();
        }
    }
}
