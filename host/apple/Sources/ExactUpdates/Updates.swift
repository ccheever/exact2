// The update store's Swift face (LLP 1026 D9/D11; LLP 1030 D7; LLP 1030.000
// §4 stage 4): the C entries of `exact.h`'s update section, per process —
// open at launch, the selection's assets for the resolver, first pixel, the
// check on the library's own thread, the staged plan's bytes for an
// activation. No networking here: the library fetches over its own
// transport (the executor's `NSURLSession`) and reports through `done`;
// this hops to the main thread and hands `ExactApp` the line.
import ExactKit
import CExact
import Foundation

enum Updates {
    static var completed: ((String) -> Void)?
    private static let api = exact_delivery_api()
    static var linked: Bool { api != nil }
    /// What the store selected for this launch: the entry (nil for entry
    /// zero), its seq, and its plan and assets paths (empty for entry zero).
    struct Selection {
        let token: UInt64
        let entry: String
        let seq: UInt64
        let plan: Data
        let assets: [String]
    }

    private static func read(_ len: UInt32) -> Data {
        guard len > 0, let api, let bytes = api.pointee.output() else { return Data() }
        return Data(bytes: bytes, count: Int(len))
    }

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
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first?.path ?? NSTemporaryDirectory()
        guard let payload = try? JSONSerialization.data(withJSONObject: ["base": base, "assets": assets.path]) else { return false }
        let n = write(String(decoding: payload, as: UTF8.self))
        let len = api!.pointee.open(n)
        if len == 0 { return true }
        FileHandle.standardError.write(Data("exact update: \(String(decoding: read(len), as: UTF8.self))\n".utf8))
        return false
    }

    private static func selection(_ len: UInt32) -> Selection? {
        let obj = (try? JSONSerialization.jsonObject(with: read(len)) as? [String: Any]) ?? [:]
        guard let token = (obj["token"] as? NSNumber)?.uint64Value, token != 0,
              let entry = obj["entry"] as? String else { return nil }
        let plan = read(api!.pointee.plan(token))
        return Selection(token: token, entry: entry, seq: (obj["seq"] as? NSNumber)?.uint64Value ?? 0,
                         plan: plan, assets: obj["assets"] as? [String] ?? [])
    }

    static func selection() -> Selection? { selection(api!.pointee.select()) }
    static func prepare() -> Selection? { selection(api!.pointee.prepare()) }
    static func commit(_ selection: Selection) -> Bool {
        let len = api!.pointee.commit(selection.token)
        if len != 0 { fputs("exact update: \(String(decoding: read(len), as: UTF8.self))\n", stderr) }
        return len == 0
    }
    static func discard(_ selection: Selection) { api!.pointee.discard(selection.token) }
    static func refuse(_ token: UInt64, reason: String) {
        _ = api!.pointee.refuse(token, write(reason))
    }
    static func asset(_ selection: Selection, name: String) -> Result<Data?, NSError> {
        let data = read(api!.pointee.asset(selection.token, write(name)))
        switch data.first {
        case 0: return .success(nil)
        case 1: return .success(Data(data.dropFirst()))
        default: return .failure(NSError(domain: "ExactAsset", code: 1, userInfo: [NSLocalizedDescriptionKey: String(decoding: data.dropFirst(), as: UTF8.self)]))
        }
    }

    /// Only the generation which drew can bless the running selection.
    static func started(_ token: UInt64) { api!.pointee.started(token) }
    static func bootSucceeded(_ token: UInt64) { api!.pointee.boot_succeeded(token) }

    /// Start the check on the library's thread; `ExactApp.updateChecked`
    /// gets the line on the main thread. False when a check already runs or
    /// no store is open.
    static func check() -> Bool { api!.pointee.check(done, nil) == 0 }

    private static let done: ExactUpdateDoneFn = { _, line, len in
        let text = line.map { String(decoding: Data(bytes: $0, count: len), as: UTF8.self) } ?? ""
        DispatchQueue.main.async { Updates.completed?(text) }
    }

}

/// The app-owned updating composition. Core ExactKit owns preparation and
/// presentation; this target alone owns its store, checks and boot marks.
public final class ExactUpdates: ExactAppLifecycle {
    private weak var app: ExactApp?
    private let storeOpen: Bool
    private var firstPixelSeen = false
    public private(set) var status: String?

    @discardableResult
    public static func install(on app: ExactApp) -> ExactUpdates {
        precondition(Updates.linked, "the updating composition requires an update-capable app archive")
        precondition(app.sessions.isEmpty && app.lifecycle == nil, "install the app composition before its first session")
        let owner = ExactUpdates(app: app)
        app.lifecycle = owner
        return owner
    }

    private init(app: ExactApp) {
        self.app = app
        storeOpen = Updates.open(assets: app.assetRoot)
        if storeOpen, let selected = Updates.selection() { app.installInitial(generation(selected, app: app)) }
        Updates.completed = { [weak self] line in
            self?.status = line
            FileHandle.standardError.write(Data("exact update: \(line)\n".utf8))
            self?.app?.refreshDelivery()
        }
    }

    private func generation(_ selection: Updates.Selection, app: ExactApp) -> ExactGeneration {
        let assets = AssetResolver(root: app.assetRoot, names: selection.assets) { name in
            try Updates.asset(selection, name: name).get()
        }
        return ExactGeneration(plan: selection.plan, assets: assets, token: selection.token)
    }

    public func generationStarted(_ app: ExactApp, token: UInt64) { if storeOpen { Updates.started(token) } }
    public func initialGenerationRefused(_ app: ExactApp, token: UInt64, reason: String) {
        if storeOpen { Updates.refuse(token, reason: reason) }
    }
    public func firstPixel(_ app: ExactApp, token: UInt64) {
        guard storeOpen else { return }
        Updates.bootSucceeded(token)
        guard !firstPixelSeen else { return }
        firstPixelSeen = true
        if ExactEnv.agentMode, ExactEnv.environment["EXACT_UPDATE_ORIGIN"] == nil { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) { [weak self] in self?.check() }
    }

    public func check() {
        guard storeOpen else { return }
        if !Updates.check() { FileHandle.standardError.write(Data("exact update: a check is already running\n".utf8)) }
    }

    @discardableResult
    public func activate() -> Bool {
        guard storeOpen, let app, let selected = Updates.prepare() else { return false }
        let accepted = app.applyGeneration(generation(selected, app: app), label: "update", commit: { Updates.commit(selected) })
        if !accepted { Updates.discard(selected) }
        return accepted
    }

    public func handleCommand(_ name: String, app: ExactApp) -> Bool {
        switch name {
        case "deliveryCheck": check(); return true
        case "deliveryActivate": DispatchQueue.main.async { [weak self] in self?.activate() }; return true
        default: return false
        }
    }
}
