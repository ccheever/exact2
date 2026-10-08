// swift-tools-version:5.9
// One package for the Apple hosts (LLP 1031 D6): `ExactKit` — the session,
// the view, the app owner, the presenters, text, canvases, web views, the
// agent, the dev connection — over the C ABI in include/exact.h, with the
// AppKit and UIKit halves under `#if os(...)`; the two standalone apps
// (`ExactMac`, `ExactIOS`) and the two sample hosts (`ExactHostMac`,
// `ExactHostIOS`) as executables over it. Links the app's static library
// (`bun host/apple/build.mjs` builds it and points EXACT_LIB_DIR/EXACT_LIB
// at it; the default is Caltrain's macOS archive).
import PackageDescription
import Foundation

let libDir = ProcessInfo.processInfo.environment["EXACT_LIB_DIR"] ?? (Context.packageDirectory + "/../../target/host-dev")
let libName = ProcessInfo.processInfo.environment["EXACT_LIB"] ?? "caltrain_apple"

let composition = ProcessInfo.processInfo.environment["EXACT_APP_COMPOSITION"] ?? "embedded"
precondition(["embedded", "updating"].contains(composition), "EXACT_APP_COMPOSITION must be embedded or updating")

// The linked capabilities with a Swift half (LLP 1047.001 D4): each is its
// own target, which the composition depends on, and installs, only when the
// app links it. `build.mjs` names the app's (`EXACT_APPLE_LINK`, the
// capabilities' names, comma-separated, or `all`); a build it doesn't drive
// links them all, as development does.
let linkNames = ProcessInfo.processInfo.environment["EXACT_APPLE_LINK"] ?? "all"
func links(_ capability: String) -> Bool {
    linkNames == "all" || linkNames.split(separator: ",").contains { $0 == Substring(capability) }
}
let capabilities: [(name: String, target: String, define: String)] = [
    ("grouped_lists", "ExactGroupedLists", "EXACT_LINK_GROUPED_LISTS"),
    ("markdown", "ExactMarkdown", "EXACT_LINK_MARKDOWN"),
    ("surfaces", "ExactSurfaces", "EXACT_LINK_SURFACES"),
    ("drag", "ExactDrag", "EXACT_LINK_DRAG"),
].filter { links($0.name) }

// `swift test` builds every target a package declares, and the two UIKit
// executables cannot build for macOS. EXACT_TESTS=1 narrows the package to
// what the tests need — the same environment-driven shape the composition
// above already uses — so a normal build is unchanged and a test build is
// quick. `bun host/apple/build.mjs --test` sets it.
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
    .target(name: "ExactGroupedLists", dependencies: ["ExactKit", "CExact"], path: "Sources/ExactGroupedLists"),
    .target(name: "ExactMarkdown", dependencies: ["ExactKit", "CExact"], path: "Sources/ExactMarkdown"),
    .target(name: "ExactSurfaces", dependencies: ["ExactKit", "CExact"], path: "Sources/ExactSurfaces"),
    .target(name: "ExactDrag", dependencies: ["ExactKit", "CExact"], path: "Sources/ExactDrag"),
    .target(name: "ExactComposition", dependencies: [.target(name: "ExactKit")] + (composition == "updating" ? [.target(name: "ExactUpdates")] : [])
                + capabilities.map { .target(name: $0.target) },
            path: composition == "updating" ? "Sources/ExactUpdating" : "Sources/ExactEmbedded",
            swiftSettings: capabilities.map { .define($0.define) }),
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
    // The sound arm's C mixer (LLP 1096 D8), rendered offline by the tests;
    // the arm itself is a dylib build.mjs makes, never a package product.
    .target(name: "ExactSoundRender", path: "soundarm", exclude: ["SoundArm.swift"], sources: ["sound_render.c"], publicHeadersPath: "."),
    // The video arm's media session coordinator (LLP 1098 D7), driven by the
    // tests over stand-in players; the arm itself is build.mjs's dylib.
    .target(name: "ExactNowPlaying", path: "videoarm", exclude: ["VideoArm.swift"], sources: ["NowPlaying.swift"]),
    .testTarget(name: "ExactKitTests", dependencies: ["ExactKit", "ExactGroupedLists", "ExactMarkdown", "ExactSurfaces", "ExactDrag", "ExactSoundRender", "ExactNowPlaying"], path: "Tests/ExactKitTests"),
]

let package = Package(
    name: "Exact",
    platforms: [.macOS(.v14), .iOS(.v17), .tvOS(.v17)],
    products: testing ? [.library(name: "ExactKit", targets: ["ExactKit"])] : [
        .library(name: "ExactKit", targets: ["ExactKit"]),
        .library(name: "ExactUpdates", targets: ["ExactUpdates"]),
        .library(name: "ExactGroupedLists", targets: ["ExactGroupedLists"]),
        .library(name: "ExactMarkdown", targets: ["ExactMarkdown"]),
        .library(name: "ExactSurfaces", targets: ["ExactSurfaces"]),
        .library(name: "ExactDrag", targets: ["ExactDrag"]),
        .executable(name: "ExactMac", targets: ["ExactMac"]),
        .executable(name: "ExactIOS", targets: ["ExactIOS"]),
        .executable(name: "ExactHostMac", targets: ["ExactHostMac"]),
        .executable(name: "ExactHostIOS", targets: ["ExactHostIOS"]),
    ],
    targets: core + (testing ? tests : executables)
)
