//! A location, a retained stack per tab, and six pure verbs.
//!
//! @ref LLP 1038 §3 (the whole core), D1–D3 (URLs and patterns), D9 (one crate).
//!
//! The table says which screen a location names and what sits beneath it on
//! a deep link. The value says where the user has been. An entry's id belongs
//! to that visit, not to the screen: pushing the same URL twice makes two
//! entries, and popping never lends an old id to a new visit.
//!
//! Nothing here runs an action or talks to a host. A refused verb returns
//! the input value and a message for its caller to journal. Serde carries
//! the same ordinary records across the data seam; the plan's positional
//! records are somebody else's conversion.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod location;
mod router;
mod table;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use location::{canonical, encode_uri_component, location_of, search_param};
pub use router::{back, depth, go, open, params, push, replace, select, stack, top};
pub use table::encode_route_segment;

/// Every table parameter, including unbound names as `""`. JSON keys are sorted;
/// [`Table::param_names`] supplies the first-declaration order for plan records.
pub type Params = BTreeMap<String, String>;

/// One declaration. A notfound row uses an empty pattern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Route {
    /// The route's distinct name.
    pub name: String,
    /// An absolute path of literal segments and `:identifier` segments.
    pub pattern: String,
    /// Index of the declared parent, if any.
    pub parent: Option<usize>,
    /// This row is a tab root.
    pub tab: bool,
    /// Absorb a location that no pattern matches.
    pub notfound: bool,
}

/// The declaration, in match order. Validate once with [`Table::check`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Table {
    /// Earlier patterns win; this is also parameter declaration order.
    pub routes: Vec<Route>,
}

/// A location's screen and decoded parameter record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Match {
    /// The first matching route's name, or the first notfound row's name.
    pub name: String,
    /// All the table's parameter names, unbound names empty.
    pub params: Params,
}

/// An entry before it has a visit id. [`Table::chain`] does not spend ids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Destination {
    /// The screen's name.
    pub name: String,
    /// Its canonical path and query.
    pub url: String,
    /// The tab whose stack holds this entry.
    pub tab: String,
    /// Parameters bound by this screen's own pattern.
    pub params: Params,
}

/// One visit to a screen. @ref LLP 1038 §3 — Entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// A monotone visit id, never reused.
    pub id: u64,
    /// The screen's name.
    pub name: String,
    /// Its canonical path and query.
    pub url: String,
    /// The tab whose stack holds this entry, including a cross-tab push.
    pub tab: String,
    /// All the table's parameter names, unbound names empty.
    pub params: Params,
}

/// A retained stack. @ref LLP 1038 §3 — Router.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tab {
    /// The tab root's route name.
    pub name: String,
    /// Root first; never empty after a successful launch.
    pub stack: Vec<Entry>,
}

/// Navigation is one serializable value. @ref LLP 1038 §3 — Router.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Router {
    /// The selected tab's name.
    pub tab: String,
    /// All retained tabs, in declaration order.
    pub tabs: Vec<Tab>,
    /// The next visit id. A fresh value begins at zero.
    pub next: u64,
}

/// A static table reject, using D2's reject ids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckError {
    /// `route-duplicate`, `route-shadowed`, `route-parent-param`, or `route-pattern`.
    pub code: String,
    /// The offending row's index.
    pub route: usize,
    /// What the declaration must fix.
    pub message: String,
}

/// A path formatting error; its code is `route-unknown` (D3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathError {
    /// The compiler's reject id.
    pub code: String,
    /// The unknown route or wrong arity.
    pub message: String,
}

/// A verb did not commit. The caller journals this message once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    /// The reason the input value was returned unchanged.
    pub message: String,
}

macro_rules! display_error {
    ($($ty:ty),*) => {$(
        impl std::fmt::Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.message)
            }
        }
        impl std::error::Error for $ty {}
    )*};
}
display_error!(CheckError, PathError, Refusal);
