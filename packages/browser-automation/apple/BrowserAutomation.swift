// @exact/browser-automation on Apple (LLP 1020.002 D9): each name goes to the
// iframe arm (`exact_web_automation`), which holds the frames. The runner has
// admitted the call under `browser.automation` before it reaches here. A frame
// is found by its `id` among the frames of this session's app files: the arm
// knows each frame by the temporary folder its session's module writes, the
// one `context.file` resolves `app:/tmp/` in, so a session never reaches
// another's frames. An arm not yet loaded holds no frame: NotFoundError.
import ExactPackage
import Foundation

public final class BrowserAutomationModule: ExactPackage {
    private typealias Done = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias Automate = @convention(c) (UnsafePointer<UInt8>?, UInt32, UnsafeMutableRawPointer?, Done?) -> Void
    private final class Pending { let reply: ([String: Any]) -> Void; init(_ r: @escaping ([String: Any]) -> Void) { reply = r } }
    private let context: ExactPackageContext

    public init(context: ExactPackageContext) { self.context = context }

    /// The arm, if this process already loaded it (`RTLD_NOLOAD` never loads it).
    private static func automate() -> Automate? {
        #if os(macOS)
        let path = Bundle.main.executableURL?.deletingLastPathComponent().appendingPathComponent("libexact_web.dylib").path ?? ""
        #else
        let directory = Bundle.main.privateFrameworksPath ?? Bundle.main.bundlePath
        let wrapped = directory + "/ExactWeb.framework/ExactWeb"
        let path = FileManager.default.fileExists(atPath: wrapped) ? wrapped : directory + "/libexact_web.dylib"
        #endif
        guard let library = dlopen(path, RTLD_NOW | RTLD_NOLOAD), let symbol = dlsym(library, "exact_web_automation") else { return nil }
        return unsafeBitCast(symbol, to: Automate.self)
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        let id = args["id"] as? String ?? ""
        guard let owner = context.file("app:/tmp/.browser-automation")?.deletingLastPathComponent().standardizedFileURL.path,
              let automate = Self.automate() else {
            return reply(["error": "NotFoundError", "message": "\(name): no live iframe with id \(id)"])
        }
        var request = args
        request["op"] = name
        // The files are this package's to resolve, never the caller's to name.
        request["pathFile"] = nil
        request["destinationFolderFile"] = nil
        request["owner"] = owner
        // A path option is the app's own file (`app:/`), admitted by the runner against the app's `fs.write` lines.
        for option in ["path", "destinationFolder"] {
            guard let path = args[option] as? String else { continue }
            guard let file = context.file(path) else { return reply(["error": "SyntaxError", "message": "\(name): \(path) names no app file"]) }
            request["\(option)File"] = file.path
        }
        guard JSONSerialization.isValidJSONObject(request), let body = try? JSONSerialization.data(withJSONObject: request) else {
            return reply(["error": "SyntaxError", "message": "\(name): its arguments are not JSON"])
        }
        let pending = Unmanaged.passRetained(Pending(reply)).toOpaque()
        let done: Done = { context, bytes, length in
            let pending = Unmanaged<Pending>.fromOpaque(context!).takeRetainedValue()
            let data = Data(UnsafeBufferPointer(start: bytes, count: Int(length)))
            let answer = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? ["error": "UnknownError", "message": "the web arm's answer was not JSON"]
            pending.reply(answer)
        }
        body.withUnsafeBytes { automate($0.bindMemory(to: UInt8.self).baseAddress, UInt32(body.count), pending, done) }
    }
}
