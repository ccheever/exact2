import CoreGraphics
import XCTest
@testable import ExactKit

/// The border box a drag of a textarea's `resize` grip asks for
/// (`ResizeGripShape.dragged`): the corner follows the pointer from the box's
/// top left, under `rtl` the width grows by the pointer's move to the left,
/// and neither axis goes below zero.
final class ResizeGripTests: XCTestCase {
    private let both = (width: true, height: true)

    func testTheLeftToRightCornerFollowsThePointer() throws {
        let size = try XCTUnwrap(ResizeGripShape.dragged(axes: both, rtl: false, pointer: CGPoint(x: 330, y: 80),
            start: CGPoint(x: 290, y: 50), size: CGSize(width: 300, height: 60), movedX: 40))
        XCTAssertEqual(size.width, 340)
        XCTAssertEqual(size.height, 90)
    }

    func testARightToLeftWidthGrowsByTheMoveLeftWhereverThePointerIsInTheBox() throws {
        // The box's left edge moved with the pointer, so the pointer is still
        // at the corner in the box's own space: only `movedX` says how far.
        for x: CGFloat in [5, -35] {
            let size = try XCTUnwrap(ResizeGripShape.dragged(axes: both, rtl: true, pointer: CGPoint(x: x, y: 50),
                start: CGPoint(x: 5, y: 50), size: CGSize(width: 300, height: 60), movedX: -40))
            XCTAssertEqual(size.width, 340, "pointer at x \(x)")
            XCTAssertEqual(size.height, 60)
        }
        let narrower = try XCTUnwrap(ResizeGripShape.dragged(axes: both, rtl: true, pointer: CGPoint(x: 25, y: 50),
            start: CGPoint(x: 5, y: 50), size: CGSize(width: 300, height: 60), movedX: 20))
        XCTAssertEqual(narrower.width, 280)
    }

    func testAVerticalGripKeepsTheWidthAndAHorizontalOneTheHeight() throws {
        let vertical = try XCTUnwrap(ResizeGripShape.dragged(axes: (width: false, height: true), rtl: false,
            pointer: CGPoint(x: 400, y: 110), start: CGPoint(x: 290, y: 50), size: CGSize(width: 300, height: 60), movedX: 110))
        XCTAssertNil(vertical.width)
        XCTAssertEqual(vertical.height, 120)
        let horizontal = try XCTUnwrap(ResizeGripShape.dragged(axes: (width: true, height: false), rtl: false,
            pointer: CGPoint(x: 250, y: 110), start: CGPoint(x: 290, y: 50), size: CGSize(width: 300, height: 60), movedX: -40))
        XCTAssertEqual(horizontal.width, 260)
        XCTAssertNil(horizontal.height)
    }

    func testNeitherAxisGoesBelowZero() throws {
        let ltr = try XCTUnwrap(ResizeGripShape.dragged(axes: both, rtl: false, pointer: CGPoint(x: -500, y: -500),
            start: CGPoint(x: 290, y: 50), size: CGSize(width: 300, height: 60), movedX: -790))
        XCTAssertEqual(ltr.width, 0)
        XCTAssertEqual(ltr.height, 0)
        let rtl = try XCTUnwrap(ResizeGripShape.dragged(axes: both, rtl: true, pointer: CGPoint(x: 800, y: 50),
            start: CGPoint(x: 5, y: 50), size: CGSize(width: 300, height: 60), movedX: 795))
        XCTAssertEqual(rtl.width, 0)
    }

    func testNoAxisAsksForNothing() {
        XCTAssertNil(ResizeGripShape.dragged(axes: (width: false, height: false), rtl: false, pointer: CGPoint(x: 330, y: 80),
            start: CGPoint(x: 290, y: 50), size: CGSize(width: 300, height: 60), movedX: 40))
    }
}
