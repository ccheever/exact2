import Foundation
import XCTest
@testable import ExactKit

// The build channel's host half (LLP 1122 D4): a pin read as the server
// sends it (scripts/tamber-builds.mjs `pinRefused`), the commands dispatched
// before generation handling, the process id the server accepts, and a live
// batch's design digest.
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
}
