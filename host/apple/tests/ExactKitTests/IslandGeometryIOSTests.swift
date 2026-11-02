#if os(iOS) && EXACT_WIDGET_GALLERY
import XCTest
import SwiftUI
@testable import ExactKit

final class IslandGeometryIOSTests: XCTestCase {
    func testExpandedSidesKeepTheCameraClearWhenTheCenterIsNarrow() {
        let geometry = IslandGeometry.iPhone17Pro
        for centerWidth in [CGFloat(0), 30, 180] {
            let frames = ExpandedArrangement.allocate(inner: 327, geometry: geometry,
                leading: CGSize(width: 140, height: 40), trailing: CGSize(width: 40, height: 40),
                center: CGSize(width: centerWidth, height: 20), bottom: CGSize(width: 327, height: 20))
            let side = (327 - geometry.obstruction.width) / 2
            XCTAssertLessThanOrEqual(frames.leading.maxX, side)
            XCTAssertGreaterThanOrEqual(frames.trailing.minX, 327 - side)
            XCTAssertEqual(frames.leading.width, frames.trailing.width)
            XCTAssertGreaterThanOrEqual(frames.bottom.minY, frames.center.maxY)
        }
    }
}
#endif
