// Pictures of a package's `img` scheme (LLP 1133.019 D2, D11 item 1): a
// source such as `photos:<id>`, whose scheme an interface declares
// (`images`), is never opened as a file. The raster loader asks the runner
// to admit it when a view loads it (a read: never a ticket), and a refusal
// is that view's unresolved image. An admitted source is read as a download
// is: a worker's inspection hands it to the main thread, which asks the
// bound implementation through the module's `image` entry with the box's
// device pixels and `object-fit`; its decoded image comes back on any thread
// and goes through the loader's metadata, plan and budget as a decoded input
// (`RasterInput.picture`). An implementation may say a picture changed (the
// host loads it again) or revoke it (the host clears it from every view and
// its cache, cancels its loads, drops a later reply, and fires `error`).
import CoreGraphics
import ExactCore
import Foundation

/// Why a package picture did not come: the implementation's error code, or
/// the runner's refusal, as the image's `error` says it.
struct PackagePictureFailure: Error, CustomStringConvertible {
    let description: String
}

/// The admitted sources and their revocations, shared by a loader's main
/// thread and its workers.
final class PackagePictures: @unchecked Sendable {
    private let lock = NSLock()
    /// Each admitted source's extension and whether it is let through as the
    /// app's own item only.
    private var admitted: [String: (extension: String, owned: Bool)] = [:]
    /// Each source's generation: a revocation moves it, and a reply asked
    /// under an older one is dropped.
    private var generations: [String: Int] = [:]

    /// The schemes a host owns, which are never a package's (`HOST_SCHEMES`,
    /// contract/cli/src/declaration.rs).
    private static let hostSchemes: Set<String> = ["app", "doc", "symbol", "data", "http", "https", "blob", "file", "asset"]

    /// Whether `source` names a scheme a package may serve.
    static func mayServe(_ source: String) -> Bool {
        guard let colon = source.firstIndex(of: ":") else { return false }
        let scheme = source[..<colon]
        guard let first = scheme.first, first.isLetter, first.isLowercase,
              scheme.allSatisfy({ ($0.isLetter && $0.isLowercase) || $0.isNumber || $0 == "-" }) else { return false }
        return !hostSchemes.contains(String(scheme))
    }

    /// The runner's admission of `source` when a view loads it, main thread:
    /// nil to load it (admitted, or no package's scheme), else why not.
    func admit(_ source: String, session: ExactSession) -> String? {
        guard Self.mayServe(source), let answer = session.runtime.picture(source) else { return nil }
        guard let name = answer["extension"] as? String else {
            return answer["message"] as? String ?? answer["error"] as? String ?? "refused"
        }
        lock.lock(); admitted[source] = (name, answer["owned"] as? Bool ?? false); lock.unlock()
        return nil
    }

    /// The extension serving `source`, once admitted; any thread.
    func serving(_ source: String) -> (extension: String, owned: Bool)? {
        lock.lock(); defer { lock.unlock() }; return admitted[source]
    }

    func generation(_ source: String) -> Int {
        lock.lock(); defer { lock.unlock() }; return generations[source] ?? 0
    }

    /// `source` was revoked: its admission is forgotten and a reply in
    /// flight is dropped.
    func revoke(_ source: String) {
        lock.lock(); admitted[source] = nil; generations[source, default: 0] += 1; lock.unlock()
    }

    /// Ask `session`'s implementation for `source` at `width` by `height`
    /// device pixels; `done` once, on any thread. Main thread.
    func ask(_ source: String, session: ExactSession, width: Int, height: Int, fit: String,
             done: @escaping (Result<RasterInput, Error>) -> Void) {
        guard let (name, owned) = serving(source) else { return done(.failure(RasterFailure.unresolved)) }
        guard let (module, entry) = session.natives.imageEntry() else {
            return done(.failure(PackagePictureFailure(description: "this build's module serves no pictures")))
        }
        let asked = generation(source)
        let reply = PackagePictureReply { [weak self] image, error in
            guard let self, self.generation(source) == asked else {
                return done(.failure(PackagePictureFailure(description: "revoked")))
            }
            guard let image else { return done(.failure(PackagePictureFailure(description: error ?? "UnknownError"))) }
            done(.success(RasterInput.picture(image)))
        }
        let request: [String: Any] = ["extension": name, "source": source, "width": width, "height": height,
                                      "fit": fit, "owned": owned]
        let json = (try? JSONSerialization.data(withJSONObject: request)) ?? Data()
        let ctx = Unmanaged.passRetained(reply).toOpaque()
        json.withUnsafeBytes { entry(module, $0.bindMemory(to: UInt8.self).baseAddress, json.count, ctx, packageImageDone) }
    }
}

typealias PackageImageFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int, UnsafeMutableRawPointer?,
                                           (@convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void)?) -> Void

private final class PackagePictureReply {
    let then: (CGImage?, String?) -> Void
    init(_ then: @escaping (CGImage?, String?) -> Void) { self.then = then }
}

/// The module's answer to `image`, from any thread: taken there, as a
/// download's body is.
private let packageImageDone: @convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void = { ctx, image, error, len in
    guard let ctx else { return }
    let reply = Unmanaged<PackagePictureReply>.fromOpaque(ctx).takeRetainedValue()
    let picture = image.map { Unmanaged<CGImage>.fromOpaque($0).takeRetainedValue() }
    let code = error.map { String(decoding: UnsafeBufferPointer(start: $0, count: len), as: UTF8.self) }
    reply.then(picture, code)
}

extension NodeView {
    /// A revoked picture leaves the view: its raster goes, and the box is
    /// drawn empty (D2), as no failed load of an ordinary image does.
    func withdrawPicture() {
        raster = nil
        applyImageLayer()
        #if os(macOS)
        needsDisplay = true
        #else
        setNeedsDisplay()
        #endif
    }
}
