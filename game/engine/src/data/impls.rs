use super::{Data, DataError, Number, Reader, Writer};
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
        *self = match r.number()? {
            Number::Unsigned(n) => n as f32,
            Number::Signed(n) => n as f32,
            Number::F32(n) => n,
            Number::F64(n) => {
                let small = n as f32;
                if n.is_finite() && !small.is_finite() {
                    return Err(DataError::new("number outside f32 range"));
                }
                small
            }
        };
        Ok(())
    }
}
impl Data for f64 {
    fn write(&self, w: &mut dyn Writer) {
        w.number(Number::F64(*self));
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        *self = match r.number()? {
            Number::Unsigned(n) => n as f64,
            Number::Signed(n) => n as f64,
            Number::F32(n) => n as f64,
            Number::F64(n) => n,
        };
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
impl<T: Data> Data for Vec<T> {
    fn moving(&self, now: crate::Now) -> bool {
        self.iter().any(|v| v.moving(now))
    }
    fn write(&self, w: &mut dyn Writer) {
        w.begin_seq(self.len());
        for v in self {
            w.item();
            v.write(w);
        }
        w.end_seq();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        super::limits::read_vec(r, self, super::MAX_LOAD_BYTES)
    }
}
impl<T: Data> Data for Option<T> {
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
            let mut value = T::default();
            value.read(r)?;
            *self = Some(value);
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
    fn write(&self, w: &mut dyn Writer) {
        w.begin_seq(N);
        for v in self {
            w.item();
            v.write(w);
        }
        w.end_seq();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.begin_seq()?;
        *self = Self::default();
        let mut i = 0;
        while r.item()? {
            if let Some(v) = self.get_mut(i) {
                v.read(r).map_err(|e| e.at(i))?;
            } else {
                r.skip()?;
            }
            i += 1;
        }
        Ok(())
    }
}
impl<T: Data> Data for Box<T> {
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
        self.clear();
        while let Some(k) = r.field()? {
            r.claim(64 + std::mem::size_of::<T>())?;
            let mut value = T::default();
            value.read(r).map_err(|e| e.at(&k))?;
            self.insert(k, value);
        }
        Ok(())
    }
}
macro_rules! tuple {
    ($n:expr; $($T:ident:$i:tt),*) => {
        impl<$($T: Data),*> Data for ($($T,)*) {
            fn moving(&self, now: crate::Now) -> bool { false $(|| self.$i.moving(now))* }
            fn write(&self, w: &mut dyn Writer) {
                w.begin_seq($n); $(w.item(); self.$i.write(w);)* w.end_seq();
            }
            fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
                r.begin_seq()?; *self = Self::default(); let mut i = 0;
                while r.item()? {
                    match i { $($i => self.$i.read(r).map_err(|e| e.at(i))?,)* _ => r.skip()?, }
                    i += 1;
                } Ok(())
            }
        }
    };
}
impl Data for () {
    fn write(&self, w: &mut dyn Writer) {
        w.begin_seq(0);
        w.end_seq();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.begin_seq()?;
        while r.item()? {
            r.skip()?;
        }
        Ok(())
    }
}
tuple!(1; A:0);
tuple!(2; A:0, B:1);
tuple!(3; A:0, B:1, C:2);
tuple!(4; A:0, B:1, C:2, D:3);

macro_rules! vector {
    ($($ty:ty),*) => {$(impl Data for $ty {
        fn write(&self, w: &mut dyn Writer) { self.to_array().write(w); }
        fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
            let mut a = self.to_array(); a.read(r)?; *self = Self::from_array(a); Ok(())
        }
    })*};
}
vector!(glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat);
