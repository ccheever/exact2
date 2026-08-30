// swift-tools-version:5.9
// The iOS presenter: host glue over the C ABI in ../include/exact.h, the
// macOS package's shape on UIKit. Links the app's static library built for
// the simulator (cargo build --release -p caltrain-apple --target
// aarch64-apple-ios-sim); EXACT_LIB_DIR overrides where it is found.
// Built by `node host/apple/build.mjs --ios`, which also assembles the .app.
import PackageDescription
import Foundation

let libDir = ProcessInfo.processInfo.environment["EXACT_LIB_DIR"] ?? (Context.packageDirectory + "/../../../target/aarch64-apple-ios-sim/release")
let libName = ProcessInfo.processInfo.environment["EXACT_LIB"] ?? "caltrain_apple"

let package = Package(
    name: "ExactIOS",
    platforms: [.iOS(.v17)],
    targets: [
        .systemLibrary(name: "CExact", path: "Sources/CExact"),
        .executableTarget(
            name: "ExactIOS",
            dependencies: ["CExact"],
            path: "Sources/ExactIOS",
            linkerSettings: [.unsafeFlags(["-L", libDir]), .linkedLibrary(libName), .linkedLibrary("c++")]
        ),
    ]
)
