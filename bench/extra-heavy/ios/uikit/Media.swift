// Media: bundled images decoded downsampled off the main thread (the same ImageIO thumbnail decode and
// 300-entry cache as the SwiftUI app's loader, plus prefetch and cancellation), looping muted video,
// animated GIF/WebP, Lottie, the web view's HTML.
import AVFoundation
import ImageIO
import Lottie
import UIKit
import WebKit

// MARK: - Image loader

final class ImageLoader {
    static let shared = ImageLoader()
    private let cache: NSCache<NSString, UIImage> = {
        let c = NSCache<NSString, UIImage>()
        c.countLimit = 300
        return c
    }()
    private let queue: OperationQueue = {
        let q = OperationQueue()
        q.maxConcurrentOperationCount = max(2, ProcessInfo.processInfo.activeProcessorCount - 2)
        q.qualityOfService = .userInitiated
        return q
    }()
    /// In-flight decodes (main thread only): the operation and who waits for it.
    private var pending: [String: (op: Operation, waiters: [Int: (UIImage) -> Void], prefetch: Bool)] = [:]
    private var nextToken = 1

    static func key(_ name: String, _ px: CGSize) -> String { "\(name)@\(Int(px.width))x\(Int(px.height))" }

    func cached(_ name: String, _ size: CGSize) -> UIImage? {
        cache.object(forKey: Self.key(name, px(size)) as NSString)
    }

    private func px(_ size: CGSize) -> CGSize { CGSize(width: size.width * screenScale, height: size.height * screenScale) }

    /// Asks for `name` decoded to aspect-fill `size` points. Calls `done` on the main thread (never
    /// synchronously); returns a token for `cancel`, or 0 if it was a cache hit handled inline.
    @discardableResult
    func load(_ name: String, _ size: CGSize, _ done: @escaping (UIImage) -> Void) -> Int {
        let p = px(size)
        let key = Self.key(name, p)
        if let hit = cache.object(forKey: key as NSString) { done(hit); return 0 }
        let token = nextToken
        nextToken += 1
        if pending[key] != nil {
            pending[key]!.waiters[token] = done
            pending[key]!.op.queuePriority = .high
        } else {
            start(name, p, key, waiters: [token: done], prefetch: false)
        }
        return token
    }

    func prefetch(_ name: String, _ size: CGSize) {
        let p = px(size)
        let key = Self.key(name, p)
        if pending[key] != nil || cache.object(forKey: key as NSString) != nil { return }
        start(name, p, key, waiters: [:], prefetch: true)
    }

    func cancel(_ token: Int) {
        guard token != 0 else { return }
        for (key, var e) in pending where e.waiters[token] != nil {
            e.waiters[token] = nil
            if e.waiters.isEmpty && !e.prefetch { e.op.cancel(); pending[key] = nil } else { pending[key] = e }
            return
        }
    }

    private func start(_ name: String, _ p: CGSize, _ key: String, waiters: [Int: (UIImage) -> Void], prefetch: Bool) {
        let op = BlockOperation()
        op.addExecutionBlock { [unowned op] in
            guard !op.isCancelled else { return }
            let img = decodeThumbnail(name, pixels: p)
            DispatchQueue.main.async {
                guard let e = self.pending[key] else { return }
                self.pending[key] = nil
                guard let img else { return }
                self.cache.setObject(img, forKey: key as NSString)
                for w in e.waiters.values { w(img) }
            }
        }
        op.queuePriority = prefetch ? .low : .high
        pending[key] = (op, waiters, prefetch)
        queue.addOperation(op)
    }
}

/// ImageIO thumbnail that aspect-fills `pixels` (never upscaled), decoded now (off the main thread).
func decodeThumbnail(_ name: String, pixels: CGSize) -> UIImage? {
    guard let url = Bundle.main.url(forResource: name, withExtension: nil),
          let src = CGImageSourceCreateWithURL(url as CFURL, [kCGImageSourceShouldCache: false] as CFDictionary),
          let props = CGImageSourceCopyPropertiesAtIndex(src, 0, nil) as? [CFString: Any],
          let iw = props[kCGImagePropertyPixelWidth] as? CGFloat,
          let ih = props[kCGImagePropertyPixelHeight] as? CGFloat
    else { return nil }
    let fill = max(pixels.width / iw, pixels.height / ih)
    let maxPixel = ceil(max(iw, ih) * min(fill, 1))
    let opts: [CFString: Any] = [
        kCGImageSourceCreateThumbnailFromImageAlways: true,
        kCGImageSourceCreateThumbnailWithTransform: true,
        kCGImageSourceShouldCacheImmediately: true,
        kCGImageSourceThumbnailMaxPixelSize: maxPixel,
    ]
    guard let cg = CGImageSourceCreateThumbnailAtIndex(src, 0, opts as CFDictionary) else { return nil }
    return UIImage(cgImage: cg, scale: screenScale, orientation: .up)
}

/// An aspect-fill image view over the loader, #E5E5EA until decoded.
final class LoadingImageView: UIImageView {
    private var key = ""
    private var token = 0
    var fadeIn = false

    override init(frame: CGRect) {
        super.init(frame: frame)
        contentMode = .scaleAspectFill
        clipsToBounds = true
        backgroundColor = .hairline
        layer.cornerCurve = .continuous
    }
    required init?(coder: NSCoder) { fatalError() }
    convenience init() { self.init(frame: .zero) }

    func set(_ name: String, _ size: CGSize) {
        let k = "\(name)@\(size.width)x\(size.height)"
        if k == key { return }
        key = k
        ImageLoader.shared.cancel(token)
        token = 0
        if let hit = ImageLoader.shared.cached(name, size) { image = hit; return }
        image = nil
        token = ImageLoader.shared.load(name, size) { [weak self] img in
            guard let self, self.key == k else { return }
            self.token = 0
            self.image = img
        }
    }
}

// MARK: - Video: muted, autoplay, looping, aspect-fill AVPlayerLayer; plays while its cell is displayed

final class PlayerView: UIView {
    override class var layerClass: AnyClass { AVPlayerLayer.self }
    var playerLayer: AVPlayerLayer { layer as! AVPlayerLayer }
    private let player = AVQueuePlayer()
    private var looper: AVPlayerLooper?
    private var name = ""
    private var visible = false

    override init(frame: CGRect) {
        super.init(frame: frame)
        player.isMuted = true
        player.preventsDisplaySleepDuringVideoPlayback = false
        playerLayer.player = player
        playerLayer.videoGravity = .resizeAspectFill
        backgroundColor = .hairline
    }
    required init?(coder: NSCoder) { fatalError() }

    func load(_ name: String) {
        guard name != self.name, let url = Bundle.main.url(forResource: name, withExtension: nil) else { return }
        self.name = name
        looper?.disableLooping()
        player.removeAllItems()
        looper = AVPlayerLooper(player: player, templateItem: AVPlayerItem(url: url))
        update()
    }

    func setVisible(_ v: Bool) { visible = v; update() }

    private func update() {
        if visible && !freeze { player.play() } else { player.pause() }
    }
}

// MARK: - Animated GIF / WebP: ImageIO's CGAnimateImageAtURLWithBlock into a UIImageView

final class AnimatedImageView: UIImageView {
    private var name = ""
    private var generation = 0
    private var running = false
    private var visible = false

    func load(_ name: String) {
        guard name != self.name else { return }
        self.name = name
        image = nil
        generation += 1
        running = false
        if freeze {
            if let url = Bundle.main.url(forResource: name, withExtension: nil),
               let src = CGImageSourceCreateWithURL(url as CFURL, nil),
               let cg = CGImageSourceCreateImageAtIndex(src, 0, nil) { image = UIImage(cgImage: cg) }
            return
        }
        if visible { restart() }
    }

    func setVisible(_ v: Bool) {
        visible = v
        if !v { generation += 1; running = false } else if !running && !freeze { restart() }
    }

    private func restart() {
        guard let url = Bundle.main.url(forResource: name, withExtension: nil) else { return }
        generation += 1
        running = true
        let gen = generation
        CGAnimateImageAtURLWithBlock(url as CFURL, nil) { [weak self] _, cg, stop in
            guard let self, self.generation == gen else { stop.pointee = true; return }
            self.image = UIImage(cgImage: cg)
        }
    }
}

// MARK: - Lottie (lottie-ios, default rendering engine, as SwiftUI's LottieView)

let lottieCache = NSCache<NSString, LottieAnimation>()
func lottieAnimation(_ name: String) -> LottieAnimation? {
    if let a = lottieCache.object(forKey: name as NSString) { return a }
    guard let path = Bundle.main.path(forResource: name, ofType: nil), let a = LottieAnimation.filepath(path) else { return nil }
    lottieCache.setObject(a, forKey: name as NSString)
    return a
}

// MARK: - Web view HTML

let embedTemplate: String = {
    guard let url = Bundle.main.url(forResource: "embed", withExtension: "html"),
          let s = try? String(contentsOf: url, encoding: .utf8) else { return "" }
    return s
}()

func embedHTML(_ row: Row) -> String {
    let bars = row.bars!.map { "<div class=\"bar\" style=\"height:\($0)%\"></div>" }.joined()
    return embedTemplate.replacingOccurrences(of: "{{HUE}}", with: "\(row.hue!)")
        .replacingOccurrences(of: "{{TITLE}}", with: row.title ?? "")
        .replacingOccurrences(of: "{{N}}", with: "\(row.index)")
        .replacingOccurrences(of: "{{PLAY}}", with: freeze ? "paused" : "running")
        .replacingOccurrences(of: "{{BARS}}", with: bars)
}
