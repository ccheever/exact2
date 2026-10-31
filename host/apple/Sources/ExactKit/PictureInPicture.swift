// @ref LLP 1042 §9. `requestPictureInPicture(id)`: an `image` floats in the platform's
// picture-in-picture window, which shows each picture its `src` names next. AVKit stays in
// the video arm (`host/apple/videoarm/PictureInPicture.swift`); this side follows the node.
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

    private init(_ node: NodeView) { self.node = node }

    /// The arm's three exports, from the video arm's library.
    private static let exports: (create: Create, frame: Frame, destroy: Destroy)? = {
        guard let library = VideoArmLibrary.handle,
              let create = dlsym(library, "exact_pip_create"), let frame = dlsym(library, "exact_pip_frame"),
              let destroy = dlsym(library, "exact_pip_destroy") else { return nil }
        return (unsafeBitCast(create, to: Create.self), unsafeBitCast(frame, to: Frame.self), unsafeBitCast(destroy, to: Destroy.self))
    }()

    static func request(_ node: NodeView) {
        if let current, current.node === node { current.sendSource(); return }
        current?.end(heard: true)
        guard let exports else { return refuse(node, "the video module is not in this build") }
        let host = PictureInPictureHost(node)
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

    private static func refuse(_ node: NodeView, _ why: String) {
        node.presenter?.session?.log("requestPictureInPicture \"\(node.props["id"] ?? "")\" refused: \(why)")
    }

    /// The image's `src` changed: its new picture floats.
    static func changed(_ node: NodeView) { if let current, current.node === node { current.sendSource() } }
    /// The image left the document: its window closes, with no event, as there is no node to hear one.
    static func gone(_ node: NodeView) { if let current, current.node === node { current.end(heard: false) } }

    private func sendSource() {
        guard let node, let handle, let exports = Self.exports, let src = node.imageSource, src != source else { return }
        source = src
        // A file the image names, as it resolves one: the app's own (`app:/`) or a bundled asset.
        let url = src.hasPrefix("app:/") ? AppFiles.url(src) : URL(string: src)?.scheme == nil ? node.presenter?.session?.app.resolveAsset(src) : nil
        guard let path = url?.path else { return Self.refuse(node, "\(src) is not a file this window can read") }
        Array(path.utf8).withUnsafeBufferPointer { exports.frame(handle, $0.baseAddress, $0.count) }
    }

    private func heard(_ text: String) {
        guard Self.current === self, let node else { return }
        if text == "enter" { entered = true; dispatch(node, "enterpictureinpicture") }
        else if text == "leave" { end(heard: true) }
        else { Self.refuse(node, String(text.dropFirst("error ".count))); end(heard: false) }
    }

    private func end(heard: Bool) {
        if Self.current === self { Self.current = nil }
        if let handle { Self.exports?.destroy(handle) }
        handle = nil
        if heard, entered, let node { dispatch(node, "leavepictureinpicture") }
        entered = false
    }

    private func dispatch(_ node: NodeView, _ event: String) {
        guard node.handlers.contains(event), let session = node.presenter?.session else { return }
        session.apply(session.runtime.media(node.id, event: event, payload: "", now: session.now()))
    }
}

extension Presenter {
    /// `requestPictureInPicture(id)` on the `image` with that HTML `id`.
    func pictureInPicture(_ args: [Any]) {
        let id = args.first as? String ?? ""
        let node = views.values.sorted(by: { $0.id < $1.id }).first(where: { $0.props["id"] == id })
        guard let node, node.kind == "image" else {
            session?.log("requestPictureInPicture \"\(id)\" refused: \(node == nil ? "no live node with that id" : "not an image")")
            return
        }
        PictureInPictureHost.request(node)
    }
}
