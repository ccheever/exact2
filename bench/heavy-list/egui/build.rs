fn main() {
    println!("cargo:rerun-if-changed=../gpui/src/thumbnail.m");
    cc::Build::new()
        .file("../gpui/src/thumbnail.m")
        .flag("-fobjc-arc")
        .flag("-mmacosx-version-min=14.0")
        .compile("thumbnail");
    for framework in ["Foundation", "ImageIO", "CoreGraphics"] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
    println!("cargo:rustc-link-arg=-mmacosx-version-min=14.0");
    println!("cargo:rustc-link-arg=-Wl,-headerpad_max_install_names");
}
