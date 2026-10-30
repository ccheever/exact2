fn main() {
    std::process::exit(exact_apple_painted::run::<exact_heavylist_data::Heavy>(
        "Heavy list",
        include_bytes!(concat!(env!("OUT_DIR"), "/app.plan")),
        include_str!(concat!(env!("OUT_DIR"), "/compat.json")),
    ));
}
