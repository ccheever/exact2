fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        eprintln!("usage: exact-game-bake INPUT.glb OUTPUT.model");
        std::process::exit(2);
    }
    let result = exact_game_bake::model(std::path::Path::new(&args[0])).and_then(|m| {
        std::fs::write(&args[1], exact_game::bin::to_vec(&m)).map_err(|e| e.to_string())
    });
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
