import CoreGraphics
import QuartzCore
import XCTest
@testable import ExactKit

/// LLP 1066: `background-image` gradients as the node view paints them, held
/// to Chrome's pictures of the parity page (`scripts/fixtures/gradients.contract`,
/// the web host at 1×) within the border parity's band (`BorderParity`). The
/// styles are what the host's Rust side sends for the page (`style.rs`
/// gradient_json): the shape, and stops ready to mix.
enum GradientParity {
    private static func g(_ shape: (String, BatchValue), _ stops: [Double], dark: [Double]? = nil) -> BatchValue {
        var o: [String: BatchValue] = [shape.0: shape.1, "stops": .array(stops.map { .number($0) })]
        if let dark { o["dark"] = .array(dark.map { .number($0) }) }
        return .object(o)
    }
    private static func n(_ v: [Double]) -> BatchValue { .array(v.map { .number($0) }) }
    private static func linear(_ deg: Double) -> (String, BatchValue) { ("linear", .number(deg)) }
    private static func radial(_ v: [Double]) -> (String, BatchValue) { ("radial", n(v)) }

    static func cases() -> [BorderParity.Case] {
        let styles: [(String, NodeStyle)] = [
            ("linear", ["background_image": g(linear(180), [0, 229, 57, 53, 255, 1, 30, 136, 229, 255])]),
            ("angle", ["background_image": g(linear(30), [0, 253, 216, 53, 255, 0.6, 67, 160, 71, 255, 1, 30, 136, 229, 255])]),
            ("corner", ["background_image": g(("corner", n([1, 0])), [0, 229, 57, 53, 255, 0.5, 253, 216, 53, 255, 1, 30, 136, 229, 255])]),
            ("fade", ["background_image": g(linear(180), [0, 30, 136, 229, 0, 1, 30, 136, 229, 255])]),
            ("over", ["background_color": n([253, 216, 53, 255]), "background_image": g(linear(90), [0, 229, 57, 53, 0, 1, 229, 57, 53, 204])]),
            ("clip", ["clip_path": .object(["rule": "nonzero", "commands": .array([.array([.string("M"), n([50, 0])]), .array([.string("L"), n([100, 70])]), .array([.string("L"), n([0, 70])]), .array([.string("Z"), n([])])])]),
                      "background_image": g(linear(90), [0, 229, 57, 53, 255, 0.5, 229, 57, 53, 255, 0.5, 30, 136, 229, 255, 1, 30, 136, 229, 255])]),
            ("radial", ["background_image": g(radial([0, 3, 50, 0, 50, 0]), [0, 253, 216, 53, 255, 1, 229, 57, 53, 255])]),
            ("circle", ["background_image": g(radial([1, 0, 30, 0, 40, 0]), [0, 255, 255, 255, 255, 0.8, 30, 136, 229, 255, 1, 0, 0, 0, 255])]),
            ("ellipse", ["background_image": g(radial([0, 1, 0, 70, 0, 20]), [0, 67, 160, 71, 255, 1, 67, 160, 71, 0])]),
            ("rounded", ["border_radius_top_left": 24, "border_radius_top_right": 24, "border_radius_bottom_right": 24, "border_radius_bottom_left": 24,
                         "background_image": g(linear(90), [0, 67, 160, 71, 255, 1, 30, 136, 229, 255])]),
            ("bordered", {
                var s: NodeStyle = ["background_image": g(radial([1, 3, 0, 0, 0, 0]), [0, 253, 216, 53, 255, 1, 229, 57, 53, 255])]
                for side in ["top", "right", "bottom", "left"] { s["border_width_" + side] = 8; s["border_color_" + side] = n([0, 0, 0, 255]) }
                for corner in ["top_left", "top_right", "bottom_right", "bottom_left"] { s["border_radius_" + corner] = 16 }
                return s
            }()),
            ("scheme", ["border_radius_top_left": 20, "border_radius_top_right": 20, "border_radius_bottom_right": 20, "border_radius_bottom_left": 20,
                        "background_image": g(linear(180), [0, 229, 57, 53, 255, 1, 253, 216, 53, 255], dark: [0, 128, 222, 234, 255, 1, 206, 147, 216, 255])]),
            ("hatch", ["background_color": n([252, 252, 252, 255]), "background_image": ["css": "repeating-linear-gradient(-45deg, transparent 0 4.243px, #e6e6e6 4.243px 5.657px)"]]),
            ("length", ["background_color": n([252, 252, 252, 255]), "background_image": ["css": "linear-gradient(-45deg, transparent 0px 4.243px, #e6e6e6 4.243px 5.657px)"]]),
            ("mixed", ["background_image": ["css": "linear-gradient(to right, #e53935 20px, #fdd835, #1e88e5 80%)"]]),
            ("zero-two", ["background_image": ["css": "repeating-linear-gradient(to right, red 0px, blue 0px)"]]),
            ("zero-three", ["background_image": ["css": "repeating-linear-gradient(to right, red 0px, lime 0px, blue 0px)"]]),
            ("zero-four", ["background_image": ["css": "repeating-linear-gradient(to right, red 0px, lime 0px, blue 0px, white 0px)"]]),
        ]
        return styles.enumerated().map { i, s in BorderParity.Case(name: s.0, x: [20, 145, 270][i % 3], y: [20, 115, 210, 305, 400, 495][i / 3], style: s.1) }
    }

    static func checkLengthResize() {
        let gradient = Gradient(["css": "linear-gradient(to right, red 20px, green, blue 80%)"] as BatchValue)!
        let layer = CAGradientLayer()
        for (width, first) in [(100.0, 0.2), (200.0, 0.1)] {
            let box = CGRect(x: 0, y: 0, width: width, height: 70)
            gradient.apply(layer, bounds: box, box: box, dark: false)
            XCTAssertEqual(layer.locations?[1].doubleValue ?? -1, first, accuracy: 0.0001)
        }
    }

    /// Every case against one of Chrome's pictures; the failures, named.
    static func check(dark: Bool, band: (mean: Double, over: Double) = (2, 2), render: (NodeStyle, Bool, CGColor) -> [UInt8]) -> [String] {
        let reference = BorderParity.chrome(dark ? "gradients.web-dark.png" : "gradients.web.png")
        let page = dark ? CGColor(srgbRed: 0x12 / 255, green: 0x12 / 255, blue: 0x12 / 255, alpha: 1) : CGColor(srgbRed: 1, green: 1, blue: 1, alpha: 1)
        var failures: [String] = []
        for c in cases() {
            let (mean, over) = BorderParity.compare(render(c.style, dark, page), reference, at: c.x, c.y)
            print("gradient parity \(dark ? "dark" : "light") \(c.name): mean \(String(format: "%.2f", mean))/255, \(String(format: "%.2f", over))% beyond 48")
            if mean > band.mean || over > band.over { failures.append("\(c.name) \(dark ? "dark" : "light"): mean \(mean), \(over)% beyond 48") }
        }
        return failures
    }
}
