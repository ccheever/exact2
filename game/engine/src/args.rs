use crate::Value;

/// A canvas argument that either constructs the world or is read live.
#[derive(Clone, Copy, Debug)]
pub struct Arg {
    pub(crate) name: &'static str,
    pub(crate) setup: bool,
}
impl Arg {
    /// Changing this argument constructs a fresh world at tick zero.
    pub const fn setup(name: &'static str) -> Self {
        Self { name, setup: true }
    }
    /// Changing this argument is visible to subsequent ticks without a restart.
    pub const fn live(name: &'static str) -> Self {
        Self { name, setup: false }
    }
}
/// Named canvas arguments with refusals that name the author-facing argument.
#[derive(Clone, Default)]
pub struct Args {
    pub(crate) values: Vec<Value>,
    pub(crate) declarations: &'static [Arg],
}
impl Args {
    fn index(&self, name: &str) -> usize {
        self.declarations
            .iter()
            .position(|a| a.name == name)
            .unwrap_or_else(|| panic!("unknown argument `{name}`"))
    }
    /// Number of supplied arguments.
    pub fn len(&self) -> usize {
        self.values.len()
    }
    /// Whether no arguments were supplied.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
    fn expected(&self, i: usize, name: &str, kind: &str) -> String {
        format!(
            "argument `{name}` at position {i}: expected {kind}{}",
            if i >= self.len() {
                ", got no value"
            } else {
                ""
            }
        )
    }
    /// Read text, refusing missing values or a different type.
    pub fn text(&self, name: &str) -> Result<&str, String> {
        let i = self.index(name);
        self.values
            .get(i)
            .and_then(Value::as_str)
            .ok_or_else(|| self.expected(i, name, "text"))
    }
    /// Read a finite number, refusing missing values or a different type.
    pub fn number(&self, name: &str) -> Result<f64, String> {
        let i = self.index(name);
        self.values
            .get(i)
            .and_then(Value::as_number)
            .filter(|n| n.is_finite())
            .ok_or_else(|| self.expected(i, name, "a finite number"))
    }
    /// Read a non-negative safe integer (at most 2^53 - 1), refusing by name.
    pub fn integer(&self, name: &str) -> Result<u64, String> {
        let i = self.index(name);
        let n = self.number(name)?;
        if !(0.0..=9_007_199_254_740_991.0).contains(&n) || n.fract() != 0.0 {
            return Err(self.expected(i, name, "a non-negative safe integer"));
        }
        Ok(n as u64)
    }
    pub(crate) fn arity(&self, game: &str, names: &[Arg]) -> Result<(), String> {
        if self.len() < names.len() {
            Err(format!(
                "missing argument `{}` at position {}",
                names[self.len()].name,
                self.len()
            ))
        } else if self.len() > names.len() {
            Err(format!(
                "{} expects {} arguments ({}), got {}",
                game,
                names.len(),
                names.iter().map(|a| a.name).collect::<Vec<_>>().join(", "),
                self.len()
            ))
        } else {
            Ok(())
        }
    }
    /// Read a boolean, refusing missing values or a different type.
    pub fn flag(&self, name: &str) -> Result<bool, String> {
        let i = self.index(name);
        self.values
            .get(i)
            .and_then(Value::as_bool)
            .ok_or_else(|| self.expected(i, name, "a boolean"))
    }
    pub(crate) fn json(&self) -> String {
        format!(
            "{{{}}}",
            self.declarations
                .iter()
                .zip(&self.values)
                .map(|(n, v)| format!(
                    "{}:{}",
                    crate::values::quote(n.name),
                    crate::values::value_json(v, true)
                ))
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}
