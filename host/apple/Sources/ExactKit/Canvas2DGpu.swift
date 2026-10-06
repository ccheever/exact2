// @ref LLP 1056 §8.5 — Canvas 2D replayed on the GPU. The module
// (`libexact_canvas_gpu.dylib`, a separate artifact loaded on demand, as the
// SVG island module is) replays a canvas's lists into an IOSurface this file
// owns, and the canvas's layer shows that IOSurface as its contents, as the
// bitmap path shows its CGImage: with the row's batch, a frame later at most.
//
// The module holds one GPU context per process and is thread-safe; each
// presenter calls it from its own replay queue. A canvas keeps its pixels in
// the IOSurface last drawn (`current`); the next replay draws into another
// (at most three: one Core Animation shows, one it may still read, one being
// drawn) and the module copies `current` into it unless the lists cover the
// canvas first.
import CoreGraphics
import CoreText
import Foundation
import IOSurface
import QuartzCore
#if canImport(UIKit)
import UIKit
#endif

/// The module's C ABI (version 2).
final class Canvas2DGpuModule {
    typealias New = @convention(c) (UInt32, UInt32, Double) -> OpaquePointer?
    typealias Free = @convention(c) (OpaquePointer?) -> Void
    typealias Replay = @convention(c) (OpaquePointer?, IOSurfaceRef?, IOSurfaceRef?, UnsafePointer<UnsafeRawPointer?>?,
                                       UnsafePointer<Int>?, Int, UnsafeRawPointer?) -> UInt32
    typealias Stats = @convention(c) (OpaquePointer?, UnsafeMutablePointer<Double>?) -> Void
    typealias Unsupported = @convention(c) (OpaquePointer?) -> UnsafePointer<CChar>?
    typealias Memory = @convention(c) () -> UInt64
    typealias Trim = @convention(c) () -> Void

    let name: String
    let new: New, free: Free, replay: Replay, stats: Stats, unsupported: Unsupported, memory: Memory, trim: Trim

    /// Which canvases the module draws (`EXACT_CANVAS_GPU`): `off`, those whose
    /// draw asks for the next frame (`animated`, the default), or every one
    /// (`always`, the parity smoke's).
    enum Mode { case off, animated, always }
    static let mode: Mode = {
        switch ProcessInfo.processInfo.environment["EXACT_CANVAS_GPU"] {
        case "off", "0": return .off
        case "always": return .always
        default: return .animated
        }
    }()

    /// The module, opened the first time a canvas asks for it; nil when it is
    /// off or not in the bundle (Core Graphics draws then).
    static let shared: Canvas2DGpuModule? = {
        guard mode != .off else { return nil }
        let env = ProcessInfo.processInfo.environment
        #if os(macOS)
        let dir = Bundle.main.executableURL?.deletingLastPathComponent().path ?? ""
        #else
        let dir = Bundle.main.privateFrameworksPath ?? Bundle.main.bundlePath
        #endif
        let path = env["EXACT_CANVAS_GPU_DYLIB"] ?? dir + "/libexact_canvas_gpu.dylib"
        let t0 = CFAbsoluteTimeGetCurrent()
        guard FileManager.default.fileExists(atPath: path) else { return nil }
        // A module that is there but does not load is said, never silently
        // replaced by Core Graphics (a stripped build dyld refused, 2026-09-30).
        guard let lib = dlopen(path, RTLD_NOW | RTLD_LOCAL) else {
            Canvas2DGpuModule.log("canvas gpu: \(path) did not load: \(dlerror().map { String(cString: $0) } ?? "?")")
            return nil
        }
        guard let abi = dlsym(lib, "ecg_abi"), unsafeBitCast(abi, to: (@convention(c) () -> UInt32).self)() == 2,
              let n = dlsym(lib, "ecg_name") else {
            Canvas2DGpuModule.log("canvas gpu: \(path) is not an ABI 2 module (built without the Metal toolchain?)")
            return nil
        }
        func sym<T>(_ s: String, _: T.Type) -> T? { dlsym(lib, s).map { unsafeBitCast($0, to: T.self) } }
        guard let new = sym("ecg_canvas_new", New.self), let free = sym("ecg_canvas_free", Free.self),
              let replay = sym("ecg_canvas_replay", Replay.self), let stats = sym("ecg_canvas_stats", Stats.self),
              let unsupported = sym("ecg_canvas_unsupported", Unsupported.self), let memory = sym("ecg_memory", Memory.self),
              let trim = sym("ecg_trim", Trim.self) else { return nil }
        let name = String(cString: unsafeBitCast(n, to: (@convention(c) () -> UnsafePointer<CChar>).self)())
        #if canImport(UIKit)
        // The module's caches are rebuilt on demand: a memory warning or the
        // background drops them.
        for note in [UIApplication.didReceiveMemoryWarningNotification, UIApplication.didEnterBackgroundNotification] {
            NotificationCenter.default.addObserver(forName: note, object: nil, queue: nil) { _ in trim() }
        }
        #endif
        Canvas2DGpuModule.log(String(format: "canvas gpu: %@ loaded in %.1f ms", name, (CFAbsoluteTimeGetCurrent() - t0) * 1000))
        return Canvas2DGpuModule(name: name, new: new, free: free, replay: replay, stats: stats, unsupported: unsupported, memory: memory, trim: trim)
    }()

    private init(name: String, new: @escaping New, free: @escaping Free, replay: @escaping Replay, stats: @escaping Stats,
                 unsupported: @escaping Unsupported, memory: @escaping Memory, trim: @escaping Trim) {
        self.name = name; self.new = new; self.free = free; self.replay = replay; self.stats = stats
        self.unsupported = unsupported; self.memory = memory; self.trim = trim
    }

    static func log(_ s: String) { FileHandle.standardError.write(Data((s + "\n").utf8)) }
}

/// `EcgRun`: one Core Text run of a text line.
struct EcgRun {
    var font: UnsafeRawPointer?
    var glyphs: UnsafePointer<UInt16>?
    var positions: UnsafePointer<Double>?
    var count: Int
}

/// `EcgHost`: what the module asks of the host during one replay.
struct EcgHost {
    typealias TextPath = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<Double>?, Int, UnsafePointer<UInt32>?, Int, UInt32,
                                         UnsafeMutablePointer<Int>?) -> UnsafePointer<Float>?
    typealias TextRuns = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<Double>?, Int, UnsafePointer<UInt32>?, Int, UInt32,
                                         UnsafeMutablePointer<Int>?) -> UnsafeRawPointer?
    typealias Image = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<UInt32>?,
                                      UnsafeMutablePointer<UInt32>?) -> UnsafePointer<UInt8>?
    var ctx: UnsafeMutableRawPointer?
    var textPath: TextPath?
    var textRuns: TextRuns?
    var image: Image?
}

/// Decoded images as premultiplied RGBA8, by handle: the module caches its
/// uploads by pointer, so a handle's pixels live as long as the process.
final class Canvas2DGpuPixels {
    static let shared = Canvas2DGpuPixels()
    private let lock = NSLock()
    private var pixels: [String: (image: CGImage, bytes: UnsafeMutableRawPointer, w: Int, h: Int)] = [:]

    func rgba(_ src: String, _ image: CGImage, p3: Bool) -> (UnsafeMutableRawPointer, Int, Int)? {
        lock.lock(); defer { lock.unlock() }
        let key = p3 ? "p3 " + src : src
        if let hit = pixels[key], hit.image === image { return (hit.bytes, hit.w, hit.h) }
        let (w, h) = (image.width, image.height)
        guard w > 0, h > 0 else { return nil }
        let bytes = UnsafeMutableRawPointer.allocate(byteCount: w * h * 4, alignment: 16)
        guard let c = CGContext(data: bytes, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                space: Canvas2DSpace(p3: p3).space,
                                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue) else {
            bytes.deallocate(); return nil
        }
        c.clear(CGRect(x: 0, y: 0, width: w, height: h))
        c.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
        // A re-decoded handle keeps its old pixels alive: the module may hold
        // an upload keyed by the old pointer.
        pixels[key] = (image, bytes, w, h)
        return (bytes, w, h)
    }
}

/// What one replay's callbacks read and hand back: the snapshot's fonts and
/// images, and the buffers the last callback returned (valid until the next
/// callback, freed with this object).
final class Canvas2DGpuCallbacks {
    let env: Canvas2DEnv
    /// The last callback's buffers, and the fonts its runs point at.
    private var held: [UnsafeMutableRawPointer] = []
    private var fonts: [CTFont] = []

    let p3: Bool
    init(env: Canvas2DEnv, p3: Bool) { self.env = env; self.p3 = p3 }
    deinit { release() }

    private func release() {
        for p in held { p.deallocate() }
        held.removeAll(); fonts.removeAll()
    }

    /// A buffer holding `values`, kept until the next callback.
    private func hold<T>(_ values: [T]) -> UnsafePointer<T> {
        let p = UnsafeMutablePointer<T>.allocate(capacity: max(1, values.count))
        _ = UnsafeMutableBufferPointer(start: p, count: values.count).initialize(from: values)
        held.append(UnsafeMutableRawPointer(p))
        return UnsafePointer(p)
    }

    private func line(_ font: UnsafePointer<Double>?, _ fontN: Int, _ text: UnsafePointer<UInt32>?, _ textN: Int, _ rtl: UInt32) -> CTLine? {
        guard let font else { return nil }
        let f = Canvas2DFont(record: Array(UnsafeBufferPointer(start: font, count: fontN)), count: fontN)
        guard let ct = env.canvasFont(f) else { return nil }
        var s = String.UnicodeScalarView()
        if let text { for i in 0..<textN { if let u = Unicode.Scalar(text[i]) { s.append(u) } } }
        return CanvasText.line(ct, f, String(s), rtl: rtl != 0)
    }

    static let textPath: EcgHost.TextPath = { ctx, font, fontN, text, textN, rtl, len in
        guard let ctx, let len else { return nil }
        let me = Unmanaged<Canvas2DGpuCallbacks>.fromOpaque(ctx).takeUnretainedValue()
        me.release()
        guard let line = me.line(font, fontN, text, textN, rtl) else { return nil }
        var floats: [Float] = []
        CanvasText.outline(line).applyWithBlock { e in
            let p = e.pointee.points
            switch e.pointee.type {
            case .moveToPoint: floats += [0, Float(p[0].x), Float(p[0].y)]
            case .addLineToPoint: floats += [1, Float(p[0].x), Float(p[0].y)]
            case .addQuadCurveToPoint: floats += [2, Float(p[0].x), Float(p[0].y), Float(p[1].x), Float(p[1].y)]
            case .addCurveToPoint: floats += [3, Float(p[0].x), Float(p[0].y), Float(p[1].x), Float(p[1].y), Float(p[2].x), Float(p[2].y)]
            case .closeSubpath: floats.append(4)
            @unknown default: break
            }
        }
        len.pointee = floats.count
        return me.hold(floats)
    }

    static let textRuns: EcgHost.TextRuns = { ctx, font, fontN, text, textN, rtl, count in
        guard let ctx, let count else { return nil }
        let me = Unmanaged<Canvas2DGpuCallbacks>.fromOpaque(ctx).takeUnretainedValue()
        me.release()
        guard let line = me.line(font, fontN, text, textN, rtl) else { return nil }
        var runs: [EcgRun] = []
        for case let run as CTRun in CTLineGetGlyphRuns(line) as [AnyObject] {
            let attrs = CTRunGetAttributes(run) as NSDictionary
            guard let f = attrs[kCTFontAttributeName] as! CTFont? else { continue }
            let n = CTRunGetGlyphCount(run)
            var g = [CGGlyph](repeating: 0, count: n), p = [CGPoint](repeating: .zero, count: n)
            CTRunGetGlyphs(run, CFRange(location: 0, length: 0), &g)
            CTRunGetPositions(run, CFRange(location: 0, length: 0), &p)
            me.fonts.append(f)
            runs.append(EcgRun(font: UnsafeRawPointer(Unmanaged.passUnretained(f).toOpaque()), glyphs: me.hold(g),
                               positions: me.hold(p.flatMap { [Double($0.x), Double($0.y)] }), count: n))
        }
        count.pointee = runs.count
        return UnsafeRawPointer(me.hold(runs))
    }

    static let image: EcgHost.Image = { ctx, src, srcN, w, h in
        guard let ctx, let src, let w, let h else { return nil }
        let me = Unmanaged<Canvas2DGpuCallbacks>.fromOpaque(ctx).takeUnretainedValue()
        let key = String(decoding: UnsafeBufferPointer(start: src, count: srcN), as: UTF8.self)
        guard let image = me.env.canvasImage(key), let (bytes, iw, ih) = Canvas2DGpuPixels.shared.rgba(key, image, p3: me.p3) else { return nil }
        w.pointee = UInt32(iw); h.pointee = UInt32(ih)
        return UnsafePointer(bytes.assumingMemoryBound(to: UInt8.self))
    }
}

/// One canvas on the GPU: the module's canvas and the IOSurfaces it draws
/// into. Touched only on its presenter's replay queue.
final class Canvas2DGpuCanvas {
    let module: Canvas2DGpuModule
    let handle: OpaquePointer
    let lifetime: UInt64, generation: UInt32
    let width: Int, height: Int
    private var surfaces: [IOSurface] = []
    /// The surface holding the canvas's pixels (the one last drawn).
    private(set) var current: IOSurface?
    private(set) var lastMs = [Double](repeating: 0, count: 3)

    let p3: Bool

    init?(module: Canvas2DGpuModule, width: Int, height: Int, scale: Double, lifetime: UInt64, generation: UInt32, p3: Bool) {
        guard width > 0, height > 0, let h = module.new(UInt32(width), UInt32(height), scale) else { return nil }
        self.module = module; handle = h; self.p3 = p3
        self.width = width; self.height = height; self.lifetime = lifetime; self.generation = generation
    }

    deinit { module.free(handle) }

    private static let srgb = CGColorSpace(name: CGColorSpace.sRGB)!.copyPropertyList()
    private static let displayP3 = CGColorSpace(name: CGColorSpace.displayP3)!.copyPropertyList()

    private func makeSurface() -> IOSurface? {
        let props: [IOSurfacePropertyKey: Any] = [
            .width: width, .height: height, .bytesPerElement: 4, .pixelFormat: 0x4247_5241, // 'BGRA'
        ]
        guard let s = IOSurface(properties: props) else { return nil }
        if let tag = p3 ? Canvas2DGpuCanvas.displayP3 : Canvas2DGpuCanvas.srgb { IOSurfaceSetValue(s, kIOSurfaceColorSpace, tag) }
        return s
    }

    /// A surface neither holding the pixels nor still read by Core Animation.
    private func target() -> IOSurface? {
        let free = { self.surfaces.first(where: { $0 !== self.current && !$0.isInUse }) }
        if let s = free() { return s }
        if surfaces.count < 2, let s = makeSurface() { surfaces.append(s); return s }
        // The one shown before is still Core Animation's until its next
        // frame: wait up to a 120 Hz frame for it before a third surface
        // (a full-screen surface is 14 MB on a 3x phone).
        for _ in 0..<16 {
            usleep(500)
            if let s = free() { return s }
        }
        if surfaces.count < 3, let s = makeSurface() { surfaces.append(s); return s }
        for _ in 0..<40 {
            usleep(500)
            if let s = free() { return s }
        }
        return surfaces.first { $0 !== current }
    }

    private static var drawn = false
    /// The cold first draw, logged once: process start to the first GPU
    /// replay returned, and that replay's own time.
    private static func firstDraw(_ ms: [Double]) {
        guard !drawn, Canvas2DStats.on else { return }
        drawn = true
        var info = kinfo_proc(), size = MemoryLayout<kinfo_proc>.stride
        var mib: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_PID, getpid()]
        sysctl(&mib, 4, &info, &size, nil, 0)
        let start = info.kp_proc.p_starttime
        var now = timeval(); gettimeofday(&now, nil)
        let since = Double(now.tv_sec - start.tv_sec) * 1000 + Double(now.tv_usec - start.tv_usec) / 1000
        Canvas2DGpuModule.log(String(format: "canvas gpu: first replay %.1f ms (cpu %.1f, gpu %.1f); process start to first GPU pixels %.0f ms", ms[2], ms[0], ms[1], since))
    }

    /// Replay `lists` over the kept pixels; the surface to show, or nil when
    /// the GPU failed (the host falls back to Core Graphics).
    func replay(_ lists: [Data], env: Canvas2DEnv) -> (surface: IOSurface?, unreadable: Bool) {
        guard let t = target() else { return (nil, false) }
        let callbacks = Canvas2DGpuCallbacks(env: env, p3: p3)
        var host = EcgHost(ctx: Unmanaged.passUnretained(callbacks).toOpaque(), textPath: Canvas2DGpuCallbacks.textPath,
                           textRuns: Canvas2DGpuCallbacks.textRuns, image: Canvas2DGpuCallbacks.image)
        let ns = lists.map { $0 as NSData }
        let ptrs: [UnsafeRawPointer?] = ns.map { $0.bytes }
        let lens = ns.map { $0.length }
        let code = withExtendedLifetime((ns, callbacks)) {
            withUnsafePointer(to: &host) { hostPtr in
                module.replay(handle, unsafeBitCast(t, to: IOSurfaceRef.self), current.map { unsafeBitCast($0, to: IOSurfaceRef.self) },
                              ptrs, lens, ptrs.count, UnsafeRawPointer(hostPtr))
            }
        }
        module.stats(handle, &lastMs)
        Canvas2DGpuCanvas.firstDraw(lastMs)
        if let u = module.unsupported(handle) {
            Canvas2DGpuModule.log("canvas gpu: unsupported \(String(cString: u))")
        }
        guard code != 2 else { return (nil, false) }
        current = t
        return (t, code == 1)
    }
}

/// `EXACT_CANVAS_STATS=1`: once a second, how many replays each path showed
/// and the GPU module's time, and the cold first GPU draw: the device
/// measure of LLP 1056 §8.5.
enum Canvas2DStats {
    static let on = ProcessInfo.processInfo.environment["EXACT_CANVAS_STATS"] == "1"
    private static var gpu = 0, bitmap = 0, recorded = 0, gpuMs = 0.0, t0 = CACurrentMediaTime()
    /// Lists that arrived (one a draw) and those that joined a replay already waiting.
    private static var arrived = 0, joined = 0
    static func arrival(joined j: Bool) {
        guard on else { return }
        arrived += 1
        if j { joined += 1 }
    }
    static func shown(gpu g: Bool, recorded r: Bool, ms: Double) {
        guard on else { return }
        if g { gpu += 1; gpuMs += ms } else if r { recorded += 1 } else { bitmap += 1 }
        let dt = CACurrentMediaTime() - t0
        guard dt >= 1 else { return }
        let mem = Canvas2DGpuModule.shared.map { Double($0.memory()) / 1_048_576 } ?? 0
        NSLog("canvas stats: %.1f gpu/s (%.2f ms each), %.1f recorded/s, %.1f bitmap/s, module %.1f MB; draws %.1f/s, %.1f/s joined a waiting replay",
              Double(gpu) / dt, gpu > 0 ? gpuMs / Double(gpu) : 0, Double(recorded) / dt, Double(bitmap) / dt, mem,
              Double(arrived) / dt, Double(joined) / dt)
        gpu = 0; bitmap = 0; recorded = 0; gpuMs = 0; arrived = 0; joined = 0; t0 = CACurrentMediaTime()
    }
}
