//! Build-only composition shared by this app's Web, Apple and Linux entries.

use std::ffi::OsStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Reuse,
    Stateless,
}

impl Source {
    pub fn parse(value: Option<&OsStr>) -> Result<Self, &'static str> {
        match value {
            None => Ok(Self::Reuse),
            Some(value) => match value.to_str() {
                Some("reuse") => Ok(Self::Reuse),
                Some("stateless") => Ok(Self::Stateless),
                _ => Err("EXACT_MESSAGES_SOURCE must be exactly reuse or stateless (or unset)"),
            },
        }
    }

    /// Called before compiling or baking the Contract; malformed selectors cannot
    /// leave a newly baked plan behind. No runtime environment lookup is needed.
    pub fn selected() -> Self {
        println!("cargo:rerun-if-env-changed=EXACT_MESSAGES_SOURCE");
        println!("cargo:rerun-if-changed=../factory.rs");
        Self::parse(std::env::var_os("EXACT_MESSAGES_SOURCE").as_deref())
            .unwrap_or_else(|error| panic!("{error}"))
    }

    pub fn entry(self, rust_mode: &str) -> Result<String, String> {
        let (source, constructor) = match self {
            Self::Reuse => (
                "messages_stress_data::ReusableMessagesStress",
                "messages_stress_data::ReusableMessagesStress::default()",
            ),
            Self::Stateless => (
                "messages_stress_data::MessagesStress",
                "messages_stress_data::MessagesStress",
            ),
        };
        contract::rust_entry(source, constructor, rust_mode)
    }
}
