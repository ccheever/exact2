import IOSurface
import QuartzCore
import XCTest
@testable import ExactKit

/// A live filter picture under load (LLP 1055.000 D14): two pictures
/// updated from the main thread faster than their draws land, at sizes
/// that change every time, their draws encoded on the pictures' queue with
/// the Metal chain's shared working textures. Run it under the Thread
/// Sanitizer too (`swift test --sanitize=thread --filter SvgFilterLiveTests`).
class SvgFilterLiveTests: XCTestCase {
    func testManyUpdatesLandTheLatestPicture() throws {
        guard SvgFilterGPU.metal != nil, SvgFilterMetal.shared != nil else { throw XCTSkip("no Metal") }
        let sub: [Float] = [-10, -10, 80, 70]
        let saturate: [Float] = [1.6, -0.6, -0.1, 0, 0, -0.2, 1.4, -0.1, 0, 0, -0.2, -0.6, 1.9, 0, 0, 0, 0, 0, 1, 0]
        let program: [Float] = sub + [2] + [0, -1, -3] + sub + [0, 2, 2] + [5, 0, -3] + sub + [0] + saturate
        func els(_ x: Double) -> [Any] {
            [["id": 1, "o": 1, "n": 1, "p": [0, 0, 0, 1, 40 + x, 0, 1, 40 + x, 30, 1, 0, 30, 3], "f": [220, 40, 40, 255],
              "w": 0, "cap": 0, "join": 0, "ml": 4, "rule": 0, "dash": [Any](), "ph": 0] as [String: Any]]
        }
        let pictures = [SvgFilterLive(layer: CALayer()), SvgFilterLive(layer: CALayer())]
        for i in 0..<300 {
            let grow = CGFloat(i % 17)
            for (n, p) in pictures.enumerated() {
                let rect = CGRect(x: -10, y: -10, width: 80 + grow, height: 70 + grow / 2)
                p.update(els: els(Double(i % 9 + n)), rect: rect, w: Int((rect.width * 2).rounded(.up)), h: Int((rect.height * 2).rounded(.up)),
                         k: 2, program: program, dark: false, fonts: nil, clock: Double(i), now: i == 0)
            }
            if i % 7 == 0 { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.001)) }
        }
        let end = Date(timeIntervalSinceNow: 10)
        while SvgFilterLive.inFlight > 0 && Date() < end { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.005)) }
        XCTAssertEqual(SvgFilterLive.inFlight, 0)
        for p in pictures {
            XCTAssertNotNil(p.layer.contents, "a picture landed")
            XCTAssertTrue(CFGetTypeID(p.layer.contents as CFTypeRef) == IOSurfaceGetTypeID())
            XCTAssertEqual(p.layer.bounds.width, CGFloat(80 + 299 % 17), accuracy: 0.001, "the latest input's place")
        }
    }
}
