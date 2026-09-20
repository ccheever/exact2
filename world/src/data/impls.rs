use super::BulkKind;
use super::{Data, DataError, Number, Reader, Writer};
use std::any::Any;
use std::collections::BTreeMap;

impl Data for bool {
    fn write(&self, w: &mut dyn Writer) {
        w.boolean(*self);
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        *self = r.boolean()?;
        Ok(())
    }
}
// i128 holds every supported integer exactly, including u64::MAX. Comparing
// after conversion avoids the rounded f64 upper bound at i64::MAX/u64::MAX.
fn integral(n: f64) -> Option<i128> {
    (n.is_finite() && n.fract() == 0.0 && n >= i64::MIN as f64 && n < 18_446_744_073_709_551_616.0)
        .then_some(n as i128)
}
macro_rules! integer {
    ($kind:ident, $($ty:ty),+) => {$(impl Data for $ty {
        fn write(&self, w: &mut dyn Writer) { w.number(Number::$kind((*self).into())); }
        fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
            *self = match r.number()? {
                Number::Unsigned(n) => Self::try_from(n).ok(),
                Number::Signed(n) => Self::try_from(n).ok(),
                Number::F32(n) => integral(n as f64).and_then(|n| Self::try_from(n).ok()),
                Number::F64(n) => integral(n).and_then(|n| Self::try_from(n).ok()),
            }.ok_or_else(|| DataError::new(concat!("expected an integer in ", stringify!($ty), " range")))?;
            Ok(())
        }
    })+};
}
integer!(Unsigned, u8, u16, u32, u64);
integer!(Signed, i8, i16, i32, i64);
impl Data for f32 {
    fn write(&self, w: &mut dyn Writer) {
        w.number(Number::F32(*self));
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        *self = r.f32()?;
        Ok(())
    }
}
impl Data for f64 {
    fn write(&self, w: &mut dyn Writer) {
        w.number(Number::F64(*self));
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        *self = r.f64()?;
        Ok(())
    }
}
impl Data for String {
    fn write(&self, w: &mut dyn Writer) {
        w.string(self);
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        *self = r.string()?;
        Ok(())
    }
}
impl Data for std::borrow::Cow<'static, str> {
    fn write(&self, w: &mut dyn Writer) {
        w.string(self);
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        *self = std::borrow::Cow::Owned(r.string()?);
        Ok(())
    }
}
impl<T: Data> Data for Vec<T> {
    fn moving(&self, now: crate::Now) -> bool {
        self.iter().any(|v| v.moving(now))
    }
    fn settle_tick(&self, now: crate::Now) -> Option<u64> {
        self.iter()
            .try_fold(now.tick, |at, v| Some(at.max(v.settle_tick(now)?)))
    }
    fn write(&self, w: &mut dyn Writer) {
        // Stable Rust has no specialization: downcasts select the four closed
        // bulk types without unsafe layout casts or changing other Vec<T> values.
        let any = self as &dyn Any;
        if let Some(v) = any.downcast_ref::<Vec<u8>>() {
            w.bytes(BulkKind::U8, v);
            return;
        }
        macro_rules! bulk {
            ($ty:ty, $kind:ident, $bytes:expr) => {
                if let Some(v) = any.downcast_ref::<Vec<$ty>>() {
                    let bytes: Vec<u8> = v.iter().flat_map($bytes).collect();
                    w.bytes(BulkKind::$kind, &bytes);
                    return;
                }
            };
        }
        bulk!(u16, U16, |v: &u16| v.to_le_bytes());
        bulk!(u32, U32, |v: &u32| v.to_le_bytes());
        bulk!(f32, F32, |v: &f32| super::f32_bits(*v).to_le_bytes());
        w.begin_seq(self.len());
        for v in self {
            w.item();
            v.write(w);
        }
        w.end_seq();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        let any = self as &mut dyn Any;
        if let Some(v) = any.downcast_mut::<Vec<u8>>() {
            *v = r.bytes(BulkKind::U8)?;
            return Ok(());
        }
        macro_rules! bulk {
            ($ty:ty, $kind:ident, $decode:expr) => {
                if let Some(v) = any.downcast_mut::<Vec<$ty>>() {
                    let bytes = r.bytes(BulkKind::$kind)?;
                    const WIDTH: usize = std::mem::size_of::<$ty>();
                    if bytes.len() % WIDTH != 0 {
                        return Err(DataError::new(concat!(
                            "invalid byte length for Vec<",
                            stringify!($ty),
                            ">"
                        )));
                    }
                    // Reader::bytes already claimed WIDTH * element count.
                    let mut out = Vec::new();
                    out.try_reserve_exact(bytes.len() / WIDTH)
                        .map_err(super::limits::allocation)?;
                    out.extend(bytes.chunks_exact(WIDTH).map($decode));
                    *v = out;
                    return Ok(());
                }
            };
        }
        bulk!(u16, U16, |b: &[u8]| u16::from_le_bytes(
            b.try_into().unwrap()
        ));
        bulk!(u32, U32, |b: &[u8]| u32::from_le_bytes(
            b.try_into().unwrap()
        ));
        bulk!(f32, F32, |b: &[u8]| f32::from_bits(super::f32_bits(
            f32::from_le_bytes(b.try_into().unwrap())
        )));
        super::limits::read_vec(r, self, super::MAX_LOAD_BYTES)
    }
}
impl<T: Data> Data for Option<T> {
    fn settle_tick(&self, now: crate::Now) -> Option<u64> {
        self.as_ref().map_or(Some(now.tick), |v| v.settle_tick(now))
    }
    fn moving(&self, now: crate::Now) -> bool {
        self.as_ref().is_some_and(|v| v.moving(now))
    }
    fn write(&self, w: &mut dyn Writer) {
        w.option(self.is_some());
        if let Some(v) = self {
            v.write(w);
        }
        w.end_option();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        if r.option()? {
            if r.patching() {
                self.get_or_insert_with(T::default).read(r)?;
            } else {
                let mut value = T::default();
                value.read(r)?;
                *self = Some(value);
            }
        } else {
            *self = None;
        }
        r.end_option()
    }
}
impl<T: Data, const N: usize> Data for [T; N]
where
    [T; N]: Default,
{
    fn moving(&self, now: crate::Now) -> bool {
        self.iter().any(|v| v.moving(now))
    }
    fn settle_tick(&self, now: crate::Now) -> Option<u64> {
        self.iter()
            .try_fold(now.tick, |at, v| Some(at.max(v.settle_tick(now)?)))
    }
    fn write(&self, w: &mut dyn Writer) {
        write_slice(self, w);
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.begin_seq()?;
        if !r.patching() {
            *self = Self::default();
        }
        read_slice(self, r)
    }
}
// One walk per element type, not per array length. Keep byte-vector codecs above
// separate: arrays' scalar sequence tags are part of existing saves and hashes.
#[inline(never)]
fn write_slice<T: Data>(values: &[T], w: &mut dyn Writer) {
    w.begin_seq(values.len());
    for v in values {
        w.item();
        v.write(w);
    }
    w.end_seq();
}
#[inline(never)]
fn read_slice<T: Data>(values: &mut [T], r: &mut dyn Reader) -> Result<(), DataError> {
    let mut i = 0;
    while r.item()? {
        if let Some(v) = values.get_mut(i) {
            v.read(r).map_err(|e| e.at(i))?;
        } else {
            if r.strict() {
                return Err(DataError::new("wrong array length"));
            }
            r.skip()?;
        }
        i += 1;
    }
    if r.strict() && i != values.len() {
        return Err(DataError::new("wrong array length"));
    }
    Ok(())
}
impl<T: Data> Data for Box<T> {
    fn settle_tick(&self, now: crate::Now) -> Option<u64> {
        (**self).settle_tick(now)
    }
    fn moving(&self, now: crate::Now) -> bool {
        (**self).moving(now)
    }
    fn write(&self, w: &mut dyn Writer) {
        (**self).write(w);
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.claim(std::mem::size_of::<T>())?;
        (**self).read(r)
    }
}
impl<T: Data> Data for BTreeMap<String, T> {
    fn moving(&self, now: crate::Now) -> bool {
        self.values().any(|v| v.moving(now))
    }
    fn settle_tick(&self, now: crate::Now) -> Option<u64> {
        self.values()
            .try_fold(now.tick, |at, v| Some(at.max(v.settle_tick(now)?)))
    }
    fn write(&self, w: &mut dyn Writer) {
        w.begin_struct();
        for (k, v) in self {
            w.key(k);
            v.write(w);
        }
        w.end_struct();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.begin_struct()?;
        if !r.patching() {
            self.clear();
        }
        while let Some(k) = r.field()? {
            r.claim(64 + std::mem::size_of::<T>())?;
            if r.patching() {
                self.entry(k.clone())
                    .or_default()
                    .read(r)
                    .map_err(|e| e.at(&k))?;
            } else {
                let mut value = T::default();
                value.read(r).map_err(|e| e.at(&k))?;
                self.insert(k, value);
            }
        }
        Ok(())
    }
}
macro_rules! tuple {
    ($n:expr; $($T:ident:$i:tt),*) => {
        impl<$($T: Data),*> Data for ($($T,)*) {
            fn moving(&self, now: crate::Now) -> bool { false $(|| self.$i.moving(now))* }
            fn settle_tick(&self, now: crate::Now) -> Option<u64> { Some(now.tick $(.max(self.$i.settle_tick(now)?))*) }
            fn write(&self, w: &mut dyn Writer) {
                w.begin_seq($n); $(w.item(); self.$i.write(w);)* w.end_seq();
            }
            fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
                r.begin_seq()?; *self = Self::default(); let mut i = 0;
                while r.item()? {
                    match i { $($i => self.$i.read(r).map_err(|e| e.at(i))?,)* _ => r.unknown().map_err(|e| e.at(i))?, }
                    i += 1;
                }
                if r.strict() && i != $n { return Err(DataError::new("wrong tuple length")); }
                Ok(())
            }
        }
    };
}
impl Data for () {
    fn write(&self, w: &mut dyn Writer) {
        w.unit();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.begin_seq()?;
        while r.item()? {
            r.unknown()?;
        }
        Ok(())
    }
}
tuple!(1; A:0);
tuple!(2; A:0, B:1);
tuple!(3; A:0, B:1, C:2);
tuple!(4; A:0, B:1, C:2, D:3);
