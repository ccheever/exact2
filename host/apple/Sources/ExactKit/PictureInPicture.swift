// @ref LLP 1042 §9. `requestPictureInPicture(id)`: an `image` floats in the platform's
// picture-in-picture window, which shows each picture its `src` names next, until
// `exitPictureInPicture()` or the window's own close; a failure is the image's
// `pictureinpictureerror`. AVKit stays in the video arm
// (`host/apple/videoarm/PictureInPicture.swift`); this side follows the node.
// `presentation="window"` or `"panel"` (LLP 1042 §9.1) floats it in Exact's own window on
// macOS instead (`Mac/PictureInPictureWindowMac.swift`), which needs no arm.
import Foundation

private typealias FloatCallback = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void

/// The one floating image of the process, as a document has one picture-in-picture element:
/// a request for another image moves the window to it, and the first hears it leave.
final class PictureInPictureHost {
    private typealias Create = @convention(c) (UnsafeMutableRawPointer?, FloatCallback?) -> UnsafeMutableRawPointer?
    private typealias Frame = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void
    private typealias Destroy = @convention(c) (UnsafeMutableRawPointer?) -> Void
    private static var current: PictureInPictureHost?
    private weak var node: NodeView?
    private var handle: UnsafeMutableRawPointer?
    private var source: String?
    private var entered = false
    let options: Options
    #if os(macOS)
    /// Exact's window, where the request asked for it: the arm's handle stays nil.
    private(set) var window: PictureInPictureWindow?
    #endif

    /// The request's options (LLP 1042 §9.1): `presentation="window"` and how that window opens.
    struct Options: Equatable {
        /// `"window"` or `"panel"`, Exact's window; nil, the platform's.
        var presentation: String?
        var title: String?
        var width: Double?
        var height: Double?
        var window: Bool { presentation != nil }
        init(_ args: [Any]) {
            presentation = args.count > 1 ? (args[1] as? String).flatMap { ["window", "panel"].contains($0) ? $0 : nil } : nil
            title = args.count > 2 ? args[2] as? String : nil
            width = args.count > 3 ? (args[3] as? NSNumber)?.doubleValue : nil
            height = args.count > 4 ? (args[4] as? NSNumber)?.doubleValue : nil
        }
    }

    private init(_ node: NodeView, _ options: Options) { self.node = node; self.options = options }

    /// The image floating now, and its host: the agent reads and presses its window.
    static var floating: (node: NodeView, host: PictureInPictureHost)? {
        guard let current, let node = current.node, current.entered else { return nil }
        return (node, current)
    }

    /// The arm's three exports, from the video arm's library.
    private static let exports: (create: Create, frame: Frame, destroy: Destroy)? = {
        guard let library = VideoArmLibrary.handle,
              let create = dlsym(library, "exact_pip_create"), let frame = dlsym(library, "exact_pip_frame"),
              let destroy = dlsym(library, "exact_pip_destroy") else { return nil }
        return (unsafeBitCast(create, to: Create.self), unsafeBitCast(frame, to: Frame.self), unsafeBitCast(destroy, to: Destroy.self))
    }()

    static func request(_ node: NodeView, _ options: Options) {
        if let current, current.node === node, current.options == options { current.sendSource(); return }
        current?.end(heard: true)
        if options.window {
            #if os(macOS)
            let host = PictureInPictureHost(node, options)
            host.window = PictureInPictureWindow(panel: options.presentation == "panel", title: options.title ?? ExactEnv.appName, width: options.width, height: options.height) { [weak host] text in host?.heard(text) }
            current = host
            host.sendSource()
            #else
            refuse(node, "a floating window is the Mac's: this platform floats no window over other apps")
            #endif
            return
        }
        guard let exports else { return refuse(node, "the video module is not in this build") }
        let host = PictureInPictureHost(node, options)
        let callback: FloatCallback = { context, bytes, count in
            guard let context, let bytes else { return }
            let text = String(decoding: UnsafeBufferPointer(start: bytes, count: count), as: UTF8.self)
            let host = Unmanaged<PictureInPictureHost>.fromOpaque(context).takeUnretainedValue()
            DispatchQueue.main.async { host.heard(text) }
        }
        guard let handle = exports.create(Unmanaged.passUnretained(host).toOpaque(), callback) else {
            return refuse(node, "picture in picture is not available on this platform")
        }
        host.handle = handle
        current = host
        host.sendSource()
    }

    /// The request failed: a journal line, and the image's `pictureinpictureerror` with the
    /// reason, as HTML's promise rejects.
    static func refuse(_ node: NodeView, _ why: String) {
        node.presenter?.session?.log("requestPictureInPicture \"\(node.props["id"] ?? "")\" refused: \(why)")
        dispatch(node, "pictureinpictureerror", payload: why)
    }

    /// `exitPictureInPicture()`: the floating image leaves the window and hears it.
    static func exit(_ session: ExactSession?) {
        guard let current else { session?.log("exitPictureInPicture ignored: no image is in picture in picture"); return }
        current.end(heard: true)
    }

    /// The image's `src` changed: its new picture floats.
    static func changed(_ node: NodeView) { if let current, current.node === node { current.sendSource() } }
    /// The image left the document: its window closes, with no event, as there is no node to hear one.
    static func gone(_ node: NodeView) { if let current, current.node === node { current.end(heard: false) } }

    private func sendSource() {
        guard let node, let src = node.imageSource, src != source else { return }
        #if os(macOS)
        let window = self.window
        #else
        let window: AnyObject? = nil
        #endif
        guard window != nil || handle != nil else { return }
        source = src
        // A file the image names, as it resolves one: the app's own (`app:/`) or a bundled asset.
        let url = src.hasPrefix("app:/") ? AppFiles.url(src) : URL(string: src)?.scheme == nil ? node.presenter?.session?.app.resolveAsset(src) : nil
        guard let path = url?.path else {
            // Before the window opens, an unreadable picture fails the request; after, the window keeps the last.
            if entered { node.presenter?.session?.log("requestPictureInPicture: \(src) is not a file the window can read") }
            else { end(heard: false); Self.refuse(node, "\(src) is not a file the window can read") }
            return
        }
        #if os(macOS)
        if let window { return window.frame(path) }
        #endif
        guard let handle, let exports = Self.exports else { return }
        Array(path.utf8).withUnsafeBufferPointer { exports.frame(handle, $0.baseAddress, $0.count) }
    }

    private func heard(_ text: String) {
        guard Self.current === self, let node else { return }
        if text == "enter" { entered = true; Self.dispatch(node, "enterpictureinpicture") }
        else if text == "leave" { end(heard: true) }
        else { end(heard: false); Self.refuse(node, String(text.dropFirst("error ".count))) }
    }

    private func end(heard: Bool) {
        if Self.current === self { Self.current = nil }
        if let handle { Self.exports?.destroy(handle) }
        handle = nil
        #if os(macOS)
        window?.close()
        window = nil
        #endif
        if heard, entered, let node { Self.dispatch(node, "leavepictureinpicture") }
        entered = false
    }

    private static func dispatch(_ node: NodeView, _ event: String, payload: String = "") {
        guard node.handlers.contains(event), let session = node.presenter?.session else { return }
        session.apply(session.runtime.media(node.id, event: event, payload: payload, now: session.now()))
    }
}

extension Presenter {
    /// `requestPictureInPicture(id)` on the `image` with that HTML `id`; its
    /// options follow it as `(presentation, title, width, height)`.
    func pictureInPicture(_ args: [Any]) {
        let id = args.first as? String ?? ""
        let node = views.values.sorted(by: { $0.id < $1.id }).first(where: { $0.props["id"] == id })
        guard let node else { session?.log("requestPictureInPicture \"\(id)\" refused: no live node with that id"); return }
        guard node.kind == "image" else { return PictureInPictureHost.refuse(node, "not an image") }
        PictureInPictureHost.request(node, .init(args))
    }
}
