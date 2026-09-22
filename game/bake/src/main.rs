fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        eprintln!(
            "usage: exact-game-bake --art APP_DIR | INPUT.glb OUTPUT.model | INPUT.png OUTPUT.tex"
        );
        std::process::exit(2);
    }
    if args[0] == "--art" {
        if let Err(error) = exact_game_bake::bake_art(&args[1]) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if std::path::Path::new(&args[0])
        .extension()
        .is_some_and(|v| v == "png")
    {
        let result = exact_game_bake::sprite(&args[0]).and_then(|texture| {
            let name = args[1].to_string_lossy();
            let bytes = exact_game_bake::encode(&name, &texture)?;
            if let Some(parent) = std::path::Path::new(&args[1])
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&args[1], bytes).map_err(|e| e.to_string())
        });
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    let result =
        exact_game_bake::assets(std::path::Path::new(&args[0])).and_then(|(m, textures)| {
            let out = std::path::Path::new(&args[1]);
            std::fs::create_dir_all(out.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(
                out,
                exact_game_bake::encode(&out.display().to_string(), &m)?,
            )
            .map_err(|e| e.to_string())?;
            for (name, texture) in textures {
                let path = out.parent().unwrap().join(&name);
                std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
                std::fs::write(path, exact_game_bake::encode(&name, &texture)?)
                    .map_err(|e| e.to_string())?;
            }
            Ok::<_, String>(())
        });
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
