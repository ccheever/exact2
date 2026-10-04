fn main() {
    println!("cargo:rerun-if-changed=app.contract");
    let plan = contract::compile_path(std::path::Path::new("app.contract"))
        .unwrap_or_else(|error| panic!("app.contract: {error}"));
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(out.join("app.plan"), plan.encode()).unwrap();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // Link an activation manifest so common controls use Windows' current theme.
        let manifest = out.join("windows-desk.manifest");
        std::fs::write(&manifest, r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="1.0.0.0" processorArchitecture="*" name="Exact.WindowsDesk" type="win32"/>
  <description>Windows Desk built with Exact2</description>
  <dependency><dependentAssembly><assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/></dependentAssembly></dependency>
</assembly>"#).unwrap();
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
}
