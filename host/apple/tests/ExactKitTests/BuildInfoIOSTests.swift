import XCTest
@testable import ExactKit

/// The development menu's build lines, from the keys build.mjs stamps (no
/// UIKit: named for the iOS lane, it runs in the macOS suite too).
class BuildInfoIOSTests: XCTestCase {
    func testTheStampedBuildReadsAsLines() {
        let now = ISO8601DateFormatter().date(from: "2026-10-08T12:00:00Z")!
        let lines = BuildInfo.lines([
            "CFBundleDisplayName": "Bluesky Exact2", "CFBundleIdentifier": "dev.tuft.blueskyclone",
            "CFBundleShortVersionString": "0.1.0", "CFBundleVersion": "12",
            "ExactBuildTime": "2026-10-08T09:59:59.500Z", "ExactBuildKind": "archive",
            "ExactCommit": "3adf67106ead52aa2ee5221151013b06c09fd238", "ExactCommitDirty": true,
            "ExactAppCommit": "575c6e8aa", "ExactAppCommitDirty": false, "ExactAppBranch": "main",
            "ExactBuildHost": "studio", "ExactBuildXcode": "Xcode 26.0 Build version 17A100",
            "ExactDistributionRevision": "12", "ExactReleaseNotes": "- the lightbox opens from its first frame",
        ], device: "iPhone17,1 · iOS 27.0", now: now)
        XCTAssertEqual(lines[0], "Bluesky Exact2 — dev.tuft.blueskyclone")
        XCTAssertEqual(lines[1], "version 0.1.0 (12)")
        let built = ISO8601DateFormatter().date(from: "2026-10-08T09:59:59Z")!
        XCTAssertEqual(lines[2], "built \(BuildInfo.local.string(from: built)) (2 h ago) on studio", "local time, whatever the zone")
        XCTAssertEqual(Array(lines[3...]), ["Xcode 26.0 Build version 17A100", "exact2 3adf67106e (dirty)", "app 575c6e8aa on main",
                                            "archive", "revision 12", "iPhone17,1 · iOS 27.0"])
        let info: [String: Any] = ["ExactReleaseNotes": "- the lightbox opens from its first frame"]
        XCTAssertEqual(BuildInfo.text(["a", "b"], notes: BuildInfo.notes(info)), "a\nb\n\nRelease notes\n- the lightbox opens from its first frame")
        XCTAssertEqual(BuildInfo.text(["a"], notes: BuildInfo.notes([:])), "a", "no notes, no section")
    }

    func testAnUnstampedBuildShowsWhatItHas() {
        XCTAssertEqual(BuildInfo.lines(["CFBundleName": "X", "CFBundleIdentifier": "x"]), ["X — x", "version ? (?)"])
        let now = Date()
        XCTAssertEqual(BuildInfo.age(of: now.addingTimeInterval(-30), now: now), "just now")
        XCTAssertEqual(BuildInfo.age(of: now.addingTimeInterval(-300), now: now), "5 min ago")
        XCTAssertEqual(BuildInfo.age(of: now.addingTimeInterval(-3 * 86400), now: now), "3 d ago")
    }
}
