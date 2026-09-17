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
macro_rules! integer {
    ($kind:ident, $($ty:ty),+) => {$(impl Data for $ty {
        fn write(&self, w: &mut dyn Writer) { w.number(Number::$kind((*self).into())); }
        fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
            *self = match r.number()? {
                Number::Unsigned(n) => Self::try_from(n).ok(),
                Number::Signed(n) => Self::try_from(n).ok(),
                _ => None,
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
    fn write(&self, w: &mut dyn Writer) {
        w.begin_seq(self.len());
        for v in self {
            w.item();
            v.write(w);
        }
        w.end_seq();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.begin_seq()?;
        self.clear();
        while r.item()? {
            let mut v = T::default();
            v.read(r).map_err(|e| e.at(self.len()))?;
            self.push(v);
        }
        Ok(())
    }
}
impl<T: Data> Data for Option<T> {
    fn write(&self, w: &mut dyn Writer) {
        w.option(self.is_some());
        if let Some(v) = self {
            v.write(w);
        }
        w.end_option();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        if r.option()? {
            self.get_or_insert_with(T::default).read(r)?;
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
    fn write(&self, w: &mut dyn Writer) {
        (**self).write(w);
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        (**self).read(r)
    }
}
impl<T: Data> Data for BTreeMap<String, T> {
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
        while let Some(k) = r.field()? {
            self.entry(k.clone())
                .or_default()
                .read(r)
                .map_err(|e| e.at(k))?;
        }
        Ok(())
    }
}
macro_rules! tuple {
    ($n:expr; $($T:ident:$i:tt),*) => {
        impl<$($T: Data),*> Data for ($($T,)*) {
            fn write(&self, w: &mut dyn Writer) {
                w.begin_seq($n); $(w.item(); self.$i.write(w);)* w.end_seq();
            }
            fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
                r.begin_seq()?; let mut i = 0;
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
