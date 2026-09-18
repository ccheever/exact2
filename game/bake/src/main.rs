fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        eprintln!("usage: exact-game-bake INPUT.glb OUTPUT.model");
        std::process::exit(2);
    }
    let result =
        exact_game_bake::assets(std::path::Path::new(&args[0])).and_then(|(m, textures)| {
            let out = std::path::Path::new(&args[1]);
            std::fs::create_dir_all(out.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(out, exact_game::bin::to_vec(&m)).map_err(|e| e.to_string())?;
            for (name, texture) in textures {
                let path = out.parent().unwrap().join(name);
                std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
                std::fs::write(path, exact_game::bin::to_vec(&texture))
                    .map_err(|e| e.to_string())?;
            }
            Ok::<_, String>(())
        });
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
