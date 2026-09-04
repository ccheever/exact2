// The update store's Swift face (LLP 1026 D9/D11; LLP 1030 D7; LLP 1030.000
// §4 stage 4): the C entries of `exact.h`'s update section, per process —
// open at launch, the selection's assets for the resolver, first pixel, the
// check on the library's own thread, the staged plan's bytes for an
// activation. No networking here: the library fetches over its own
// transport (the executor's `NSURLSession`) and reports through `done`;
// this hops to the main thread and hands `ExactApp` the line.
import CExact
import Foundation

enum Updates {
    private static let api = exact_delivery_api()
    static var linked: Bool { api != nil }
    /// What the store selected for this launch: the entry (nil for entry
    /// zero), its seq, and its plan and assets paths (empty for entry zero).
    struct Selection {
        let entry: String?
        let seq: Int
        let plan: String
        let assets: String
    }

    private static func read(_ len: UInt32) -> Data { Data(bytes: api!.pointee.output(), count: Int(len)) }

    private static func write(_ text: String) -> Int {
        let data = Data(text.utf8)
        guard !data.isEmpty, let p = api!.pointee.input(data.count) else { _ = api!.pointee.input(0); return 0 }
        data.withUnsafeBytes { src in if let base = src.baseAddress { p.update(from: base.assumingMemoryBound(to: UInt8.self), count: data.count) } }
        return data.count
    }

    /// Open the store under the platform's Application Support directory
    /// (the app's container on iOS, `~/Library/Application Support` on
    /// macOS) — the library puts it at `exact/<app id>/update` — with
    /// `assets` as what the binary embeds by name. A refusal is one stderr
    /// line, and the app runs on the embedded facts.
    static func open(assets: URL) -> Bool {
        guard linked else { return false }
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first?.path ?? NSTemporaryDirectory()
        guard let payload = try? JSONSerialization.data(withJSONObject: ["base": base, "assets": assets.path]) else { return false }
        let n = write(String(decoding: payload, as: UTF8.self))
        let len = api!.pointee.open(n)
        if len == 0 { return true }
        FileHandle.standardError.write(Data("exact update: \(String(decoding: read(len), as: UTF8.self))\n".utf8))
        return false
    }

    static func selection() -> Selection {
        let len = api!.pointee.select()
        let obj = (try? JSONSerialization.jsonObject(with: read(len)) as? [String: Any]) ?? [:]
        return Selection(entry: obj["entry"] as? String, seq: obj["seq"] as? Int ?? 0, plan: obj["plan"] as? String ?? "", assets: obj["assets"] as? String ?? "")
    }

    /// First pixel: the selection that booted is good (LLP 1026 D11).
    static func bootSucceeded() { api?.pointee.boot_succeeded() }

    /// Start the check on the library's thread; `ExactApp.updateChecked`
    /// gets the line on the main thread. False when a check already runs or
    /// no store is open.
    static func check() -> Bool { api?.pointee.check(done, nil) == 0 }

    private static let done: ExactUpdateDoneFn = { _, line, len in
        let text = line.map { String(decoding: Data(bytes: $0, count: len), as: UTF8.self) } ?? ""
        DispatchQueue.main.async { ExactApp.shared.updateChecked(text) }
    }

    /// The staged plan's bytes, or nil when nothing is staged.
    static func activate() -> Data? {
        let len = api!.pointee.activate()
        return len == 0 ? nil : read(len)
    }
}
