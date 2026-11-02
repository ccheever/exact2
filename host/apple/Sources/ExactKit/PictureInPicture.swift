// @ref LLP 1042 §9. `requestPictureInPicture(id)`: an `image` floats in the platform's
// picture-in-picture window, which shows each picture its `src` names next, until
// `exitPictureInPicture()` or the window's own close; a failure is the image's
// `pictureinpictureerror`. AVKit stays in the video arm
// (`host/apple/videoarm/PictureInPicture.swift`); this side follows the node.
// `presentation="window"` or `"panel"` (LLP 1042 §9.1) floats it in Exact's own window on
// macOS instead (`Mac/PictureInPictureWindowMac.swift`), which needs no arm.
import Foundation

private typealias FloatCallback = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void

/// An image floating in a window. The process has one picture-in-picture window, as a document
/// has one picture-in-picture element: AVKit's panel or Exact's browser window (`"window"`). A
/// request for another image moves it there, and the first hears it leave. Panels (`"panel"`) are
/// windows of their own, one per image, as a page opens a window per preview.
final class PictureInPictureHost {
    private typealias Create = @convention(c) (UnsafeMutableRawPointer?, FloatCallback?) -> UnsafeMutableRawPointer?
    private typealias Frame = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void
    private typealias Destroy = @convention(c) (UnsafeMutableRawPointer?) -> Void
    /// The image in the one picture-in-picture window.
    private static var current: PictureInPictureHost?
    /// The images floating each in a panel of its own, in the order they were asked for.
    private static var panels: [PictureInPictureHost] = []
    private static var all: [PictureInPictureHost] { (current.map { [$0] } ?? []) + panels }
    private weak var node: NodeView?
    private var handle: UnsafeMutableRawPointer?
    private var source: String?
    private var entered = false
    let options: Options
    #if os(macOS)
    /// Exact's window, where the request asked for it: the arm's handle stays nil.
    private(set) var window: PictureInPictureWindow?
    #endif

    /// The request's options (LLP 1042 §9.1): `presentation=` and how Exact's window opens.
    struct Options: Equatable {
        /// `"window"` or `"panel"`, Exact's window; nil, the platform's.
        var presentation: String?
        var title: String?
        var width: Double?
        var height: Double?
        var window: Bool { presentation != nil }
        var panel: Bool { presentation == "panel" }
        init(_ args: [Any]) {
            presentation = args.count > 1 ? (args[1] as? String).flatMap { ["window", "panel"].contains($0) ? $0 : nil } : nil
            title = args.count > 2 ? args[2] as? String : nil
            width = args.count > 3 ? (args[3] as? NSNumber)?.doubleValue : nil
            height = args.count > 4 ? (args[4] as? NSNumber)?.doubleValue : nil
        }
    }

    private init(_ node: NodeView, _ options: Options) { self.node = node; self.options = options }

    /// The images floating now, each with its host: the agent reads and presses their windows.
    static var floating: [(node: NodeView, host: PictureInPictureHost)] {
        all.compactMap { host in host.entered ? host.node.map { ($0, host) } : nil }
    }

    /// The arm's three exports, from the video arm's library.
    private static let exports: (create: Create, frame: Frame, destroy: Destroy)? = {
        guard let library = VideoArmLibrary.handle,
              let create = dlsym(library, "exact_pip_create"), let frame = dlsym(library, "exact_pip_frame"),
              let destroy = dlsym(library, "exact_pip_destroy") else { return nil }
        return (unsafeBitCast(create, to: Create.self), unsafeBitCast(frame, to: Frame.self), unsafeBitCast(destroy, to: Destroy.self))
    }()

    static func request(_ node: NodeView, _ options: Options) {
        let floats = all.first { $0.node === node }
        if let floats, floats.options == options { floats.sendSource(); return }
        // An image floats in one window: asked for another, it leaves the one it is in.
        floats?.end(heard: true)
        if options.window {
            #if os(macOS)
            let title = options.title ?? ExactEnv.appName
            let host = PictureInPictureHost(node, options)
            if options.panel {
                panels.append(host)
            } else if let previous = current, let window = previous.window, !window.titled, window.isOpen {
                // As Chromium's: the open window shows the next image's pictures where it stands. It
                // changes hands before the first image hears it leave.
                window.follow(title: title) { [weak host] text in host?.heard(text) }
                host.window = window
                current = host
                previous.window = nil
                previous.end(heard: true)
                host.sendSource()
                return
            } else {
                current?.end(heard: true)
                current = host
            }
            host.window = PictureInPictureWindow(panel: options.panel, title: title, width: options.width, height: options.height) { [weak host] text in host?.heard(text) }
            host.sendSource()
            #else
            refuse(node, "a floating window is the Mac's: this platform floats no window over other apps")
            #endif
            return
        }
        current?.end(heard: true)
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

    /// `exitPictureInPicture()`: the image in the picture-in-picture window leaves it and hears it,
    /// as HTML's `document.exitPictureInPicture()`; a panel stays, as a page's other windows do.
    static func exit(_ session: ExactSession?) {
        guard let current else { session?.log("exitPictureInPicture ignored: no image is in the picture-in-picture window"); return }
        current.end(heard: true)
    }

    /// The image's `src` changed: its new picture floats.
    static func changed(_ node: NodeView) { all.first { $0.node === node }?.sendSource() }
    /// The image left the document: its window closes, with no event, as there is no node to hear one.
    static func gone(_ node: NodeView) { all.first { $0.node === node }?.end(heard: false) }

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
        guard Self.all.contains(where: { $0 === self }), let node else { return }
        if text == "enter" { entered = true; Self.dispatch(node, "enterpictureinpicture") }
        else if text == "leave" { end(heard: true) }
        else { end(heard: false); Self.refuse(node, String(text.dropFirst("error ".count))) }
    }

    /// The image leaves its window, which closes.
    private func end(heard: Bool) {
        if Self.current === self { Self.current = nil }
        Self.panels.removeAll { $0 === self }
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
