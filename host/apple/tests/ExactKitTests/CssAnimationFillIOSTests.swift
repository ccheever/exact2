#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A CSS animation that has ended affects its property only with a forwards
/// fill, as CSS has it. Held on the agent's clock, an ended entrance with no
/// fill kept its last frame, so a later change of the property (an opacity
/// transition to 0) never showed (Signal Clone's reaction picker).
final class CssAnimationFillIOSTests: XCTestCase {
    private func spec(fill: Double, held: Double) -> [String: Any] {
        ["id": "fade#0#opacity", "k": "opacity", "s": 0.0, "dl": 0.0, "d": 0.2, "n": 1.0,
         "t": [0.0, 1.0], "v": [0.0, 1.0], "c": [], "fill": fill, "h": held]
    }

    func testAnEndedAnimationWithoutAForwardsFillHasNoEffect() {
        let layer = CALayer()
        for fill in [0.0, 2.0] {
            XCTAssertNil(CssAnimations.make(spec(fill: fill, held: 0.5), layer: layer, clock: nil), "fill \(fill), ended: no effect")
            XCTAssertNotNil(CssAnimations.make(spec(fill: fill, held: 0.1), layer: layer, clock: nil), "fill \(fill), running")
        }
        for fill in [1.0, 3.0] {
            let held = CssAnimations.make(spec(fill: fill, held: 0.5), layer: layer, clock: nil)
            XCTAssertNotNil(held, "fill \(fill), ended: holds the last frame")
            XCTAssertEqual(held?.timeOffset ?? 0, 0.2, accuracy: 1e-3)
        }
    }

    /// On a view: once the entrance has ended, the model opacity shows.
    func testTheModelValueShowsOnceAnEntranceHasEnded() {
        let layer = CALayer()
        layer.opacity = 0
        var installed: [String: String] = [:]
        CssAnimations.apply([spec(fill: 0, held: 0.1)], to: layer, clock: 100, installed: &installed)
        XCTAssertNotNil(layer.animation(forKey: "fade#0#opacity"), "running at 100 ms")
        CssAnimations.apply([spec(fill: 0, held: 0.5)], to: layer, clock: 500, installed: &installed)
        XCTAssertNil(layer.animation(forKey: "fade#0#opacity"), "ended at 500 ms: removed, the model value (0) shows")
        // The agent's clock alone (no `h`): to the end exactly, and back.
        var plain = spec(fill: 0, held: 0); plain.removeValue(forKey: "h")
        var clocked: [String: String] = [:]
        CssAnimations.apply([plain], to: layer, clock: 200, installed: &clocked)
        XCTAssertNil(layer.animation(forKey: "fade#0#opacity"), "at its end, 200 ms: no effect")
        CssAnimations.apply([plain], to: layer, clock: 100, installed: &clocked)
        XCTAssertNotNil(layer.animation(forKey: "fade#0#opacity"), "seeked back into it: installed again")
    }
}
#endif
