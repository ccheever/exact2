// @exact/contacts on Apple (LLP 1133.018): the person's contacts, picked
// through the system's picker or read from the store under access. Under the
// agent it asks the Contacts framework for nothing and answers from the
// agent's address book (AgentContacts.swift, §10); the runner holds the
// permission and sheet tickets and answers the status.
//
// - A pick is a copy (D4): no `id`, icons written to
//   `app:/tmp/picked/contacts/` for the app to keep.
// - A store read answers a bounded page (D6) whose cursor is the last
//   contact's id: a page after a contact the store no longer shows fails
//   `InvalidStateError`, `cursor-stale`. Its icons are the store's, written
//   to `app:/tmp/contacts/` and deleted with their answer: when the reader is
//   released, when its page is answered again, at the first call of a
//   launch, and when access narrows to none.
// - A change is announced, coalesced (D7): while the re-asks one
//   announcement caused are in flight, further changes set a mark, and one
//   more announcement follows them landing.
// - `showContactAccessPicker` is iOS's alone; macOS has no limited access.
import ExactPackage
import Foundation

public final class ContactsModule: ExactPackage {
    let context: ExactPackageContext
    private let book = AgentContacts()
    private var picks = 0, pages = 0
    /// The live names read and not released, which observing follows.
    private var readers: Set<String> = []
    /// Each store reader's icon directory, by its arguments.
    private var iconDirectories: [String: String] = [:]
    /// Coalescing (D7): an announcement's re-asks in flight; a change since.
    private var awaiting = false, dirty = false, rounds = 0
    /// The agent's words last seen, so each new value acts once.
    private var store = "contacts.json", notify: Int?
    private static var emptied = false

    public init(context: ExactPackageContext) {
        self.context = context
        if !Self.emptied {
            Self.emptied = true
            emptyStoreIcons()
        }
    }

    private func say(_ line: String) { context.log("contacts: \(line)") }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        guard context.agent else { return reply(["error": "NotSupportedError", "message": "the store is read under the agent only in this build"]) }
        switch name {
        case "getContactProperties": reply(["properties": AgentContacts.properties])
        case "contactsStatus": reply(status())
        case "selectContacts": reply(pick(args, preferred as? [String: Any]))
        case "queryContacts":
            readers.insert(name)
            reply(page(args))
            if awaiting { DispatchQueue.main.async { [weak self] in self?.landed() } }
        case "showContactAccessPicker":
            #if os(macOS)
            reply(["error": "NotSupportedError"])
            #else
            reply(share(preferred as? [String: Any]))
            #endif
        default: reply(["error": "NotSupportedError"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "contactsStatus" && context.agent ? status() : ["error": "NotSupportedError"]
    }

    public func release(_ name: String) {
        guard readers.remove(name) != nil, readers.isEmpty else { return }
        say("no reader: observing stops")
    }

    public func release(_ name: String, args: [String: Any]) {
        if let dir = iconDirectories.removeValue(forKey: Self.key(args)) { remove(dir) }
    }

    /// Under the agent: the access the runner's status gives, the store the
    /// agent names, and a burst of `notify` changes.
    public func preferred(_ values: [String: Any]) {
        let access = (values["contactsStatus"] as? [String: Any])?["access"] as? String ?? book.access
        let file = values["store"] as? String ?? "contacts.json"
        let narrowed = access != book.access
        book.access = access
        if !["full", "limited"].contains(access) { emptyStoreIcons() }
        if file != store {
            store = file
            if book.load(file) { say("store \(file)") } else { say("store \(file): no such fixture") }
            changedStore()
        } else if narrowed {
            changedStore()
        }
        let burst = (values["notify"] as? NSNumber)?.intValue
        if let n = burst, burst != notify {
            say("notify \(n)")
            for _ in 0..<n { changedStore() }
        }
        notify = burst
    }

    public func permissions(_ states: [String: String]) {
        if states["contacts"] == "denied" { preferred(["contactsStatus": ["access": "none"], "store": store, "notify": notify as Any]) }
    }

    public func foreground(_ shown: Bool) {
        if shown { changedStore() }
    }

    // MARK: announcing (D7)

    /// The store changed: announced, or marked while an announcement's re-asks are in flight.
    private func changedStore() {
        guard !readers.isEmpty else { return }
        if awaiting { dirty = true; return }
        announce()
    }

    private func announce() {
        awaiting = true
        rounds += 1
        say("announced (round \(rounds))")
        context.changed("contacts.queryContacts")
        context.changed("contacts.contactsStatus")
        // A round no reader asks again still ends.
        let round = rounds
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) { [weak self] in
            if self?.rounds == round { self?.landed() }
        }
    }

    private func landed() {
        guard awaiting else { return }
        awaiting = false
        if dirty {
            dirty = false
            announce()
        }
    }

    // MARK: the agent's answers (§10)

    private func status() -> [String: Any] {
        ["available": true, "state": "prompt", "access": book.access, "apple": NSNull(), "android": NSNull()]
    }

    private static func properties(_ args: [String: Any]) -> [String]? {
        let asked = args["properties"] as? [String] ?? []
        return !asked.isEmpty && asked.allSatisfy(AgentContacts.properties.contains) ? asked : nil
    }

    private func pick(_ args: [String: Any], _ sheet: [String: Any]?) -> [String: Any] {
        guard let properties = Self.properties(args) else { return ["error": "TypeError", "message": "properties is empty or names one the picker has not"] }
        guard let ids = sheet?["typed"] as? [String] else { return ["contacts": []] }
        picks += 1
        var out: [[String: Any]] = []
        for id in ids {
            guard let contact = book.contact(id) else { return ["error": "NotFoundError", "message": "no fixture contact \(id)"] }
            out.append(record(contact, properties, id: false, icons: "app:/tmp/picked/contacts/\(picks)"))
        }
        say("picked \(ids.joined(separator: ",")) (\(properties.joined(separator: ",")))")
        return ["contacts": out]
    }

    private func page(_ args: [String: Any]) -> [String: Any] {
        guard let properties = Self.properties(args) else { return ["error": "TypeError", "message": "properties is empty or names one the store has not"] }
        guard ["full", "limited"].contains(book.access) else { return ["error": "NotAllowedError"] }
        var list = book.visible
        if let ids = args["ids"] as? [String] {
            let asked = Set(ids)
            list = list.filter { asked.contains($0["id"] as? String ?? "") }
        }
        if let text = args["text"] as? String, !text.isEmpty { list = list.filter { AgentContacts.matches($0, text) } }
        if let after = args["after"] as? String {
            guard let i = list.firstIndex(where: { $0["id"] as? String == after }) else {
                return ["error": "InvalidStateError", "message": "cursor-stale"]
            }
            list = Array(list[(i + 1)...])
        }
        let limit = max(1, min(1000, (args["limit"] as? NSNumber)?.intValue ?? 100))
        let shown = Array(list.prefix(limit))
        // This reader's icons from its last page go with that answer.
        let key = Self.key(args)
        if let old = iconDirectories[key] { remove(old) }
        pages += 1
        let dir = "app:/tmp/contacts/\(pages)"
        if properties.contains("icon") { iconDirectories[key] = dir }
        let next: Any = list.count > limit ? shown.last?["id"] ?? NSNull() : NSNull()
        say("page queryContacts \(shown.count) of \(list.count) after \(args["after"] as? String ?? "start") (\(book.access))")
        return ["contacts": shown.map { record($0, properties, id: true, icons: dir) }, "next": next]
    }

    /// The access picker's answer: the contacts newly shared (iOS 18).
    private func share(_ sheet: [String: Any]?) -> [String: Any] {
        guard book.access == "limited", let ids = sheet?["typed"] as? [String] else { return ["contacts": []] }
        let seen = Set(book.visible.compactMap { $0["id"] as? String })
        let newly = ids.filter { book.contact($0) != nil && !seen.contains($0) }
        book.shared.formUnion(newly)
        say("shared \(newly.joined(separator: ","))")
        if !newly.isEmpty { changedStore() }
        return ["contacts": newly.map { ["id": $0, "name": NSNull(), "email": NSNull(), "tel": NSNull(), "address": NSNull(), "icon": NSNull()] }]
    }

    /// `contact` with the properties asked, its icons written under `icons`.
    private func record(_ contact: [String: Any], _ properties: [String], id: Bool, icons: String) -> [String: Any] {
        let cid = contact["id"] as? String ?? ""
        var out: [String: Any] = ["id": id ? cid : NSNull(), "name": NSNull(), "email": NSNull(), "tel": NSNull(), "address": NSNull(), "icon": NSNull()]
        for p in properties {
            guard p == "icon" else { out[p] = contact[p] ?? [Any](); continue }
            var paths: [String] = []
            for (i, name) in (contact["icon"] as? [String] ?? []).enumerated() {
                let path = "\(icons)/\(cid)-\(i).png"
                guard let bytes = book.icon(name), let url = context.file(path) else { continue }
                try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
                if (try? bytes.write(to: url)) != nil { paths.append(path) }
            }
            out["icon"] = paths
        }
        return out
    }

    private func remove(_ dir: String) {
        if let url = context.file("\(dir)/x")?.deletingLastPathComponent() { try? FileManager.default.removeItem(at: url) }
    }

    private func emptyStoreIcons() {
        remove("app:/tmp/contacts")
        iconDirectories = [:]
    }

    /// A reader's arguments as one key: its page's place included.
    private static func key(_ args: [String: Any]) -> String {
        (try? JSONSerialization.data(withJSONObject: args, options: [.sortedKeys])).map { String(decoding: $0, as: UTF8.self) } ?? ""
    }
}
