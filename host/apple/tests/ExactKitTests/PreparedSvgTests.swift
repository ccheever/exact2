import XCTest
@testable import ExactKit

/// A batch's SVG scene, prepared on the owner (LLP 1072 stage 4), draws as
/// the raw one does: a shape whose path changes is redrawn, and one that
/// did not change is left alone (LLP 1055.000 D14).
class PreparedSvgTests: XCTestCase {
    private func scene(_ path: [Double], prepared: Bool) -> [String: Any] {
        let shape: [String: Any] = ["id": 1, "n": 7, "o": 1, "p": path.map { NSNumber(value: $0) }, "f": [255, 0, 0, 255].map { NSNumber(value: $0) }]
        var s: [String: Any] = ["box": [0, 0, 100, 100].map { NSNumber(value: $0) }, "t": [1, 0, 0, 1, 0, 0].map { NSNumber(value: $0) }, "els": [shape]]
        if prepared { SvgPrepare.scene(&s) }
        return s
    }

    private func shapePath(_ scene: SvgScene) -> CGPath? {
        (scene.root.sublayers?.first as? CAShapeLayer)?.path
    }

    func testAPreparedPathThatChangesIsRedrawn() {
        let svg = SvgScene()
        svg.apply(scene([0, 0, 0, 1, 10, 0, 1, 10, 10, 3], prepared: true), dark: false, clock: nil)
        let first = shapePath(svg)
        XCTAssertEqual(first?.boundingBox, CGRect(x: 0, y: 0, width: 10, height: 10))
        svg.apply(scene([0, 0, 0, 1, 40, 0, 1, 40, 30, 3], prepared: true), dark: false, clock: nil)
        XCTAssertEqual(shapePath(svg)?.boundingBox, CGRect(x: 0, y: 0, width: 40, height: 30))
    }

    func testAPreparedPathThatDidNotChangeIsLeftAlone() {
        let svg = SvgScene()
        let path: [Double] = [0, 0, 0, 1, 10, 0, 1, 10, 10, 3]
        svg.apply(scene(path, prepared: true), dark: false, clock: nil)
        let first = shapePath(svg)
        svg.apply(scene(path, prepared: true), dark: false, clock: nil)
        XCTAssertTrue(shapePath(svg) === first)
    }

    func testAPreparedPathIsTheRawOne() {
        let path: [Double] = [0, 2, 3, 2, 5, 7, 11, 13, 17, 19, 23, 1, 4, 9, 3]
        let raw = SvgScene(), prepared = SvgScene()
        raw.apply(scene(path, prepared: false), dark: false, clock: nil)
        prepared.apply(scene(path, prepared: true), dark: false, clock: nil)
        XCTAssertEqual(shapePath(raw), shapePath(prepared))
    }
}
