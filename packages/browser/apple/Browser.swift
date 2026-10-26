// @exact/browser on Apple (LLP 1020.002 D4): `clearSiteData` clears a
// partition's website data store through the iframe arm that holds it
// (`exact_web_clear_site_data`). The arm loads at the first iframe, and
// every partition the session names is named by a mounted frame, so an arm
// not yet loaded means no partition was named: NotFoundError. The promise
// settles in WebKit's completion handler.
import ExactPackage
import Foundation

public final class BrowserModule: ExactPackage {
    private typealias Done = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias Clear = @convention(c) (UnsafePointer<UInt8>?, UInt32, UnsafeMutableRawPointer?, Done?) -> Void
    private final class Pending { let reply: ([String: Any]) -> Void; let answer: [String: Any]; init(_ r: @escaping ([String: Any]) -> Void, _ a: [String: Any]) { reply = r; answer = a } }

    public init(context: ExactPackageContext) {}

    /// The arm, if this process already loaded it (`RTLD_NOLOAD` never loads it).
    private static func clearFunction() -> Clear? {
        #if os(macOS)
        let path = Bundle.main.executableURL?.deletingLastPathComponent().appendingPathComponent("libexact_web.dylib").path ?? ""
        #else
        let directory = Bundle.main.privateFrameworksPath ?? Bundle.main.bundlePath
        let wrapped = directory + "/ExactWeb.framework/ExactWeb"
        let path = FileManager.default.fileExists(atPath: wrapped) ? wrapped : directory + "/libexact_web.dylib"
        #endif
        guard let library = dlopen(path, RTLD_NOW | RTLD_NOLOAD), let symbol = dlsym(library, "exact_web_clear_site_data") else { return nil }
        return unsafeBitCast(symbol, to: Clear.self)
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        guard name == "clearSiteData" else { return reply(["error": "NotSupportedError"]) }
        let partition = args["partition"] as? String ?? "", types = args["types"] as? [String] ?? []
        guard let clear = Self.clearFunction() else {
            return reply(["error": "NotFoundError", "message": "clearSiteData: this session has never named the partition \(partition)"])
        }
        guard let request = try? JSONSerialization.data(withJSONObject: ["partition": partition, "types": types]) else { return reply(["error": "SyntaxError"]) }
        let pending = Unmanaged.passRetained(Pending(reply, ["partition": partition, "types": types])).toOpaque()
        let done: Done = { context, bytes, length in
            let pending = Unmanaged<Pending>.fromOpaque(context!).takeRetainedValue()
            let text = String(decoding: UnsafeBufferPointer(start: bytes, count: Int(length)), as: UTF8.self)
            guard !text.isEmpty else { return pending.reply(pending.answer) }
            let (code, message) = text.firstIndex(of: ":").map { (String(text[..<$0]), String(text[text.index(after: $0)...]).trimmingCharacters(in: .whitespaces)) } ?? (text, text)
            pending.reply(["error": code, "message": message])
        }
        request.withUnsafeBytes { clear($0.bindMemory(to: UInt8.self).baseAddress, UInt32(request.count), pending, done) }
    }
}
