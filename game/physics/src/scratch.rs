//! Derived storage: capacity and ordering hints never affect the arithmetic order.
use crate::solver::{Constraint, Node};
use exact_game::Vec3;

#[derive(Clone, Debug, Default)]
pub(crate) struct Scratch {
    pub nodes: Vec<Node>,
    pub constraints: Vec<Constraint>,
    pub bounds: Vec<(usize, Vec3, Vec3)>,
    pub statics: Vec<(usize, Vec3, Vec3)>,
    pub pairs: Vec<(usize, usize)>,
}

/// Small clipping polygons have at most eight vertices (quad clipped by quad).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Inline<T: Copy + Default, const N: usize> {
    data: [T; N],
    len: usize,
}
impl<T: Copy + Default, const N: usize> Default for Inline<T, N> {
    fn default() -> Self {
        Self {
            data: [T::default(); N],
            len: 0,
        }
    }
}
impl<T: Copy + Default, const N: usize> Inline<T, N> {
    pub fn push(&mut self, value: T) {
        self.data[self.len] = value;
        self.len += 1;
    }
    pub fn remove(&mut self, i: usize) -> T {
        let value = self.data[i];
        self.data.copy_within(i + 1..self.len, i);
        self.len -= 1;
        value
    }
    pub fn dedup_by(&mut self, mut same: impl FnMut(&T, &T) -> bool) {
        let mut i = 1;
        while i < self.len {
            if same(&self.data[i - 1], &self.data[i]) {
                self.remove(i);
            } else {
                i += 1;
            }
        }
    }
}
impl<T: Copy + Default, const N: usize> std::ops::Deref for Inline<T, N> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.data[..self.len]
    }
}
impl<T: Copy + Default, const N: usize> std::ops::DerefMut for Inline<T, N> {
    fn deref_mut(&mut self) -> &mut [T] {
        &mut self.data[..self.len]
    }
}
impl<T: Copy + Default, const N: usize> IntoIterator for Inline<T, N> {
    type Item = T;
    type IntoIter = std::iter::Take<std::array::IntoIter<T, N>>;
    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter().take(self.len)
    }
}
impl<'a, T: Copy + Default, const N: usize> IntoIterator for &'a Inline<T, N> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a, T: Copy + Default, const N: usize> IntoIterator for &'a mut Inline<T, N> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
impl<T: Copy + Default, const N: usize> FromIterator<T> for Inline<T, N> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut out = Self::default();
        for value in iter {
            out.push(value);
        }
        out
    }
}
