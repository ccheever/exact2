//! Colours in a profile's space (LLP 1100 D1, D3): `color(--name c…)`, a
//! standard space (`exact_color::PROFILES`) or an app's `@color-profile`.
//! Only the platform converts them: no sRGB fallback exists.

use super::ColorValue;
use std::sync::{Arc, Mutex};

/// Where the platform finds a profile's space.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// Apple's `CGColorSpace` name.
    Named(&'static str),
    /// An app's ICC file and its rendering intent.
    Icc {
        /// The file, as an asset path.
        src: Box<str>,
        /// CSS Color 5's rendering intent.
        intent: Box<str>,
    },
}

/// One interned colour in a profile's space.
#[derive(Debug, PartialEq)]
pub struct ProfiledValue {
    /// The space.
    pub source: Source,
    /// The components as written, in the profile's own order and range.
    pub components: Vec<f32>,
    /// Alpha, 0–1.
    pub alpha: f32,
    /// CSS's serialisation.
    pub text: Box<str>,
}

/// A declared profile: its name, ICC asset and rendering intent.
type Declared = (Box<str>, Box<str>, Box<str>);
static DECLARED: Mutex<Vec<Declared>> = Mutex::new(Vec::new());
static PROFILED: Mutex<Vec<Arc<ProfiledValue>>> = Mutex::new(Vec::new());
const PROFILED_CAP: usize = 1024;

/// An app's `@color-profile`; a later declaration of a name replaces it.
pub fn declare(name: &str, src: &str, intent: &str) {
    if let Ok(mut d) = DECLARED.lock() {
        d.retain(|(n, ..)| &**n != name);
        d.push((name.into(), src.into(), intent.into()));
    }
}

/// An interned profiled colour by id.
pub fn profiled(id: u16) -> Option<Arc<ProfiledValue>> {
    PROFILED.lock().ok()?.get(usize::from(id)).cloned()
}

/// `color(--name c… [/ alpha])` interned; `None` for an unknown name, a
/// wrong component count, or a host that can't show it.
pub fn parse_profiled(text: &str) -> Option<ColorValue> {
    let (name, components, alpha) = exact_color::parse_profiled(text)?;
    if super::wide::limited() {
        return None;
    }
    let source = if let Some((_, apple, n)) = exact_color::PROFILES.iter().find(|p| p.0 == name) {
        if components.len() != *n {
            return None;
        }
        Source::Named(apple)
    } else {
        let declared = DECLARED.lock().ok()?;
        let (_, src, intent) = declared.iter().find(|d| *d.0 == *name)?;
        Source::Icc {
            src: src.clone(),
            intent: intent.clone(),
        }
    };
    let n = exact_color::number_text;
    let mut canonical = format!("color({name}");
    for c in &components {
        canonical.push(' ');
        canonical.push_str(&n(*c));
    }
    if alpha < 1.0 {
        canonical.push_str(" / ");
        canonical.push_str(&n(alpha));
    }
    canonical.push(')');
    let mut table = PROFILED.lock().ok()?;
    if let Some(i) = table
        .iter()
        .position(|p| *p.text == *canonical && p.source == source)
    {
        return Some(ColorValue::Profiled(u16::try_from(i).ok()?));
    }
    if table.len() >= PROFILED_CAP {
        return None;
    }
    table.push(Arc::new(ProfiledValue {
        source,
        components: components.iter().map(|c| *c as f32).collect(),
        alpha: alpha as f32,
        text: canonical.into(),
    }));
    Some(ColorValue::Profiled(u16::try_from(table.len() - 1).ok()?))
}
