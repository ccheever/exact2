#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgumentKind {
    Setup,
    Restart,
    Live,
}
pub trait Args: crate::Data {
    fn argument(&self, name: &str) -> Option<ArgumentRef<'_>>;
    const FIELDS: &'static [(&'static str, ArgumentKind)];
    fn check_scalars(&self) -> Result<(), String>;
    fn setup_changed(&self, next: &Self) -> bool;
}
impl Args for () {
    fn argument(&self, _: &str) -> Option<ArgumentRef<'_>> {
        None
    }
    const FIELDS: &'static [(&'static str, ArgumentKind)] = &[];
    fn check_scalars(&self) -> Result<(), String> {
        Ok(())
    }
    fn setup_changed(&self, _: &Self) -> bool {
        false
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArgumentRef<'a> {
    Bool(bool),
    Unsigned(u64),
    Signed(i64),
    Float(f64),
    Text(&'a str),
}
pub struct SetupArgs<'a, A: Args>(pub(crate) &'a A);
impl<A: Args> SetupArgs<'_, A> {
    pub fn get(&self, name: &str) -> Option<ArgumentRef<'_>> {
        A::FIELDS
            .iter()
            .find(|(n, k)| *n == name && *k != ArgumentKind::Live)?;
        self.0.argument(name)
    }
}
