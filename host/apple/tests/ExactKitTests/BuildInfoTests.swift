import XCTest
@testable import ExactKit

/// The development menu's build lines, from the keys build.mjs stamps.
class BuildInfoTests: XCTestCase {
    func testTheStampedBuildReadsAsLines() {
        let now = ISO8601DateFormatter().date(from: "2026-10-08T12:00:00Z")!
        let lines = BuildInfo.lines([
            "CFBundleDisplayName": "Bluesky Exact2", "CFBundleIdentifier": "dev.tuft.blueskyclone",
            "CFBundleShortVersionString": "0.1.0", "CFBundleVersion": "12",
            "ExactBuildTime": "2026-10-08T10:00:00.123Z", "ExactBuildKind": "archive",
            "ExactCommit": "3adf67106ead52aa2ee5221151013b06c09fd238", "ExactCommitDirty": true,
            "ExactAppCommit": "575c6e8aa", "ExactAppCommitDirty": false,
        ], now: now)
        XCTAssertEqual(lines[0], "Bluesky Exact2 — dev.tuft.blueskyclone")
        XCTAssertEqual(lines[1], "version 0.1.0 (12)")
        XCTAssertTrue(lines[2].hasPrefix("built 2026-10-08 ") && lines[2].hasSuffix("(2 h ago)"), lines[2])
        XCTAssertEqual(Array(lines[3...]), ["exact2 3adf67106e (dirty)", "app 575c6e8aa", "archive"])
    }

    func testAnUnstampedBuildShowsWhatItHas() {
        XCTAssertEqual(BuildInfo.lines(["CFBundleName": "X", "CFBundleIdentifier": "x"]), ["X — x", "version ? (?)"])
        let now = Date()
        XCTAssertEqual(BuildInfo.age(of: now.addingTimeInterval(-30), now: now), "just now")
        XCTAssertEqual(BuildInfo.age(of: now.addingTimeInterval(-300), now: now), "5 min ago")
        XCTAssertEqual(BuildInfo.age(of: now.addingTimeInterval(-3 * 86400), now: now), "3 d ago")
    }
}
