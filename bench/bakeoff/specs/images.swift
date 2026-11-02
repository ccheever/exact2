// Writes the image specs' pictures: deterministic abstract scenes (sky gradient, hills, a sun, shapes), JPEG at
// the asked size, so a list of images decodes real pixels without shipping them in the repository.
//   swift bench/bakeoff/specs/images.swift <out dir> <count> <seed> [max side]
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

let args = CommandLine.arguments
guard args.count >= 4, let count = Int(args[2]), var seed = UInt64(args[3]) else {
    print("usage: images.swift <out dir> <count> <seed> [max side]"); exit(2)
}
let maxSide = args.count > 4 ? Int(args[4]) ?? 900 : 900
let out = URL(fileURLWithPath: args[1])
try FileManager.default.createDirectory(at: out, withIntermediateDirectories: true)
func rnd() -> Double { seed = seed &* 6364136223846793005 &+ 1442695040888963407; return Double(seed >> 11) / Double(1 << 53) }
func color(_ h: Double, _ s: Double, _ b: Double) -> CGColor {
    let i = Int(h * 6) % 6, f = h * 6 - floor(h * 6), p = b * (1 - s), q = b * (1 - f * s), t = b * (1 - (1 - f) * s)
    let (r, g, bl) = [(b, t, p), (q, b, p), (p, b, t), (p, q, b), (t, p, b), (b, p, q)][i]
    return CGColor(red: r, green: g, blue: bl, alpha: 1)
}
let aspects: [(Int, Int)] = [(1, 1), (4, 3), (3, 4), (16, 9), (3, 2), (2, 3)]
var manifest: [[String: Any]] = []
for n in 1...count {
    let (aw, ah) = aspects[Int(rnd() * Double(aspects.count))]
    let w = aw >= ah ? maxSide : maxSide * aw / ah, h = ah >= aw ? maxSide : maxSide * ah / aw
    let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)!
    let hue = rnd()
    let sky = CGGradient(colorsSpace: nil, colors: [color(hue, 0.5, 0.95), color((hue + 0.08).truncatingRemainder(dividingBy: 1), 0.35, 0.75)] as CFArray, locations: [0, 1])!
    ctx.drawLinearGradient(sky, start: CGPoint(x: 0, y: h), end: CGPoint(x: 0, y: 0), options: [])
    ctx.setFillColor(color((hue + 0.5).truncatingRemainder(dividingBy: 1), 0.6, 1))
    let r = Double(min(w, h)) * (0.08 + rnd() * 0.1)
    ctx.fillEllipse(in: CGRect(x: rnd() * Double(w), y: Double(h) * (0.55 + rnd() * 0.3), width: r * 2, height: r * 2))
    for layer in 0..<3 {
        ctx.setFillColor(color((hue + 0.3 + Double(layer) * 0.05).truncatingRemainder(dividingBy: 1), 0.5, 0.55 - Double(layer) * 0.15))
        ctx.move(to: CGPoint(x: 0, y: 0))
        var x = 0.0
        while x <= Double(w) {
            ctx.addLine(to: CGPoint(x: x, y: Double(h) * (0.2 + 0.25 * rnd() - Double(layer) * 0.06)))
            x += Double(w) / 6
        }
        ctx.addLine(to: CGPoint(x: Double(w), y: 0)); ctx.closePath(); ctx.fillPath()
    }
    for _ in 0..<Int(4 + rnd() * 10) {
        ctx.setFillColor(color(rnd(), 0.4, 0.4 + rnd() * 0.5).copy(alpha: 0.5)!)
        let s = Double(min(w, h)) * (0.02 + rnd() * 0.06)
        ctx.fill(CGRect(x: rnd() * Double(w), y: rnd() * Double(h) * 0.4, width: s, height: s * (1 + rnd())))
    }
    let name = String(format: "p%03d.jpg", n)
    let dest = CGImageDestinationCreateWithURL(out.appendingPathComponent(name) as CFURL, UTType.jpeg.identifier as CFString, 1, nil)!
    CGImageDestinationAddImage(dest, ctx.makeImage()!, [kCGImageDestinationLossyCompressionQuality: 0.8] as CFDictionary)
    CGImageDestinationFinalize(dest)
    manifest.append(["file": name, "width": w, "height": h])
}
try JSONSerialization.data(withJSONObject: manifest, options: [.prettyPrinted, .sortedKeys]).write(to: out.appendingPathComponent("images.json"))
print("wrote \(count) images to \(out.path)")
