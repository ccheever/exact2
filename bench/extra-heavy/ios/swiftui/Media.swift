// Media the SwiftUI app draws through UIKit/ImageIO/AVFoundation/WebKit/Lottie (SPEC Decisions):
// bundled images decoded downsampled off the main thread, looping muted video, animated GIF/WebP,
// Lottie, and the web view.
import AVFoundation
import ImageIO
import Lottie
import SwiftUI
import WebKit

// MARK: - Images: downsampled off the main thread, a small in-memory cache (the heavy bench's loader)

final class ThumbnailCache: @unchecked Sendable {
    static let shared = ThumbnailCache()
    private let cache: NSCache<NSString, UIImage> = {
        let c = NSCache<NSString, UIImage>()
        c.countLimit = 300
        return c
    }()

    func cached(_ key: String) -> UIImage? { cache.object(forKey: key as NSString) }

    /// Decode `name` from the bundle so that it aspect-fills `pixels`.
    func load(_ name: String, pixels: CGSize, key: String) async -> UIImage? {
        if let hit = cached(key) { return hit }
        let image = await Task.detached(priority: .userInitiated) { () -> UIImage? in
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
            return UIImage(cgImage: cg)
        }.value
        if let image { cache.setObject(image, forKey: key as NSString) }
        return image
    }
}

struct BundleImage: View {
    let name: String
    let size: CGSize
    @Environment(\.displayScale) private var scale
    @State private var image: UIImage?

    private var key: String { "\(name)@\(Int(size.width * scale))x\(Int(size.height * scale))" }

    var body: some View {
        ZStack {
            Color.hairline
            if let image {
                Image(uiImage: image).resizable().scaledToFill()
            }
        }
        .frame(width: size.width, height: size.height)
        .clipped()
        .task(id: key) {
            if let hit = ThumbnailCache.shared.cached(key) { image = hit; return }
            image = nil
            image = await ThumbnailCache.shared.load(
                name, pixels: CGSize(width: size.width * scale, height: size.height * scale), key: key)
        }
    }
}

// MARK: - Video: muted, autoplay, looping, aspect-fill (AVPlayerLayer; VideoPlayer has controls)

final class PlayerUIView: UIView {
    override class var layerClass: AnyClass { AVPlayerLayer.self }
    var playerLayer: AVPlayerLayer { layer as! AVPlayerLayer }
    private let player = AVQueuePlayer()
    private var looper: AVPlayerLooper?
    private(set) var name = ""

    override init(frame: CGRect) {
        super.init(frame: frame)
        player.isMuted = true
        player.preventsDisplaySleepDuringVideoPlayback = false
        playerLayer.player = player
        playerLayer.videoGravity = .resizeAspectFill
        backgroundColor = UIColor(red: 0xE5 / 255, green: 0xE5 / 255, blue: 0xEA / 255, alpha: 1)
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

    override func didMoveToWindow() {
        super.didMoveToWindow()
        update()
    }

    private func update() {
        if window != nil && !freeze { player.play() } else { player.pause() }
    }
}

struct LoopingVideo: UIViewRepresentable {
    let name: String
    func makeUIView(context: Context) -> PlayerUIView { PlayerUIView() }
    func updateUIView(_ v: PlayerUIView, context: Context) { v.load(name) }
    static func dismantleUIView(_ v: PlayerUIView, coordinator: ()) { v.playerLayer.player?.pause() }
}

// MARK: - Animated GIF / WebP: UIImageView fed by ImageIO's CGAnimateImageAtURLWithBlock

final class AnimatedUIImageView: UIImageView {
    private var name = ""
    private var generation = 0
    private var running = false

    func load(_ name: String) {
        guard name != self.name else { return }
        self.name = name
        image = nil
        if freeze {
            if let url = Bundle.main.url(forResource: name, withExtension: nil),
               let src = CGImageSourceCreateWithURL(url as CFURL, nil),
               let cg = CGImageSourceCreateImageAtIndex(src, 0, nil) { image = UIImage(cgImage: cg) }
            return
        }
        restart()
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        if window == nil { generation += 1; running = false } else if !running && !freeze { restart() }
    }

    private func restart() {
        guard window != nil, let url = Bundle.main.url(forResource: name, withExtension: nil) else { return }
        generation += 1
        running = true
        let gen = generation
        CGAnimateImageAtURLWithBlock(url as CFURL, nil) { [weak self] _, cg, stop in
            guard let self, self.generation == gen else { stop.pointee = true; return }
            self.image = UIImage(cgImage: cg)
        }
    }
}

struct AnimatedImage: UIViewRepresentable {
    let name: String
    func makeUIView(context: Context) -> AnimatedUIImageView {
        let v = AnimatedUIImageView()
        v.contentMode = .scaleAspectFit
        v.setContentHuggingPriority(.defaultLow, for: .horizontal)
        v.setContentHuggingPriority(.defaultLow, for: .vertical)
        v.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        v.setContentCompressionResistancePriority(.defaultLow, for: .vertical)
        return v
    }
    func updateUIView(_ v: AnimatedUIImageView, context: Context) { v.load(name) }
}

// MARK: - Lottie

struct LottieTile: View {
    let name: String
    var body: some View {
        let path = Bundle.main.path(forResource: name, ofType: nil) ?? ""
        let view = LottieView(animation: LottieAnimation.filepath(path)).resizable()
        if freeze {
            view.currentProgress(0.5)
        } else {
            view.playing(loopMode: .loop)
        }
    }
}

// MARK: - Web view

struct HTMLView: UIViewRepresentable {
    let html: String

    final class Coordinator { var html = "" }
    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeUIView(context: Context) -> WKWebView {
        let v = WKWebView(frame: .zero, configuration: WKWebViewConfiguration())
        v.scrollView.isScrollEnabled = false
        v.isOpaque = false
        return v
    }

    func updateUIView(_ v: WKWebView, context: Context) {
        guard context.coordinator.html != html else { return }
        context.coordinator.html = html
        v.loadHTMLString(html, baseURL: nil)
    }
}
