//! The independently linked Caltrain business-logic module.
//! The schedule model stays in data/, shared with the native first-frame bake.
exact_logic_abi::export!(caltrain_data::Caltrain, caltrain_data::Caltrain);
// The line map, drawn for a host whose runner is not Rust.
exact_logic_abi::export_draw!(caltrain_data::Caltrain);
