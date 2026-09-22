//! Small, bounded asset registries. Lexical iteration keeps diagnostics stable;
//! these delivery maps are not Data and never enter the simulation hash/save.
#[derive(Clone)]
pub(crate) struct AssetMap<V>(Vec<(String, V)>, u64);
impl<V> Default for AssetMap<V> {
    fn default() -> Self {
        Self::EMPTY
    }
}
impl<V> AssetMap<V> {
    pub(crate) const EMPTY: Self = Self(Vec::new(), 0);
    pub(crate) fn revision(&self) -> u64 {
        self.1
    }
    fn find(&self, name: &str) -> Result<usize, usize> {
        self.0.binary_search_by(|(key, _)| key.as_str().cmp(name))
    }
    pub(crate) fn get(&self, name: &str) -> Option<&V> {
        self.find(name).ok().map(|i| &self.0[i].1)
    }
    pub(crate) fn contains_key(&self, name: &str) -> bool {
        self.find(name).is_ok()
    }
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
    pub(crate) fn insert(&mut self, name: String, value: V) {
        self.1 = self.1.wrapping_add(1);
        match self.find(&name) {
            Ok(i) => self.0[i].1 = value,
            Err(i) => self.0.insert(i, (name, value)),
        }
    }
    pub(crate) fn remove(&mut self, name: &str) {
        if let Ok(i) = self.find(name) {
            self.0.remove(i);
            self.1 = self.1.wrapping_add(1);
        }
    }
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&String, &V)> {
        self.0.iter().map(|(k, v)| (k, v))
    }
    pub(crate) fn values(&self) -> impl Iterator<Item = &V> {
        self.0.iter().map(|(_, v)| v)
    }
    pub(crate) fn keys(&self) -> impl Iterator<Item = &String> {
        self.0.iter().map(|(k, _)| k)
    }
}
impl<'a, V> IntoIterator for &'a AssetMap<V> {
    type Item = (&'a String, &'a V);
    type IntoIter =
        std::iter::Map<std::slice::Iter<'a, (String, V)>, fn(&'a (String, V)) -> Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().map(|(k, v)| (k, v))
    }
}
