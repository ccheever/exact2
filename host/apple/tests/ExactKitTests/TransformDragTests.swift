import XCTest
import CoreGraphics
@testable import ExactKit

final class TransformDragTests: XCTestCase {
    func testPacketMatchesFrozen120ByteOffsetsAndPreservesHighIdentities() {
        let packet = TransformDragPacket(op: 13, runtime: UInt64.max - 1,
            handleKey: 9_007_199_254_740_993, targetKey: 0xffff_ffff_0000_0001,
            clipKey: 0xffff_ffff_0000_0002, sequence: 9_007_199_254_740_995,
            translateToken: 0xffff_ffff_0000_0003, scaleToken: 0xffff_ffff_0000_0004,
            values: [12.25, -7.5, 1.375, 90, -45, -0.25], now: 1234.5)
        let bytes = packet.encoded()!
        XCTAssertEqual(bytes.count, 120)
        func word(_ at: Int, _ count: Int) -> UInt64 {
            (0..<count).reduce(0) { $0 | UInt64(bytes[at + $1]) << ($1 * 8) }
        }
        XCTAssertEqual(word(0, 4), 2); XCTAssertEqual(word(4, 4), 13)
        XCTAssertEqual(word(8, 8), UInt64.max - 1)
        XCTAssertEqual(word(16, 8), 9_007_199_254_740_993)
        XCTAssertEqual(word(24, 8), 0xffff_ffff_0000_0001)
        XCTAssertEqual(word(32, 8), 0xffff_ffff_0000_0002)
        XCTAssertEqual(word(40, 8), 9_007_199_254_740_995)
        XCTAssertEqual(word(48, 8), 0xffff_ffff_0000_0003)
        XCTAssertEqual(word(56, 8), 0xffff_ffff_0000_0004)
        for (index, value) in packet.values.enumerated() {
            XCTAssertEqual(Double(bitPattern: word(64 + index * 8, 8)), value)
        }
        XCTAssertEqual(Double(bitPattern: word(112, 8)), 1234.5)
    }

    func testCaughtPairKeepsZeroDisplacementAndDoesNotDividePanByScale() {
        let origin = TransformDragPosition(x: 112.5, y: -35.25, scale: 2)!
        XCTAssertEqual(origin.moved(dx: 0, dy: 0), [112.5, -35.25, 2])
        XCTAssertEqual(origin.moved(dx: -60, dy: 20), [52.5, -15.25, 2])
        XCTAssertEqual(origin.moved(dx: -140, dy: 0), [-27.5, -35.25, 2])
        XCTAssertNil(origin.moved(dx: .nan, dy: 0))
    }

    func testPhotoPositionAdmissionIsSeparateFromSignedTerminalVelocity() {
        for scale in [0, -1, 1e-100, Double.infinity, Double.nan, 1e39] {
            XCTAssertNil(TransformDragPosition(x: 0, y: 0, scale: scale))
        }
        for value in [Double.infinity, Double.nan, 1e39, -1e39] {
            XCTAssertNil(TransformDragPosition(x: value, y: 0, scale: 1))
            XCTAssertNil(TransformDragPosition(x: 0, y: value, scale: 1))
        }
        XCTAssertNotNil(TransformDragPosition(x: -1, y: 1, scale: Double(Float.leastNonzeroMagnitude)))
        // A pinch keeps the content under its focal point there (LLP 1057.001 §4).
        let origin = try! XCTUnwrap(TransformDragPosition(x: 10, y: -20, scale: 2))
        let still = try! XCTUnwrap(origin.focused(from: CGPoint(x: 50, y: 30), to: CGPoint(x: 50, y: 30), factor: 1.5))
        XCTAssertEqual(still.values, [-10, -45, 3], "content 20,25 under the focus stays at 50,30")
        XCTAssertEqual((50 - still.x) / still.scale, (50 - origin.x) / origin.scale, accuracy: 1e-12)
        let panned = try! XCTUnwrap(origin.focused(from: CGPoint(x: 5, y: 5), to: CGPoint(x: 25, y: -5), factor: 1))
        XCTAssertEqual(panned.values, [30, -30, 2], "factor 1 is a pan by the centroid's travel")
        XCTAssertNil(origin.focused(from: .zero, to: .zero, factor: 0))
        XCTAssertNil(origin.focused(from: .zero, to: .zero, factor: .nan))
    }

    func testBindingKeepsAllGenerationsAndRequiresCoherentUnbind() {
        var op: [String: Any] = ["id": 11, "runtime": "18446744073709551614",
            "handleKey": "9007199254740993", "target": 12, "targetKey": "9007199254740994",
            "clip": 13, "clipKey": "9007199254740995"]
        let binding = TransformDragBinding(op)!
        XCTAssertEqual(binding.runtime, UInt64.max - 1)
        XCTAssertEqual(binding.handleKey, 9_007_199_254_740_993)
        XCTAssertEqual(binding.targetKey, 9_007_199_254_740_994)
        XCTAssertEqual(binding.clipKey, 9_007_199_254_740_995)
        op["target"] = NSNull(); XCTAssertNil(TransformDragBinding(op))
        for key in ["targetKey", "clip", "clipKey"] { op[key] = NSNull() }
        let retired = TransformDragBinding(op)!
        XCTAssertNil(retired.target); XCTAssertNil(retired.targetKey)
        XCTAssertNil(retired.clip); XCTAssertNil(retired.clipKey)
    }

    func testGeometryCertifiesActualCenteredFillAndPositiveReadiness() {
        let bounds = CGRect(x: 0, y: 0, width: 320.25, height: 200.5)
        XCTAssertNotNil(TransformGeometryFacts(targetBounds: bounds, targetFrame: bounds,
            clipBounds: bounds, windowOrigin: CGPoint(x: 41.75, y: 18.5), supportedAncestors: true))
        XCTAssertNil(TransformGeometryFacts(targetBounds: bounds, targetFrame: bounds.offsetBy(dx: 1, dy: 0),
            clipBounds: bounds, windowOrigin: .zero, supportedAncestors: true))
        XCTAssertNil(TransformGeometryFacts(targetBounds: bounds, targetFrame: bounds,
            clipBounds: bounds, windowOrigin: .zero, supportedAncestors: false))
        let zero = CGRect.zero
        let suspended = TransformGeometryFacts(targetBounds: zero, targetFrame: zero,
            clipBounds: zero, windowOrigin: .zero, supportedAncestors: true)!
        XCTAssertFalse(suspended.ready)
        let moved = TransformGeometryFacts(targetBounds: bounds, targetFrame: bounds,
            clipBounds: bounds, windowOrigin: CGPoint(x: 41.75, y: 19.5), supportedAncestors: true)!
        let still = TransformGeometryFacts(targetBounds: bounds, targetFrame: bounds,
            clipBounds: bounds, windowOrigin: CGPoint(x: 41.75, y: 18.5), supportedAncestors: true)!
        XCTAssertEqual(moved.dimensions, still.dimensions)
        XCTAssertNotEqual(moved, still)
    }
}

extension TransformDragTests {
    func testCenteredActualMatrixCatchAndUnsupportedTransforms() {
        let center = CGPoint(x: 160, y: 100)
        let matrix = CGAffineTransform(translationX: 52.5, y: -21.25)
            .translatedBy(x: center.x, y: center.y).scaledBy(x: 2, y: 2)
            .translatedBy(x: -center.x, y: -center.y)
        XCTAssertEqual(TransformDragPosition(matrix: matrix, center: center)?.values, [52.5, -21.25, 2])
        XCTAssertNil(TransformDragPosition(matrix: matrix.rotated(by: 0.01), center: center))
        XCTAssertNil(TransformDragPosition(matrix: CGAffineTransform(scaleX: 1, y: 2), center: center))
        XCTAssertNil(TransformDragPosition(matrix: CGAffineTransform(scaleX: -1, y: -1), center: center))
        XCTAssertNil(TransformDragPosition(matrix: CGAffineTransform(scaleX: 0, y: 0), center: center))
    }
}
