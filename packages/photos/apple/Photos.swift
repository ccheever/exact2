// @exact/photos on Apple (LLP 1133.019): the photo library read and added
// to. Under the agent it asks PhotoKit for nothing and answers from the
// agent's library (AgentLibrary.swift, §10); the runner holds the
// permission tickets and lays the status it keeps over each answer.
//
// - A photo reaches the app as an id, a picture (`photos:<id>`, which an
//   `img` draws through the host's raster loader, D2) or a copy in
//   `app:/tmp/photos/` (emptied when the module is first made in a launch).
// - A photo that leaves what the app may read (the selection changed, the
//   access narrowed) has its pictures revoked: every `img` showing it is
//   emptied and fires `error` (D2).
// - `managePhotoSelection` is iOS's alone; macOS has no limited picker.
import ExactPackage
import Foundation
import ImageIO

public final class PhotosModule: ExactPackage {
    let context: ExactPackageContext
    private let library = AgentLibrary()
    private var copies = 0
    /// Emptied once a launch.
    private static var emptied = false

    public init(context: ExactPackageContext) {
        self.context = context
        if !Self.emptied, let tmp = context.file("app:/tmp/photos/x") {
            Self.emptied = true
            try? FileManager.default.removeItem(at: tmp.deletingLastPathComponent())
        }
    }

    private func say(_ line: String) { context.log("photos: \(line)") }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        guard context.agent else { return reply(["error": "NotSupportedError", "message": "the library is read under the agent only in this build"]) }
        switch name {
        case "photoLibraryStatus": reply(status())
        case "photoAssets": reply(["assets": assets(args)])
        case "copyPhotoAsset": copy(args, reply)
        case "addToPhotoLibrary": add(args, reply)
        case "managePhotoSelection":
            #if os(macOS)
            reply(["error": "NotSupportedError"])
            #else
            reply(status())
            #endif
        default: reply(["error": "NotSupportedError"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "photoLibraryStatus" && context.agent ? status() : ["error": "NotSupportedError"]
    }

    /// Under the agent: the access the runner's status gives and the
    /// selection; a photo that left what the app may read is revoked.
    public func preferred(_ values: [String: Any]) {
        let before = library.visible
        if let access = (values["photoLibraryStatus"] as? [String: Any])?["access"] as? String { library.access = access }
        library.selection = (values["selection"] as? [String]) ?? AgentLibrary.defaultSelection
        let after = library.visible
        for id in before.subtracting(after).sorted() {
            say("revoked photos:\(id)")
            context.revokePicture("photos:\(id)")
        }
        if before != after { context.changed("photos.photoAssets") }
    }

    public func permissions(_ states: [String: String]) {
        // Denied reading: nothing is readable, whatever the status said before.
        if states["photos"] == "denied" { preferred(["photoLibraryStatus": ["access": "none"]]) }
    }

    public func image(_ source: String, width: Int, height: Int, fit: String, owned: Bool, reply: @escaping (CGImage?, String?) -> Void) {
        guard context.agent else { return reply(nil, "NotSupportedError") }
        let id = String(source.dropFirst("photos:".count))
        guard let asset = library.asset(id) else {
            say("picture \(id) at \(width)x\(height): NotFoundError")
            return reply(nil, "NotFoundError")
        }
        DispatchQueue.global(qos: .userInitiated).async { [library] in
            let image = library.picture(asset, width: width, height: height, fit: fit)
            reply(image, image == nil ? "UnknownError" : nil)
        }
        say("picture \(id) at \(width)x\(height) \(fit)")
    }

    // MARK: the agent's answers (§10)

    private func status() -> [String: Any] {
        ["available": true, "state": "prompt", "access": library.access, "addState": "prompt", "apple": NSNull(), "android": NSNull()]
    }

    private func assets(_ args: [String: Any]) -> [[String: Any]] {
        let limit = (args["limit"] as? NSNumber)?.intValue ?? 60
        return library.newest(limit).map { a in
            ["id": a.id, "mediaType": "image", "width": a.width, "height": a.height, "creationDate": a.created,
             "favorite": false, "picture": "photos:\(a.id)",
             "apple": ["mediaSubtypes": [String](), "burstIdentifier": NSNull(), "hasAdjustments": false], "android": NSNull()]
        }
    }

    private func copy(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        guard let id = args["asset"] as? String, let asset = library.asset(id) else { return reply(["error": "NotFoundError"]) }
        let ext = asset.file.pathExtension
        let path: String
        if let p = args["path"] as? String, !p.isEmpty { path = p.hasSuffix(".\(ext)") ? p : "\(p).\(ext)" } else { copies += 1; path = "app:/tmp/photos/\(copies).\(ext)" }
        guard let url = context.file(path), let data = try? Data(contentsOf: asset.file) else { return reply(["error": "UnknownError"]) }
        do {
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try data.write(to: url)
        } catch { return reply(["error": "UnknownError"]) }
        say("copied \(id) to \(path) sha256 \(AgentLibrary.sha256(data))")
        reply(["path": path, "name": asset.file.lastPathComponent, "type": asset.type, "size": data.count,
               "width": asset.width, "height": asset.height, "duration": NSNull(), "android": NSNull()])
    }

    private func add(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        guard let path = args["path"] as? String, let url = context.file(path), let data = try? Data(contentsOf: url) else {
            return reply(["error": "NotFoundError"])
        }
        guard let directory = context.file("app:/tmp/photos-library/x")?.deletingLastPathComponent(),
              let asset = library.add(data, directory: directory, now: context.now()) else { return reply(["error": "DataError"]) }
        say("added \(asset.id) from \(path) sha256 \(AgentLibrary.sha256(data)) \(asset.width)x\(asset.height) \(asset.type)")
        context.changed("photos.photoAssets")
        // The new photo's id where the app may read it (D6): not under add-only access.
        reply(["asset": library.visible.contains(asset.id) ? asset.id : NSNull()])
    }
}
