// swift-tools-version:5.9
// One package for the Apple hosts (LLP 1031 D6): `ExactKit` — the session,
// the view, the app owner, the presenters, text, canvases, web views, the
// agent, the dev connection — over the C ABI in include/exact.h, with the
// AppKit and UIKit halves under `#if os(...)`; the two standalone apps
// (`ExactMac`, `ExactIOS`) and the two sample hosts (`ExactHostMac`,
// `ExactHostIOS`) as executables over it. Links the app's static library
// (`node host/apple/build.mjs` builds it and points EXACT_LIB_DIR/EXACT_LIB
// at it; the default is Caltrain's macOS archive).
import PackageDescription
import Foundation

let libDir = ProcessInfo.processInfo.environment["EXACT_LIB_DIR"] ?? (Context.packageDirectory + "/../../target/release")
let libName = ProcessInfo.processInfo.environment["EXACT_LIB"] ?? "caltrain_apple"

let composition = ProcessInfo.processInfo.environment["EXACT_APP_COMPOSITION"] ?? "embedded"
precondition(["embedded", "updating"].contains(composition), "EXACT_APP_COMPOSITION must be embedded or updating")

// `swift test` builds every target a package declares, and the two UIKit
// executables cannot build for macOS. EXACT_TESTS=1 narrows the package to
// what the tests need — the same environment-driven shape the composition
// above already uses — so a normal build is unchanged and a test build is
// quick. `node host/apple/build.mjs --test` sets it.
let testing = ProcessInfo.processInfo.environment["EXACT_TESTS"] == "1"

let core: [Target] = [
    .systemLibrary(name: "CExact", path: "Sources/CExact"),
    .target(
        name: "ExactKit",
        dependencies: ["CExact"],
        path: "Sources/ExactKit",
        linkerSettings: [.unsafeFlags(["-L", libDir]), .linkedLibrary(libName), .linkedLibrary("c++")]
    ),
    .target(name: "ExactUpdates", dependencies: ["ExactKit", "CExact"], path: "Sources/ExactUpdates"),
    .target(name: "ExactComposition", dependencies: [.target(name: "ExactKit")] + (composition == "updating" ? [.target(name: "ExactUpdates")] : []),
            path: composition == "updating" ? "Sources/ExactUpdating" : "Sources/ExactEmbedded"),
]

let executables: [Target] = [
    .executableTarget(name: "ExactMac", dependencies: ["ExactKit", "ExactComposition"], path: "Sources/ExactMac"),
    .executableTarget(name: "ExactIOS", dependencies: ["ExactKit", "ExactComposition"], path: "Sources/ExactIOS"),
    .executableTarget(name: "ExactHostMac", dependencies: ["ExactKit", "ExactComposition"], path: "Sources/ExactHostMac"),
    .executableTarget(name: "ExactHostIOS", dependencies: ["ExactKit", "ExactComposition"], path: "Sources/ExactHostIOS"),
]

// Host behaviour no other check can see: a wheel event's routing, asserted
// as a decision rather than a drawn frame, so it needs no window, no run
// loop, and no clock (LLP 1033 D4a).
let tests: [Target] = [
    .testTarget(name: "ExactKitTests", dependencies: ["ExactKit"], path: "Tests/ExactKitTests"),
]

let package = Package(
    name: "Exact",
    platforms: [.macOS(.v14), .iOS(.v17)],
    products: testing ? [.library(name: "ExactKit", targets: ["ExactKit"])] : [
        .library(name: "ExactKit", targets: ["ExactKit"]),
        .library(name: "ExactUpdates", targets: ["ExactUpdates"]),
        .executable(name: "ExactMac", targets: ["ExactMac"]),
        .executable(name: "ExactIOS", targets: ["ExactIOS"]),
        .executable(name: "ExactHostMac", targets: ["ExactHostMac"]),
        .executable(name: "ExactHostIOS", targets: ["ExactHostIOS"]),
    ],
    targets: core + (testing ? tests : executables)
)
