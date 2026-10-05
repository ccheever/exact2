// Shared indexes for platform chrome; text runs never enter these sets.
/// Navigation, menus, segments, the toolbar, shortcuts and context panels
/// each used to read every view on every batch to find their own — a
/// navigation stack, a popover, a tab list — and a reader's rows are none of
/// those. A scrolling list applies a batch many times a second, so the passes
/// were a fifth of its long frames (LLP 1044 F4). They visit what this names.
struct ChromeIndex {
    static let keys = ["navigationBack", "inert", "popover", "popovertarget",
                       "contextTarget", "toolbarPlacement", "accessibilityKeyShortcuts",
                       "commandfor", "swipeContent", "id", "accessibilityLive", "autofocus", "listStyle",
                       "accessibilityModal", "accessibilityDescribedBy", "accessibilityLabelledBy"]
    /// Props a pass reads for one value. Every list row has a role and every
    /// `main` or `header` a tag, so these are indexed by that value, never by
    /// presence.
    static let values = [("role:tablist", "accessibilityRole", "tablist"), ("tag:dialog", "semanticTag", "dialog"),
                         ("type:checkbox", "type", "checkbox"), ("type:radio", "type", "radio"), ("type:select", "type", "select"),
                         ("type:range", "type", "range"), ("type:date", "type", "date"),
                         ("type:time", "type", "time"), ("type:datetime-local", "type", "datetime-local"),
                         ("type:button", "type", "button")]
    private var byKey: [String: Set<UInt32>] = [:]
    /// The views carrying each `id` value, and the values whose carriers
    /// changed since `takeChangedNames` — what resolves a name without
    /// reading every named view.
    private(set) var named: [String: Set<UInt32>] = [:]
    private var nameOf: [UInt32: String] = [:]
    private var changedNames = Set<String>()
    mutating func takeChangedNames() -> Set<String> {
        defer { changedNames.removeAll(keepingCapacity: true) }
        return changedNames
    }
    private mutating func rename(_ id: UInt32, _ name: String?) {
        let old = nameOf[id]
        guard old != name else { return }
        if let old {
            named[old]?.remove(id)
            if named[old]?.isEmpty == true { named[old] = nil }
            changedNames.insert(old)
        }
        if let name { named[name, default: []].insert(id); changedNames.insert(name) }
        nameOf[id] = name
    }

    mutating func note(_ id: UInt32, props: [String: String]) {
        rename(id, props["id"])
        for key in Self.keys {
            if props[key] != nil { byKey[key, default: []].insert(id) }
            else if let index = byKey.index(forKey: key), byKey.values[index].contains(id) {
                byKey.values[index].remove(id)
            }
        }
        for (name, key, value) in Self.values {
            if props[key] == value { byKey[name, default: []].insert(id) }
            else if let index = byKey.index(forKey: name), byKey.values[index].contains(id) {
                byKey.values[index].remove(id)
            }
        }
    }
    mutating func forget(_ id: UInt32) {
        rename(id, nil)
        // Only the member sets change; dictionary keys and indices stay fixed.
        for index in byKey.indices where byKey.values[index].contains(id) { byKey.values[index].remove(id) }
    }
    func ids(_ key: String) -> Set<UInt32> { byKey[key] ?? [] }
    /// Whether anything that can hide a view or make it inert is mounted:
    /// only the passes over these ever set either.
    var hidesOrInerts: Bool {
        ["navigationBack", "inert", "tag:dialog", "popover", "toolbarPlacement", "role:tablist"]
            .contains { !(byKey[$0]?.isEmpty ?? true) }
    }
}
