#if os(macOS)
import AppKit
#endif
import Foundation
import XCTest
@testable import ExactKit

// The build channel's host half (LLP 1122 D4): a pin read as the server
// sends it (scripts/tamber-builds.mjs `pinRefused`), the commands dispatched
// before generation handling, the process id the server accepts, a live
// batch's design digest, a batch's moved facts, and the plan's own design
// after a boot.
final class BuildChannelTests: XCTestCase {
    private func parse(_ json: String) -> (pin: BuildPin?, refused: String?) {
        let frame = try! JSONSerialization.jsonObject(with: Data("{\"pin\":\(json)}".utf8)) as! [String: Any]
        return BuildPin.parse(frame["pin"])
    }

    func testPinsAsTheServerSendsThem() {
        XCTAssertNil(parse("null").pin)
        XCTAssertNil(parse("null").refused)
        XCTAssertNil(BuildPin.parse(nil).refused)
        XCTAssertEqual(parse("{}").pin, BuildPin())
        let all = parse(#"{"scheme":"dark","contrast":"more","mode":{"density":"compact"},"rootFontSize":20}"#)
        XCTAssertNil(all.refused)
        XCTAssertEqual(all.pin, BuildPin(scheme: "dark", contrast: "more", mode: ["density": "compact"], rootFontSize: 20))
        XCTAssertEqual(parse(#"{"rootFontSize":17.5}"#).pin?.rootFontSize, 17.5)
    }

    func testRefusesWhatTheServerWouldNot() {
        for bad in ["1", "[]", #""dark""#, #"{"theme":"x"}"#, #"{"scheme":"sepia"}"#, #"{"contrast":"custom"}"#,
                    #"{"mode":{"a":"b,c"}}"#, #"{"mode":{"a=b":"c"}}"#, #"{"mode":{"a":1}}"#, #"{"mode":"a=b"}"#,
                    #"{"rootFontSize":3}"#, #"{"rootFontSize":65}"#, #"{"rootFontSize":true}"#, #"{"rootFontSize":"16"}"#] {
            let parsed = parse(bad)
            XCTAssertNil(parsed.pin, bad)
            XCTAssertNotNil(parsed.refused, bad)
        }
    }

    func testCommandsAndId() {
        XCTAssertEqual(BuildChannel.commands, ["pin", "inspect", "outline", "hold"])
        XCTAssertFalse(BuildChannel.commands.contains("tamber-batch"))
        XCTAssertNotNil(BuildChannel.id.range(of: #"^(ios|macos)-[A-Za-z0-9_.:-]{1,57}$"#, options: .regularExpression), BuildChannel.id)
        XCTAssertEqual(BuildChannel.id, BuildChannel.id)
    }

    func testModesUnderAPinAreEachSessionsOwnBeneathIt() {
        let a = ["density": "compact", "theme": "ocean"], b = ["density": "roomy"]
        let pin = BuildPin(mode: ["theme": "forest"])
        XCTAssertEqual(BuildChannel.modes(before: a, pin: pin), ["density": "compact", "theme": "forest"])
        XCTAssertEqual(BuildChannel.modes(before: b, pin: pin), ["density": "roomy", "theme": "forest"])
        // Null, or a pin naming no mode, hands each its own back.
        XCTAssertEqual(BuildChannel.modes(before: a, pin: nil), a)
        XCTAssertEqual(BuildChannel.modes(before: b, pin: BuildPin(scheme: "dark")), b)
    }

    func testLiveBatchCarriesItsDesign() {
        let batch = Batch.decode(Data(#"{"live":"applied","unsupported":true,"design":"d1","ops":[]}"#.utf8))
        XCTAssertEqual(batch.live, "applied")
        XCTAssertTrue(batch.unsupported)
        XCTAssertEqual(batch.design, "d1")
        // Any other shape under the key is not a digest, and costs the batch nothing.
        let other = Batch.decode(Data(#"{"design":{"x":1},"ops":[]}"#.utf8))
        XCTAssertNil(other.design)
        XCTAssertNotEqual(other.error, "unreadable batch")
    }

    /// A batch whose root font size or modes moved says so, whoever moved
    /// them: the app's `setMode` reaches the host as no command.
    func testABatchSaysItsFactsMoved() {
        XCTAssertTrue(Batch.decode(Data(#"{"ops":[],"facts":true}"#.utf8)).facts)
        XCTAssertFalse(Batch.decode(Data(#"{"ops":[]}"#.utf8)).facts)
    }

    func testRegistrationKnowsTheRunningPlanBeforeFetchingAGeneration() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let app = ExactApp.shared
        let session = app.makeSession(label: "build-channel-plan")
        defer { app.disconnect(); session.destroy(); BuildChannel.design = nil }
        XCTAssertNil(session.runtime.planDigest())
        XCTAssertNil(session.boot(size: CGSize(width: 400, height: 300)).error)
        let tree = try XCTUnwrap(session.runtime.agent(#"{"op":"tree","plan":true}"#).data(using: .utf8))
        let digest = try XCTUnwrap((JSONSerialization.jsonObject(with: tree) as? [String: Any])?["planDigest"] as? String)
        XCTAssertEqual(digest.count, 64)
        // No run-loop turn or network response precedes either registration's identity read.
        for _ in 0..<2 {
            app.connect("http://127.0.0.1:1")
            XCTAssertEqual(app.connection?.plan, digest)
            app.disconnect()
        }
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_BUTTONS_PLAN"])
        let replacement = try Data(contentsOf: URL(fileURLWithPath: path))
        XCTAssertNil(session.boot(plan: replacement, size: CGSize(width: 400, height: 300)).error)
        let replacedTree = try XCTUnwrap(session.runtime.agent(#"{"op":"tree","plan":true}"#).data(using: .utf8))
        let replacedDigest = try XCTUnwrap((JSONSerialization.jsonObject(with: replacedTree) as? [String: Any])?["planDigest"] as? String)
        XCTAssertNotEqual(replacedDigest, digest)
        app.connect("http://127.0.0.1:1")
        XCTAssertEqual(app.connection?.plan, replacedDigest)
        XCTAssertFalse(session.apply(Data("invalid plan".utf8)))
        XCTAssertEqual(app.connection?.plan, replacedDigest, "a refused plan keeps the running identity")
    }

    func testRegistrationUsesTheAcceptedGenerationIdentity() throws {
        let connection = try XCTUnwrap(PlanURL.open("http://127.0.0.1:1", acceptProgram: { _ in true }, current: { "accepted-generation" }, plan: { "running-plan" }, apply: { _, _, _ in false }))
        defer { connection.close() }
        XCTAssertEqual(connection.plan, "accepted-generation")
    }

    #if os(macOS)
    /// A boot, as a generation, reports its plan's own design: the server's
    /// `/__dev/design` answers 204 when its design is the plan's, so no
    /// snapshot replaces a digest a live batch left from the plan before.
    func testABootReportsItsPlansDesign() {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "build-channel-design")
        defer { session.destroy(); BuildChannel.design = nil }
        XCTAssertNil(session.runtime.designDigest(), "nothing runs before boot")
        BuildChannel.design = "the last plan's"
        XCTAssertNil(session.boot(size: CGSize(width: 400, height: 300)).error)
        let digest = session.runtime.designDigest()
        XCTAssertNotNil(digest)
        XCTAssertNotEqual(digest, "")
        XCTAssertEqual(BuildChannel.design, digest)
    }
    #endif
}
