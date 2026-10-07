// gen_video.swift <out-dir> — the four bundled clips (ffmpeg is broken on this Mac), called by gen.py.
// clip-0N.mp4: 640 × 360, 30 fps, 120 frames (4.0 s), H.264 Main, no audio track.
// Content is a pure function of (clip, frame): a hue-drifting vertical gradient, three circles on
// Lissajous paths and a progress bar along the bottom, so a loop seam and a stalled decoder are visible.
// The encoder's bytes may differ run to run; the pixels it is fed do not.
import AVFoundation
import CoreGraphics
import CoreVideo
import Foundation

let outDir = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "."
let W = 640, H = 360, FPS: Int32 = 30, FRAMES = 120

func hsv(_ h0: Double, _ s: Double, _ v: Double) -> (CGFloat, CGFloat, CGFloat) {
    let h = h0 - floor(h0)
    let i = Int(floor(h * 6)) % 6, f = h * 6 - floor(h * 6)
    let p = v * (1 - s), q = v * (1 - f * s), t = v * (1 - (1 - f) * s)
    let (r, g, b): (Double, Double, Double) = [(v, t, p), (q, v, p), (p, v, t), (p, q, v), (t, p, v), (v, p, q)][i]
    return (CGFloat(r), CGFloat(g), CGFloat(b))
}

func draw(_ ctx: CGContext, clip: Int, frame: Int) {
    let t = Double(frame) / Double(FRAMES)          // 0..1 over the loop
    let base = Double(clip) * 0.23
    let (r0, g0, b0) = hsv((base + 0.08 * sin(2 * .pi * t)).truncatingRemainder(dividingBy: 1), 0.55, 0.95)
    let (r1, g1, b1) = hsv((base + 0.35).truncatingRemainder(dividingBy: 1), 0.7, 0.45)
    let cs = CGColorSpace(name: CGColorSpace.sRGB)!
    let grad = CGGradient(colorsSpace: cs, colors: [CGColor(red: r0, green: g0, blue: b0, alpha: 1),
                                                    CGColor(red: r1, green: g1, blue: b1, alpha: 1)] as CFArray,
                          locations: [0, 1])!
    ctx.drawLinearGradient(grad, start: CGPoint(x: 0, y: H), end: CGPoint(x: 0, y: 0), options: [])
    for k in 0..<3 {
        let a = 2 * Double.pi * t
        let cx = Double(W) / 2 + Double(W) * 0.34 * sin(a * Double(k + 1) + Double(clip + k))
        let cy = Double(H) / 2 + Double(H) * 0.30 * cos(a * Double(3 - k) + Double(k))
        let rad = 36.0 + 14.0 * Double(k)
        let (r, g, b) = hsv((base + 0.15 * Double(k + 1)).truncatingRemainder(dividingBy: 1), 0.8, 1)
        ctx.setFillColor(CGColor(red: r, green: g, blue: b, alpha: 0.85))
        ctx.fillEllipse(in: CGRect(x: cx - rad, y: cy - rad, width: rad * 2, height: rad * 2))
    }
    ctx.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 0.9))
    ctx.fill(CGRect(x: 0, y: 0, width: Double(W) * (Double(frame + 1) / Double(FRAMES)), height: 8))
}

func write(clip: Int) throws {
    let url = URL(fileURLWithPath: outDir).appendingPathComponent(String(format: "clip-%02d.mp4", clip))
    try? FileManager.default.removeItem(at: url)
    let w = try AVAssetWriter(outputURL: url, fileType: .mp4)
    let input = AVAssetWriterInput(mediaType: .video, outputSettings: [
        AVVideoCodecKey: AVVideoCodecType.h264, AVVideoWidthKey: W, AVVideoHeightKey: H,
        AVVideoCompressionPropertiesKey: [AVVideoAverageBitRateKey: 900_000,
                                          AVVideoProfileLevelKey: AVVideoProfileLevelH264MainAutoLevel,
                                          AVVideoMaxKeyFrameIntervalKey: 30]])
    input.expectsMediaDataInRealTime = false
    let adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: input, sourcePixelBufferAttributes: [
        kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32ARGB, kCVPixelBufferWidthKey as String: W,
        kCVPixelBufferHeightKey as String: H])
    w.add(input)
    w.startWriting()
    w.startSession(atSourceTime: .zero)
    for f in 0..<FRAMES {
        while !input.isReadyForMoreMediaData { usleep(1000) }
        var pb: CVPixelBuffer?
        CVPixelBufferPoolCreatePixelBuffer(nil, adaptor.pixelBufferPool!, &pb)
        let buf = pb!
        CVPixelBufferLockBaseAddress(buf, [])
        let ctx = CGContext(data: CVPixelBufferGetBaseAddress(buf), width: W, height: H, bitsPerComponent: 8,
                            bytesPerRow: CVPixelBufferGetBytesPerRow(buf), space: CGColorSpace(name: CGColorSpace.sRGB)!,
                            bitmapInfo: CGImageAlphaInfo.noneSkipFirst.rawValue)!
        draw(ctx, clip: clip, frame: f)
        CVPixelBufferUnlockBaseAddress(buf, [])
        adaptor.append(buf, withPresentationTime: CMTime(value: CMTimeValue(f), timescale: FPS))
    }
    input.markAsFinished()
    let done = DispatchSemaphore(value: 0)
    w.finishWriting { done.signal() }
    done.wait()
    if w.status != .completed { throw w.error ?? NSError(domain: "gen_video", code: 1) }
    print("wrote", url.lastPathComponent)
}

for c in 0..<4 { try write(clip: c) }
