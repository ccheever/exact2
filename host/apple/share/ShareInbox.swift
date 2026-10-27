// @ref LLP 1133.024 D4 — the share extension's inbox: what a share leaves in
// the App Group for the app to claim. The extension links no Exact runtime;
// this record's versioned shape is its one contract with the app.
//
// `<group>/shares/<uuid>/` holds each attachment as `<n>.<ext>` (a counter
// and the type's extension, never the sender's name), streamed by
// `loadFileRepresentation` and never decoded, then `share.json` last, by
// write-and-rename: a directory without it is incomplete and the claim
// ignores it.
import Foundation
import UniformTypeIdentifiers

/// What the bake told the extension in its Info.plist.
public struct ShareTargetInfo {
    /// The App Group both the app and the extension can write.
    public var group: String
    /// The app's id: the Darwin notification is `<id>.shares`.
    public var appId: String
    /// Which text members the app maps (`title`, `text`, `url`).
    public var mapped: Set<String>
    /// Each files member: its name and the UTIs it accepts.
    public var members: [(name: String, types: [String])]

    public init(group: String, appId: String, mapped: Set<String>, members: [(name: String, types: [String])]) {
        self.group = group
        self.appId = appId
        self.mapped = mapped
        self.members = members
    }

    /// Read from the extension's Info.plist (`host/apple/share.mjs` writes these keys).
    public init?(bundle: Bundle) {
        guard let group = bundle.object(forInfoDictionaryKey: "ExactAppGroup") as? String,
              let appId = bundle.object(forInfoDictionaryKey: "ExactAppIdentifier") as? String else { return nil }
        let mapped = Set(bundle.object(forInfoDictionaryKey: "ExactShareParams") as? [String] ?? [])
        let members = (bundle.object(forInfoDictionaryKey: "ExactShareMembers") as? [[String: Any]] ?? []).compactMap { m -> (String, [String])? in
            guard let name = m["name"] as? String else { return nil }
            return (name, m["types"] as? [String] ?? [])
        }
        self.init(group: group, appId: appId, mapped: mapped, members: members)
    }

    /// The first files member that accepts `type`, and the accepted UTI it conforms to.
    func member(for type: UTType) -> (name: String, accepted: String)? {
        for m in members {
            for t in m.types {
                if let accepted = UTType(t), type.conforms(to: accepted) { return (m.name, t) }
            }
        }
        return nil
    }
}

/// One share being written: the sheet's text and what the attachments became.
public final class ShareInboxWriter {
    /// The 64 MB an outgoing share also refuses (LLP 1125 D12).
    public static let cap: Int64 = 64 << 20
    public let info: ShareTargetInfo
    public let directory: URL
    private var files: [[String: String]] = []
    private var dropped: [String: Int] = [:]
    private var url: String?
    private let lock = NSLock()

    /// A writer into `container/shares/<uuid>/`; nil when the container cannot be made.
    public init?(info: ShareTargetInfo, container: URL) {
        self.info = info
        directory = container.appendingPathComponent("shares", isDirectory: true).appendingPathComponent(UUID().uuidString, isDirectory: true)
        do { try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true) } catch { return nil }
    }

    /// The App Group's container, or nil when the extension is not entitled to it.
    public static func container(_ info: ShareTargetInfo) -> URL? {
        FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: info.group)
    }

    /// Takes every attachment of `items`: a web link as `url` (when mapped), each file an accepted type matches,
    /// streamed to disk; what no member accepts, or over the cap, is counted as dropped. `done` runs once all have.
    public func take(_ items: [NSExtensionItem], done: @escaping () -> Void) {
        let group = DispatchGroup()
        var n = 0
        for item in items {
            for provider in item.attachments ?? [] {
                // A web link: the claim gives it to `url`, or to `text` when only that is mapped.
                if provider.hasItemConformingToTypeIdentifier(UTType.url.identifier), !provider.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) {
                    group.enter()
                    _ = provider.loadObject(ofClass: URL.self) { [weak self] value, _ in
                        if let value, ["http", "https"].contains(value.scheme?.lowercased()) { self?.locked { self?.url = self?.url ?? value.absoluteString } }
                        group.leave()
                    }
                    continue
                }
                // Text that is not a file (neither a file URL nor a name of its own) is the sheet's text, not an attachment.
                if provider.hasItemConformingToTypeIdentifier(UTType.plainText.identifier), provider.suggestedName == nil,
                   !provider.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) { continue }
                guard let (member, accepted) = provider.registeredTypeIdentifiers.lazy.compactMap({ UTType($0) }).compactMap({ self.info.member(for: $0).map { m in (m.name, m.accepted) } }).first else {
                    count(dropped: info.members.first?.name ?? "")
                    continue
                }
                let type = provider.registeredTypeIdentifiers.first { UTType($0)?.conforms(to: UTType(accepted) ?? .data) == true } ?? accepted
                n += 1
                let index = n
                group.enter()
                let load = { [weak self] (sender: String?) in
                    _ = provider.loadFileRepresentation(forTypeIdentifier: type) { [weak self] file, _ in
                        defer { group.leave() }
                        guard let self, let file else { self?.count(dropped: member); return }
                        self.store(file, index: index, type: UTType(type) ?? .data, member: member, suggested: provider.suggestedName ?? sender)
                    }
                }
                // A file with no suggested name (a file URL's provider) is named by its URL, as the sender has it;
                // the representation's copy has a name of its own.
                if provider.suggestedName == nil, provider.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) {
                    _ = provider.loadObject(ofClass: URL.self) { url, _ in load(url?.lastPathComponent) }
                } else {
                    load(nil)
                }
            }
        }
        group.notify(queue: .main, execute: done)
    }

    /// Copies `file` (the provider's temporary copy, gone after the callback) into the share under a stored name.
    private func store(_ file: URL, index: Int, type: UTType, member: String, suggested: String?) {
        let size = (try? file.resourceValues(forKeys: [.fileSizeKey]).fileSize).map(Int64.init) ?? 0
        guard size <= Self.cap else { return count(dropped: member) }
        let ext = type.preferredFilenameExtension ?? file.pathExtension
        let stored = ext.isEmpty ? "\(index)" : "\(index).\(ext)"
        do {
            try FileManager.default.copyItem(at: file, to: directory.appendingPathComponent(stored))
        } catch {
            return count(dropped: member)
        }
        let name = suggested.map { s in s.contains(".") || ext.isEmpty ? s : "\(s).\(ext)" } ?? file.lastPathComponent
        locked { files.append(["file": stored, "name": name, "type": type.preferredMIMEType ?? "application/octet-stream", "member": member]) }
    }

    private func count(dropped member: String) { locked { dropped[member, default: 0] += 1 } }
    private func locked(_ body: () -> Void) { lock.lock(); body(); lock.unlock() }

    /// Writes `share.json` last, by write-and-rename, then pings the app.
    public func finish(title: String?, text: String?) throws {
        let ordered = files.sorted { Int($0["file"]!.split(separator: ".")[0]) ?? 0 < Int($1["file"]!.split(separator: ".")[0]) ?? 0 }
        var record: [String: Any] = ["v": 1, "files": ordered, "dropped": dropped]
        if info.mapped.contains("title"), let title, !title.isEmpty { record["title"] = title }
        if info.mapped.contains("text"), let text, !text.isEmpty { record["text"] = text }
        if let url { record["url"] = url }
        let data = try JSONSerialization.data(withJSONObject: record, options: [.sortedKeys])
        let partial = directory.appendingPathComponent("share.json.partial")
        try data.write(to: partial)
        try FileManager.default.moveItem(at: partial, to: directory.appendingPathComponent("share.json"))
        CFNotificationCenterPostNotification(CFNotificationCenterGetDarwinNotifyCenter(), CFNotificationName("\(info.appId).shares" as CFString), nil, nil, true)
    }

    /// Removes what was written (the person cancelled, or the write failed).
    public func discard() { try? FileManager.default.removeItem(at: directory) }
}
