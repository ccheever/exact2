// The content index's mirror and Spotlight (LLP 1133.023 D4, D9, D15 item 2).
//
// The mirror is what the app set, kept in its container: Spotlight's index
// cannot be read back, and the system asks the app to index everything again
// when it has lost its index. Each change is written to the mirror, marked
// pending, before Spotlight is told, and unmarked when Spotlight answers, so
// a crash between the two leaves work the next launch finishes. The mirror
// answers the app's `contentIndexEntries`, the reindex delegate, a search
// result's continuation and the generated entity queries, with no session.
//
// The module side (packages/search-index/apple) and the generated intents
// reach it through `NotificationCenter`, each body an `NSMutableDictionary`
// this answers in place:
//
// - `exact.contentIndex.set` `{kind, entries}`, `.add` `{entry}`,
//   `.delete` `{id}`: `error` set to the web's name on a refusal.
// - `exact.contentIndex.entries`: `entries`, all of them.
// - `exact.contentIndex.query` `{kind, ids?, matching?}`: `entries`.
// - `exact.contentIndex.open` `{url}`: opens an entry's location.
import Foundation
#if canImport(CoreSpotlight) && !os(tvOS)
import CoreSpotlight
import os
import UniformTypeIdentifiers
#endif

public enum ExactContentIndex {
    /// D4's limits: past them a change refuses `QuotaExceededError`.
    static let most = 5_000, textBytes = 1_024
    private struct Mirror: Codable {
        var entries: [[String: JSONValue]] = []
        /// Keys whose change Spotlight has not acknowledged.
        var pending: [String] = []
    }
    private static var mirror: Mirror?
    private static var installed = false

    /// The mirror's file, in the app's container; a test names its own and forgets what was read.
    static var file: URL = {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        return support.appendingPathComponent(Bundle.main.bundleIdentifier ?? "app").appendingPathComponent("exact-content-index.json")
    }() { didSet { mirror = nil } }

    static func install() {
        guard !installed else { return }
        installed = true
        let center = NotificationCenter.default
        func on(_ name: String, _ handle: @escaping (NSMutableDictionary) -> Void) {
            center.addObserver(forName: Notification.Name("exact.contentIndex." + name), object: nil, queue: nil) { note in
                guard let body = note.object as? NSMutableDictionary else { return }
                if Thread.isMainThread { handle(body) } else { DispatchQueue.main.sync { handle(body) } }
            }
        }
        on("set") { body in
            let entries = (body["entries"] as? [[String: Any]] ?? []).map(JSONValue.fields)
            if let error = set(kind: body["kind"] as? String, entries) { body["error"] = error }
        }
        on("add") { body in
            if let error = add(JSONValue.fields(body["entry"] as? [String: Any] ?? [:])) { body["error"] = error }
        }
        on("delete") { body in delete(id: body["id"] as? String ?? "") }
        on("entries") { body in body["entries"] = load().entries.map(JSONValue.plainFields) }
        on("query") { body in
            body["entries"] = query(kind: body["kind"] as? String ?? "", ids: body["ids"] as? [String], matching: body["matching"] as? String)
        }
        on("open") { body in if let url = body["url"] as? String { open(url) } }
        #if canImport(CoreSpotlight) && !os(tvOS)
        index.indexDelegate = Reindexer.shared
        DispatchQueue.main.async { finishPending() }
        #endif
    }

    private static func load() -> Mirror {
        if let mirror { return mirror }
        let read = (try? Data(contentsOf: file)).flatMap { try? JSONDecoder().decode(Mirror.self, from: $0) } ?? Mirror()
        mirror = read
        return read
    }
    private static func save(_ m: Mirror) {
        mirror = m
        try? FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        if let data = try? JSONEncoder().encode(m) { try? data.write(to: file, options: .atomic) }
    }

    /// An entry's identity: its kind and its id (D2).
    static func key(_ e: [String: JSONValue]) -> String { "\(e["kind"]?.string ?? ""):\(e["id"]?.string ?? "")" }

    private static func refusal(_ entries: [[String: JSONValue]]) -> String? {
        if entries.count > most { return "QuotaExceededError" }
        for e in entries {
            guard e["id"]?.string?.isEmpty == false, e["title"]?.string?.isEmpty == false, e["url"]?.string?.hasPrefix("/") == true else { return "TypeError" }
            let text = ["title", "description", "url"].compactMap { e[$0]?.string }.joined()
            if text.utf8.count > textBytes { return "QuotaExceededError" }
        }
        return nil
    }

    /// The entries of `kind` replaced by `entries`, in their order: Spotlight
    /// told the difference.
    static func set(kind: String?, _ entries: [[String: JSONValue]]) -> String? {
        var m = load()
        let tagged = entries.map { e -> [String: JSONValue] in var e = e; if let kind { e["kind"] = .string(kind) }; return e }
        let others = m.entries.filter { $0["kind"]?.string != kind }
        if let refused = refusal(others + tagged) { return refused }
        let was = Dictionary(m.entries.map { (key($0), $0) }, uniquingKeysWith: { a, _ in a })
        let now = Dictionary(tagged.map { (key($0), $0) }, uniquingKeysWith: { _, b in b })
        let changed = tagged.filter { was[key($0)] != $0 }
        let gone = m.entries.filter { $0["kind"]?.string == kind && now[key($0)] == nil }.map(key)
        m.entries = others + tagged
        publish(&m, changed: changed, gone: gone)
        return nil
    }

    static func add(_ entry: [String: JSONValue]) -> String? {
        var m = load()
        let k = key(entry)
        var entries = m.entries.filter { key($0) != k }
        entries.append(entry)
        if let refused = refusal(entries) { return refused }
        m.entries = entries
        publish(&m, changed: [entry], gone: [])
        return nil
    }

    static func delete(id: String) {
        var m = load()
        let gone = m.entries.filter { $0["id"]?.string == id }.map(key)
        guard !gone.isEmpty else { return }
        m.entries.removeAll { $0["id"]?.string == id }
        publish(&m, changed: [], gone: gone)
    }

    /// The mirror's entries of `kind` by id, or matching `text` in the title
    /// or description (case- and diacritic-insensitive, in the person's
    /// locale), or all of them in the app's order (D9).
    static func query(kind: String, ids: [String]?, matching text: String?) -> [[String: Any]] {
        let ofKind = load().entries.filter { $0["kind"]?.string == kind }
        var found = ofKind
        if let ids {
            let byId = Dictionary(ofKind.map { ($0["id"]?.string ?? "", $0) }, uniquingKeysWith: { a, _ in a })
            found = ids.compactMap { byId[$0] }
        } else if let text, !text.isEmpty {
            found = ofKind.filter { e in
                ["title", "description"].contains { e[$0]?.string?.range(of: text, options: [.caseInsensitive, .diacriticInsensitive], locale: .current) != nil }
            }
        }
        return found.map(JSONValue.plainFields)
    }

    /// The location of Spotlight's item `identifier`, from the mirror; nil
    /// when the mirror no longer has it (the app then opens at its start).
    public static func location(ofItem identifier: String) -> String? {
        load().entries.first { key($0) == identifier }?["url"]?.string
    }

    /// Open an entry's location in the app's session: a launch location
    /// before its boot, a navigation after (LLP 1133.023 D1).
    public static func open(_ url: String) {
        guard let session = ExactApp.shared.sessions.first(where: { $0.state != .destroyed }) else { return }
        _ = session.openLocation(url)
    }

    // MARK: Spotlight

    #if canImport(CoreSpotlight) && !os(tvOS)
    /// One index named for the app, readable after the first unlock as the app's files are (D4).
    static let index = CSSearchableIndex(name: Bundle.main.bundleIdentifier ?? "app", protectionClass: .completeUntilFirstUserAuthentication)

    private static func item(_ e: [String: JSONValue]) -> CSSearchableItem {
        let attributes = CSSearchableItemAttributeSet(contentType: .content)
        attributes.title = e["title"]?.string
        attributes.contentDescription = e["description"]?.string
        if case .object(let apple)? = e["apple"] {
            if case .array(let words)? = apple["keywords"] { attributes.keywords = words.compactMap(\.string) }
            if case .number(let rating)? = apple["rating"] { attributes.rating = NSNumber(value: rating) }
        }
        let item = CSSearchableItem(uniqueIdentifier: key(e), domainIdentifier: e["kind"]?.string, attributeSet: attributes)
        // An entry ends when the app removes it, not when the system expires it (D4).
        item.expirationDate = .distantFuture
        return item
    }

    private static func publish(_ m: inout Mirror, changed: [[String: JSONValue]], gone: [String]) {
        let keys = changed.map(key) + gone
        guard !keys.isEmpty else { return save(m) }
        m.pending = Array(Set(m.pending).union(keys))
        save(m)
        tell(changed: changed, gone: gone)
        parametersChanged()
    }

    /// Spotlight told, and each key unmarked once it answers.
    private static func tell(changed: [[String: JSONValue]], gone: [String]) {
        let done = { (keys: [String]) in
            DispatchQueue.main.async {
                var m = load()
                m.pending.removeAll { keys.contains($0) }
                save(m)
            }
        }
        if !changed.isEmpty {
            index.indexSearchableItems(changed.map(item)) { error in
                if let error { ExactTools.log(nil, "contentIndex: Spotlight refused \(changed.count) entries: \(error.localizedDescription)") } else { done(changed.map(key)) }
            }
        }
        if !gone.isEmpty {
            index.deleteSearchableItems(withIdentifiers: gone) { error in
                if let error { ExactTools.log(nil, "contentIndex: Spotlight kept \(gone.count) removed entries: \(error.localizedDescription)") } else { done(gone) }
            }
        }
    }

    /// What a crash left between the mirror and Spotlight, finished.
    private static func finishPending() {
        let m = load()
        guard !m.pending.isEmpty else { return }
        let byKey = Dictionary(m.entries.map { (key($0), $0) }, uniquingKeysWith: { a, _ in a })
        tell(changed: m.pending.compactMap { byKey[$0] }, gone: m.pending.filter { byKey[$0] == nil })
    }

    /// The system lost the index, or some items: the mirror again (D4).
    final class Reindexer: NSObject, CSSearchableIndexDelegate {
        static let shared = Reindexer()
        /// What the system asked, for a person checking it on a device (`packages/search-index/test/checklist.md`).
        static let log = Logger(subsystem: "dev.exact", category: "search")
        func searchableIndex(_ searchableIndex: CSSearchableIndex, reindexAllSearchableItemsWithAcknowledgementHandler acknowledgementHandler: @escaping () -> Void) {
            DispatchQueue.main.async {
                let all = ExactContentIndex.load().entries
                Self.log.log("reindex all: \(all.count) entries from the mirror")
                ExactContentIndex.index.indexSearchableItems(all.map(ExactContentIndex.item)) { _ in acknowledgementHandler() }
            }
        }
        func searchableIndex(_ searchableIndex: CSSearchableIndex, reindexSearchableItemsWithIdentifiers identifiers: [String], acknowledgementHandler: @escaping () -> Void) {
            DispatchQueue.main.async {
                let wanted = Set(identifiers)
                let some = ExactContentIndex.load().entries.filter { wanted.contains(ExactContentIndex.key($0)) }
                Self.log.log("reindex \(identifiers.count) items: \(some.count) in the mirror")
                ExactContentIndex.index.indexSearchableItems(some.map(ExactContentIndex.item)) { _ in acknowledgementHandler() }
            }
        }
    }

    /// A search result opened (`CSSearchableItemActionType`): its location, or nil for another activity.
    public static func location(of activity: NSUserActivity) -> String? {
        guard activity.activityType == CSSearchableItemActionType,
              let id = activity.userInfo?[CSSearchableItemActivityIdentifier] as? String else { return nil }
        return location(ofItem: id) ?? "/"
    }
    #else
    private static func publish(_ m: inout Mirror, changed: [[String: JSONValue]], gone: [String]) { save(m) }
    public static func location(of activity: NSUserActivity) -> String? { nil }
    #endif

    /// App Shortcuts whose phrases name a parameter learn the index's values
    /// (D9), through the generated module's hooks when it has any.
    private static func parametersChanged() {
        guard let hooks = NSClassFromString("ExactIntentsHooks") as? NSObject.Type else { return }
        _ = hooks.perform(NSSelectorFromString("parametersChanged"))
    }
}

/// A JSON value the mirror keeps, comparable so a change is told only when it differs.
enum JSONValue: Codable, Equatable {
    case string(String), number(Double), bool(Bool), null
    indirect case array([JSONValue]), object([String: JSONValue])

    static func from(_ any: Any?) -> JSONValue {
        switch any {
        case let s as String: return .string(s)
        case let n as NSNumber where CFGetTypeID(n) == CFBooleanGetTypeID(): return .bool(n.boolValue)
        case let n as NSNumber: return .number(n.doubleValue)
        case let a as [Any]: return .array(a.map(from))
        case let o as [String: Any]: return .object(o.mapValues(from))
        default: return .null
        }
    }
    static func fields(_ o: [String: Any]) -> [String: JSONValue] { o.mapValues(from) }
    static func plainFields(_ o: [String: JSONValue]) -> [String: Any] { o.mapValues(\.plain) }
    var plain: Any {
        switch self {
        case .string(let s): return s
        case .number(let n): return n
        case .bool(let b): return b
        case .null: return NSNull()
        case .array(let a): return a.map(\.plain)
        case .object(let o): return o.mapValues(\.plain)
        }
    }
    var string: String? { if case .string(let s) = self { return s }; return nil }

    init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() { self = .null }
        else if let b = try? c.decode(Bool.self) { self = .bool(b) }
        else if let n = try? c.decode(Double.self) { self = .number(n) }
        else if let s = try? c.decode(String.self) { self = .string(s) }
        else if let a = try? c.decode([JSONValue].self) { self = .array(a) }
        else { self = .object(try c.decode([String: JSONValue].self)) }
    }
    func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        switch self {
        case .string(let s): try c.encode(s)
        case .number(let n): try c.encode(n)
        case .bool(let b): try c.encode(b)
        case .null: try c.encodeNil()
        case .array(let a): try c.encode(a)
        case .object(let o): try c.encode(o)
        }
    }
}
