#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1031 D3: resources follow the containing view, including a display
/// scale change that leaves every logical point and the live runner intact.
final class DisplayIOSTests: XCTestCase {
    func testDisplayScaleChangesUpdateTheSameSessionsBudgetWithoutResizingIt() {
        let session = ExactApp.shared.makeSession(label: "display")
        defer { session.destroy() }
        let view = ExactView(session: session)
        view.frame = CGRect(x: 0, y: 0, width: 1200, height: 1000)
        view.traitOverrides.displayScale = 1
        view.updateTraitsIfNeeded()
        view.fit()
        XCTAssertTrue(session.booted, session.bootError ?? "did not boot")
        let runtime = session.runtime.rt, raster = session.rasters.id
        let points = session.viewportSize
        XCTAssertEqual(session.rasters.budget, 38_400_000)
        view.traitOverrides.displayScale = 3
        view.updateTraitsIfNeeded()
        view.fit()
        XCTAssertEqual(session.viewportSize, points)
        XCTAssertEqual(session.runtime.rt, runtime)
        XCTAssertEqual(session.rasters.id, raster)
        XCTAssertEqual(session.rasters.budget, 192 * 1024 * 1024)
        view.frame.size = CGSize(width: 402, height: 700)
        view.fit()
        XCTAssertEqual(session.viewportSize, CGSize(width: 402, height: 700))
        XCTAssertEqual(session.rasters.budget, 81_043_200)
        view.traitOverrides.displayScale = 1
        view.updateTraitsIfNeeded()
        view.fit()
        XCTAssertEqual(session.rasters.budget, 32 * 1024 * 1024)
        XCTAssertEqual(session.runtime.rt, runtime)
        XCTAssertEqual(session.rasters.id, raster)
    }
}
#endif
