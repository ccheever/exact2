// `<photo-editor>` on Apple hosts (LLP 1024): the app's one native module.
// The edit — zoom, rotation, position and a crop rectangle — lives here, in
// the platform's own gesture recognizers (PhotoEditorIOS.swift: UIKit;
// PhotoEditorMac.swift: AppKit). What crosses to the Contract is small:
//
//   props   photo   the image, relative to the app's assets root
//           turns   quarter turns clockwise (the Rotate 90° button)
//           reset   a counter; each change resets zoom, rotation, position and crop
//   events  load      once the photo is up (and `change` with the first state)
//           change    the edit state, when a gesture ends or a prop moves it
//           message   "edited", beside each change after the first
import Foundation
import CoreGraphics
import QuartzCore
#if os(macOS)
import AppKit
#else
import UIKit
#endif

let exactNativeModules: [String: ExactNativeFactory] = [
    "photo-editor": ExactNativeFactory { props, events in try PhotoEditor(props: props, events: events) },
]

/// The edit, in the canvas's own terms: the photo fits the canvas at
/// `scale` 1, centred and moved by `offset` points, turned by `rotation`
/// radians (gestures) plus `turns` quarter turns; the crop is a unit
/// rectangle of the canvas.
struct PhotoEdit: Equatable {
    var scale: CGFloat = 1
    var rotation: CGFloat = 0
    var turns = 0
    var offset = CGPoint.zero
    var crop = CGRect(x: 0, y: 0, width: 1, height: 1)
    /// A quarter turn in flight: an animation's share of the angle.
    var turning: CGFloat = 0

    static let minScale: CGFloat = 0.5, maxScale: CGFloat = 8, minCrop: CGFloat = 0.12

    /// The whole turn, radians, clockwise on screen.
    var angle: CGFloat { rotation + CGFloat(turns) * .pi / 2 + turning }

    /// The photo (`image` pixels) fitted into `stage` at scale 1, turned by
    /// its quarter turns: a portrait turn of a landscape photo fills the
    /// stage as a portrait.
    func fit(_ image: CGSize, in stage: CGSize) -> CGSize {
        guard image.width > 0, image.height > 0, stage.width > 0, stage.height > 0 else { return stage }
        let q = CGFloat(turns) * .pi / 2 + turning, c = abs(cos(q)), s = abs(sin(q))
        let k = min(stage.width / (image.width * c + image.height * s), stage.height / (image.width * s + image.height * c))
        return CGSize(width: image.width * k, height: image.height * k)
    }

    /// Degrees in [0, 360).
    var degrees: Double {
        let d = Double(angle) * 180 / .pi
        let r = d.truncatingRemainder(dividingBy: 360)
        return (r < 0 ? r + 360 : r).rounded(toPlaces: 1).truncatingRemainder(dividingBy: 360)
    }

    /// What the Contract shows: readable, and regular enough to parse. The
    /// pan is the photo's centre off the stage's, as a fraction of the stage.
    func summary(stage: CGSize) -> String {
        func z(_ v: Double) -> Double { abs(v) < 0.005 ? 0 : v }
        let px = stage.width > 0 ? Double(offset.x / stage.width) : 0, py = stage.height > 0 ? Double(offset.y / stage.height) : 0
        return String(format: "scale %.2f× · rotation %.1f° · pan x %.2f y %.2f · crop x %.2f y %.2f w %.2f h %.2f",
                      Double(scale), degrees, z(px), z(py), Double(crop.minX), Double(crop.minY), Double(crop.width), Double(crop.height))
    }

    /// Scale by `factor` about `point` (relative to the canvas centre): the
    /// point under the fingers stays under them.
    mutating func zoom(by factor: CGFloat, about point: CGPoint) {
        let next = min(max(scale * factor, PhotoEdit.minScale), PhotoEdit.maxScale)
        let k = next / scale
        offset = CGPoint(x: point.x - (point.x - offset.x) * k, y: point.y - (point.y - offset.y) * k)
        scale = next
    }

    /// Turn by `delta` radians about `point` (relative to the canvas centre).
    mutating func turn(by delta: CGFloat, about point: CGPoint) {
        let d = CGPoint(x: offset.x - point.x, y: offset.y - point.y)
        let c = cos(delta), s = sin(delta)
        offset = CGPoint(x: point.x + d.x * c - d.y * s, y: point.y + d.x * s + d.y * c)
        rotation += delta
    }

    /// How far the photo's centre may move from the canvas centre: half its
    /// scaled size, so some of the photo always covers the middle.
    func bounds(fit: CGSize) -> CGSize {
        CGSize(width: fit.width * scale / 2, height: fit.height * scale / 2)
    }

    /// `offset` inside the bounds.
    func clamped(fit: CGSize) -> CGPoint {
        let b = bounds(fit: fit)
        return CGPoint(x: min(max(offset.x, -b.width), b.width), y: min(max(offset.y, -b.height), b.height))
    }

    /// A drag past the bounds meets resistance (a rubber band), as a scroll view's does.
    func banded(_ proposed: CGPoint, fit: CGSize) -> CGPoint {
        let b = bounds(fit: fit)
        func band(_ v: CGFloat, _ limit: CGFloat) -> CGFloat {
            let over = abs(v) - limit
            return over <= 0 ? v : (limit + over * 0.35) * (v < 0 ? -1 : 1)
        }
        return CGPoint(x: band(proposed.x, b.width), y: band(proposed.y, b.height))
    }

    /// Where a flick comes to rest: the release velocity carried over a
    /// scroll view's deceleration distance, then into the bounds.
    func resting(after velocity: CGPoint, fit: CGSize) -> CGPoint {
        var projected = self
        projected.offset = CGPoint(x: offset.x + velocity.x * 0.32, y: offset.y + velocity.y * 0.32)
        return projected.clamped(fit: fit)
    }
}

/// Which part of the crop rectangle a point grabs: corners, edges, or none.
struct CropGrip: OptionSet {
    let rawValue: Int
    static let left = CropGrip(rawValue: 1), right = CropGrip(rawValue: 2), top = CropGrip(rawValue: 4), bottom = CropGrip(rawValue: 8)

    /// The grip at `p` (canvas points) for `rect` (canvas points), within `reach`.
    static func at(_ p: CGPoint, in rect: CGRect, reach: CGFloat) -> CropGrip {
        guard rect.insetBy(dx: -reach, dy: -reach).contains(p) else { return [] }
        var g: CropGrip = []
        if abs(p.x - rect.minX) <= reach { g.insert(.left) } else if abs(p.x - rect.maxX) <= reach { g.insert(.right) }
        if abs(p.y - rect.minY) <= reach { g.insert(.top) } else if abs(p.y - rect.maxY) <= reach { g.insert(.bottom) }
        return g
    }

    /// `crop` (unit) with this grip's edges moved by `delta` (unit), kept
    /// inside the canvas and no smaller than the minimum.
    func moved(_ crop: CGRect, by delta: CGPoint) -> CGRect {
        var l = crop.minX, r = crop.maxX, t = crop.minY, b = crop.maxY
        let m = PhotoEdit.minCrop
        if contains(.left) { l = min(max(0, l + delta.x), r - m) }
        if contains(.right) { r = max(min(1, r + delta.x), l + m) }
        if contains(.top) { t = min(max(0, t + delta.y), b - m) }
        if contains(.bottom) { b = max(min(1, b + delta.y), t + m) }
        return CGRect(x: l, y: t, width: r - l, height: b - t)
    }
}

/// The crop overlay, shared by both canvases: the photo outside the crop
/// dimmed, the rectangle, its thirds, and its corner brackets.
func drawCrop(_ r: CGRect, in bounds: CGRect, context ctx: CGContext) {
    ctx.saveGState()
    ctx.addRect(bounds)
    ctx.addRect(r)
    ctx.setFillColor(CGColor(gray: 0, alpha: 0.55))
    ctx.fillPath(using: .evenOdd)
    ctx.setStrokeColor(CGColor(gray: 1, alpha: 0.35))
    ctx.setLineWidth(1)
    for i in 1...2 {
        let x = r.minX + r.width * CGFloat(i) / 3, y = r.minY + r.height * CGFloat(i) / 3
        ctx.move(to: CGPoint(x: x, y: r.minY)); ctx.addLine(to: CGPoint(x: x, y: r.maxY))
        ctx.move(to: CGPoint(x: r.minX, y: y)); ctx.addLine(to: CGPoint(x: r.maxX, y: y))
    }
    ctx.strokePath()
    ctx.setStrokeColor(CGColor(gray: 1, alpha: 0.95))
    ctx.setLineWidth(1.5)
    ctx.stroke(r.insetBy(dx: 0.75, dy: 0.75))
    // Brackets at the corners and bars at the edges: the handles you can grab.
    ctx.setLineWidth(4)
    let arm = min(22, r.width / 3, r.height / 3)
    for (x, dx) in [(r.minX, arm), (r.maxX, -arm)] {
        for (y, dy) in [(r.minY, arm), (r.maxY, -arm)] {
            ctx.move(to: CGPoint(x: x + dx, y: y)); ctx.addLine(to: CGPoint(x: x, y: y)); ctx.addLine(to: CGPoint(x: x, y: y + dy))
        }
    }
    ctx.move(to: CGPoint(x: r.midX - arm / 2, y: r.minY)); ctx.addLine(to: CGPoint(x: r.midX + arm / 2, y: r.minY))
    ctx.move(to: CGPoint(x: r.midX - arm / 2, y: r.maxY)); ctx.addLine(to: CGPoint(x: r.midX + arm / 2, y: r.maxY))
    ctx.move(to: CGPoint(x: r.minX, y: r.midY - arm / 2)); ctx.addLine(to: CGPoint(x: r.minX, y: r.midY + arm / 2))
    ctx.move(to: CGPoint(x: r.maxX, y: r.midY - arm / 2)); ctx.addLine(to: CGPoint(x: r.maxX, y: r.midY + arm / 2))
    ctx.strokePath()
    ctx.restoreGState()
}

/// A short ease-out from one edit to another, stepped on the main run loop:
/// the model moves, so any capture draws where the photo is. Quarter turns
/// ease through their angle (`turning`), then land exactly.
final class EditMotion {
    private var timer: Timer?
    private(set) var target: PhotoEdit?

    func run(from start: PhotoEdit, to target: PhotoEdit, duration: Double, step: @escaping (PhotoEdit) -> Void) {
        timer?.invalidate()
        self.target = target
        let began = CACurrentMediaTime()
        func mix(_ a: CGFloat, _ b: CGFloat, _ t: CGFloat) -> CGFloat { a + (b - a) * t }
        let timer = Timer(timeInterval: 1.0 / 60, repeats: true) { [weak self] timer in
            let u = min(1, (CACurrentMediaTime() - began) / duration), t = CGFloat(1 - pow(1 - u, 3))
            var e = target
            e.scale = mix(start.scale, target.scale, t)
            e.rotation = mix(start.rotation, target.rotation, t)
            e.offset = CGPoint(x: mix(start.offset.x, target.offset.x, t), y: mix(start.offset.y, target.offset.y, t))
            e.crop = CGRect(x: mix(start.crop.minX, target.crop.minX, t), y: mix(start.crop.minY, target.crop.minY, t),
                            width: mix(start.crop.width, target.crop.width, t), height: mix(start.crop.height, target.crop.height, t))
            e.turns = start.turns
            e.turning = mix(start.turning, CGFloat(target.turns - start.turns) * .pi / 2, t)
            if u >= 1 { timer.invalidate(); self?.timer = nil; self?.target = nil; step(target) } else { step(e) }
        }
        // Every run-loop mode: a drag's tracking mode or a host's own wait.
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
    }

    /// Stop, and hand back where the motion was going.
    func land() -> PhotoEdit? {
        timer?.invalidate(); timer = nil
        defer { target = nil }
        return target
    }
}

extension Double {
    func rounded(toPlaces places: Int) -> Double {
        let p = pow(10, Double(places))
        return (self * p).rounded() / p
    }
}

/// The photo, found under the app's assets: the development asset root,
/// then the bundle (macOS Resources, the iOS bundle root).
func loadPhoto(_ relative: String?) -> CGImage? {
    guard let relative, !relative.isEmpty, !relative.contains("..") else { return nil }
    var roots: [String] = []
    if let assets = ProcessInfo.processInfo.environment["EXACT_ASSETS"] { roots.append(assets) }
    if let resources = Bundle.main.resourcePath { roots.append(resources) }
    roots.append(Bundle.main.bundlePath)
    for root in roots {
        let url = URL(fileURLWithPath: root).appendingPathComponent(relative)
        #if os(macOS)
        if let image = NSImage(contentsOf: url)?.cgImage(forProposedRect: nil, context: nil, hints: nil) { return image }
        #else
        if let image = UIImage(contentsOfFile: url.path)?.cgImage { return image }
        #endif
    }
    return nil
}

/// One `<photo-editor>`: the platform view, the props it follows, the events it sends.
final class PhotoEditor: ExactNativeInstance {
    private let canvas: PhotoCanvas
    private var turns = 0
    private var reset = "0"
    private var photo = ""

    init(props: [String: String], events: ExactNativeEvents) throws {
        canvas = PhotoCanvas(frame: .zero)
        super.init(events: events)
        canvas.onEdit = { [weak self] edit in self?.report(edit, edited: true) }
        photo = props["photo"] ?? ""
        guard let image = loadPhoto(photo) else { throw ExactNativeRefusal("no photo at \(photo)") }
        canvas.image = image
        turns = Int(props["turns"] ?? "") ?? 0
        reset = props["reset"] ?? "0"
        canvas.setTurns(turns, animated: false)
        events.load()
        report(canvas.edit, edited: false)
    }

    override var view: ExactNativeView { canvas }

    override func setProps(_ props: [String: String]) throws {
        if let next = props["photo"], next != photo {
            guard let image = loadPhoto(next) else { throw ExactNativeRefusal("no photo at \(next)") }
            photo = next
            canvas.image = image
        }
        let nextReset = props["reset"] ?? "0"
        let nextTurns = Int(props["turns"] ?? "") ?? 0
        if nextReset != reset {
            reset = nextReset
            turns = nextTurns
            canvas.reset(turns: turns)
            return
        }
        if nextTurns != turns {
            turns = nextTurns
            canvas.setTurns(turns, animated: true)
        }
    }

    private var reported: PhotoEdit?

    /// A gesture that moved nothing (a tap, a double tap's first half) is no edit.
    private func report(_ edit: PhotoEdit, edited: Bool) {
        guard edit != reported else { return }
        reported = edit
        events.change(canvas.summary(edit))
        if edited { events.message("edited") }
    }
}
