// swift-tools-version:5.9
// The macOS presenter: host glue over the C ABI in ../include/exact.h.
// Links the app's static library (cargo build --release -p caltrain-apple);
// EXACT_LIB_DIR overrides where it is found.
import PackageDescription
import Foundation

let libDir = ProcessInfo.processInfo.environment["EXACT_LIB_DIR"] ?? (Context.packageDirectory + "/../../../target/release")
let libName = ProcessInfo.processInfo.environment["EXACT_LIB"] ?? "caltrain_apple"

let package = Package(
    name: "ExactMac",
    platforms: [.macOS(.v14)],
    targets: [
        .systemLibrary(name: "CExact", path: "Sources/CExact"),
        .executableTarget(
            name: "ExactMac",
            dependencies: ["CExact"],
            path: "Sources/ExactMac",
            linkerSettings: [.unsafeFlags(["-L", libDir]), .linkedLibrary(libName), .linkedLibrary("c++")]
        ),
    ]
)
