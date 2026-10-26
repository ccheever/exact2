// The agent's address book (LLP 1133.018 §10): what @exact/contacts reads
// and picks from under the agent, where it asks the Contacts framework for
// nothing. It is this checkout's `fixture/contacts.json` (a drive runs on
// this Mac or its simulators), or another file there the agent's `store`
// names. What the app may read of it is the access the runner's status
// gives: everything under `full`; under `limited` the fixture's `limited`
// list and what the access picker shared since.
import Foundation

final class AgentContacts {
    static let properties = ["address", "email", "icon", "name", "tel"]

    /// The fixtures, beside this file's directory in the checkout.
    private static let fixtures = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("fixture")

    private(set) var contacts: [[String: Any]] = []
    private var limited: [String] = []
    private var icons: [String: Data] = [:]
    /// Shared through the access picker in this session.
    var shared: Set<String> = []
    var access = "none"

    init() { _ = load("contacts.json") }

    /// Reads fixture file `name`; false where there is none.
    func load(_ name: String) -> Bool {
        guard !name.contains("/"), let data = try? Data(contentsOf: Self.fixtures.appendingPathComponent(name)),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return false }
        contacts = json["contacts"] as? [[String: Any]] ?? []
        limited = json["limited"] as? [String] ?? []
        icons = (json["icons"] as? [String: String] ?? [:]).compactMapValues { Data(base64Encoded: $0) }
        shared = []
        return true
    }

    func contact(_ id: String) -> [String: Any]? { contacts.first { $0["id"] as? String == id } }

    /// The contacts the app may read now, in the fixture's order.
    var visible: [[String: Any]] {
        switch access {
        case "full": return contacts
        case "limited":
            let ids = Set(limited).union(shared)
            return contacts.filter { ids.contains($0["id"] as? String ?? "") }
        default: return []
        }
    }

    /// A contact whose name, email or number holds `text`, ignoring case.
    static func matches(_ contact: [String: Any], _ text: String) -> Bool {
        let needle = text.lowercased()
        return ["name", "email", "tel"].contains { key in
            (contact[key] as? [String] ?? []).contains { $0.lowercased().contains(needle) }
        }
    }

    /// The bytes of icon `name`.
    func icon(_ name: String) -> Data? { icons[name] }
}
