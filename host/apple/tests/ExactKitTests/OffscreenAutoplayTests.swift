import XCTest
@testable import ExactKit

/// Chrome 154's rule for a muted video its autoplay attribute started (LLP 1042 §3).
final class OffscreenAutoplayTests: XCTestCase {
    func testPlaysOnlyWhileVisibleAndStartsWhenFirstSeen() {
        var rule = OffscreenAutoplay()
        rule.load(autoplay: true)
        XCTAssertEqual(rule.visible(false), true, "loaded off screen: held before it starts")
        XCTAssertNil(rule.visible(false))
        XCTAssertEqual(rule.visible(true), false, "first seen: starts")
        rule.observed(paused: false)
        XCTAssertEqual(rule.visible(false), true)
        rule.observed(paused: true)
        XCTAssertEqual(rule.visible(true), false, "back in view: resumes")
        rule.observed(paused: false)
        XCTAssertTrue(rule.armed)
    }

    func testAPauseOrPlayTheRuleDidNotAskForEndsIt() {
        var rule = OffscreenAutoplay()
        rule.load(autoplay: true)
        XCTAssertNil(rule.visible(true))
        rule.observed(paused: true) // the native controls' pause
        XCTAssertFalse(rule.armed)
        XCTAssertNil(rule.visible(false), "a paused-by-hand video is not the rule's")

        rule.load(autoplay: true)
        XCTAssertEqual(rule.visible(false), true)
        rule.observed(paused: true)
        rule.observed(paused: false) // played by hand while held
        XCTAssertFalse(rule.armed)
        XCTAssertFalse(rule.holding)
    }

    func testALatePauseFromAnEarlierHoldIsNotTheUsers() {
        var rule = OffscreenAutoplay()
        rule.load(autoplay: true)
        XCTAssertEqual(rule.visible(false), true)
        XCTAssertEqual(rule.visible(true), false)
        rule.observed(paused: true) // the hold's pause, delivered after the resume was asked
        rule.observed(paused: false)
        XCTAssertTrue(rule.armed)
    }

    func testWithoutAutoplayNothingApplies() {
        var rule = OffscreenAutoplay()
        rule.load(autoplay: false)
        XCTAssertNil(rule.visible(false))
        rule.disarm()
        XCTAssertFalse(rule.armed)
    }
}
