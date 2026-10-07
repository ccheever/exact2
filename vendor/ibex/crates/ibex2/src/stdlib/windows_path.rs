//! Exact patch 4: native request normalization shares the pure grant grammar.
//! @ref LLP 1027.001#proposed-windows-native-filesystem-grant-integration — no parallel parser.
pub(crate) use exact_grants::WindowsPath as NativePath;
