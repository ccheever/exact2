fn main() {
    println!("cargo:rerun-if-changed=src/thumbnail.m");
    cc::Build::new()
        .file("src/thumbnail.m")
        .flag("-fobjc-arc")
        .compile("thumbnail");
    for framework in ["Foundation", "ImageIO", "CoreGraphics"] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
}
