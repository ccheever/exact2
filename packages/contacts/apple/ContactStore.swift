// @exact/contacts' production path on Apple (LLP 1133.018 stage 2): the
// system's contact picker, and the person's address book under access.
//
// - The picker needs no access (D4): `CNContactPickerViewController` on iOS,
//   one contact or several; `CNContactPicker` on macOS, a popover from the
//   window's centre, one contact at a time. A pick is a copy: no `id`, icons
//   written to `app:/tmp/picked/contacts/`.
// - A store read asks for access while undetermined, runs off the main thread
//   (D6), and answers a page in the store's own order whose cursor is the
//   last contact's identifier: a page after a contact the store no longer
//   shows fails `InvalidStateError`, `cursor-stale`. Under iOS 18's limited
//   access the store holds only what the person shared.
// - `showContactAccessPicker` is iOS 18's `contactAccessPicker`, shown while
//   access is limited, answering the contacts newly shared.
// - `CNContactStoreDidChange` is a change of the store, which the module announces.
import Contacts
import Foundation
#if canImport(UIKit)
import ContactsUI
import SwiftUI
import UIKit
#else
import AppKit
import ContactsUI
#endif

final class ContactStore: NSObject {
    private let store = CNContactStore()
    private let say: (String) -> Void
    private let changed: () -> Void
    private let file: (String) -> URL?
    private let reads = DispatchQueue(label: "exact.contacts.reads", qos: .userInitiated)
    private var observing = false, picks = 0, pages = 0
    /// Each store reader's icon directory, by its arguments.
    private var directories: [String: String] = [:]
    private var picking: (properties: [String], multiple: Bool, reply: ([String: Any]) -> Void)?
    #if canImport(UIKit)
    private var single: SinglePick?
    #endif
    #if os(macOS)
    private var shownPicker: CNContactPicker?
    #endif

    init(log: @escaping (String) -> Void, changed: @escaping () -> Void, file: @escaping (String) -> URL?) {
        say = log
        self.changed = changed
        self.file = file
        super.init()
        ContactsModule.contactStore?(store)
    }

    static let properties = ["address", "email", "icon", "name", "tel"]

    // MARK: access

    private static var authorization: CNAuthorizationStatus { CNContactStore.authorizationStatus(for: .contacts) }

    func status() -> [String: Any] {
        // By raw value: `limited` (4) is iOS 18's.
        let (state, access, word): (String, String, String) = switch Self.authorization.rawValue {
        case 0: ("prompt", "none", "notDetermined")
        case 1: ("denied", "none", "restricted")
        case 2: ("denied", "none", "denied")
        case 3: ("granted", "full", "authorized")
        case 4: ("granted", "limited", "limited")
        default: ("unknown", "unknown", "notDetermined")
        }
        return ["available": true, "state": state, "access": access, "apple": ["authorizationStatus": word], "android": NSNull()]
    }

    var access: String { status()["access"] as? String ?? "none" }

    private func authorize(_ done: @escaping () -> Void) {
        guard Self.authorization == .notDetermined else { return done() }
        store.requestAccess(for: .contacts) { [weak self] granted, _ in
            DispatchQueue.main.async {
                self?.say("asked: \(granted ? "granted" : "not granted")")
                done()
                self?.changed()
            }
        }
    }

    private func observe() {
        guard !observing else { return }
        observing = true
        NotificationCenter.default.addObserver(self, selector: #selector(storeChanged), name: .CNContactStoreDidChange, object: nil)
    }

    func stopObserving() {
        guard observing else { return }
        observing = false
        NotificationCenter.default.removeObserver(self, name: .CNContactStoreDidChange, object: nil)
    }

    @objc private func storeChanged() { DispatchQueue.main.async { [self] in changed() } }

    // MARK: names

    func later(_ name: String, _ args: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        switch name {
        case "getContactProperties": reply(["properties": Self.properties])
        case "contactsStatus": reply(status())
        case "selectContacts": pick(args, reply)
        case "queryContacts": authorize { [self] in observe(); page(args, reply) }
        case "showContactAccessPicker": share(reply)
        default: reply(["error": "NotSupportedError"])
        }
    }

    private static func asked(_ args: [String: Any]) -> [String]? {
        let asked = args["properties"] as? [String] ?? []
        return !asked.isEmpty && asked.allSatisfy(properties.contains) ? asked : nil
    }

    private static func keys(_ properties: [String]) -> [CNKeyDescriptor] {
        var keys: [CNKeyDescriptor] = [CNContactIdentifierKey as CNKeyDescriptor]
        for p in properties {
            switch p {
            case "name": keys.append(CNContactFormatter.descriptorForRequiredKeys(for: .fullName))
            case "email": keys.append(CNContactEmailAddressesKey as CNKeyDescriptor)
            case "tel": keys.append(CNContactPhoneNumbersKey as CNKeyDescriptor)
            case "address":
                keys += [CNContactPostalAddressesKey, CNContactOrganizationNameKey].map { $0 as CNKeyDescriptor }
                keys.append(CNContactFormatter.descriptorForRequiredKeys(for: .fullName))
            case "icon": keys += [CNContactThumbnailImageDataKey, CNContactImageDataAvailableKey].map { $0 as CNKeyDescriptor }
            default: break
            }
        }
        return keys
    }

    private static func name(_ c: CNContact) -> String? {
        guard c.areKeysAvailable([CNContactFormatter.descriptorForRequiredKeys(for: .fullName)]) else { return nil }
        let full = CNContactFormatter.string(from: c, style: .fullName)
        return full?.isEmpty == false ? full : nil
    }

    /// `c` with the properties asked, its icon written under `icons` as the `n`th of its answer.
    private func record(_ c: CNContact, _ properties: [String], id: Bool, icons: String, n: Int) -> [String: Any] {
        var out: [String: Any] = ["id": id ? c.identifier : NSNull(), "name": NSNull(), "email": NSNull(), "tel": NSNull(), "address": NSNull(), "icon": NSNull()]
        for p in properties {
            switch p {
            case "name": out["name"] = Self.name(c).map { [$0] } ?? []
            case "email": out["email"] = c.isKeyAvailable(CNContactEmailAddressesKey) ? c.emailAddresses.map { $0.value as String } : []
            case "tel": out["tel"] = c.isKeyAvailable(CNContactPhoneNumbersKey) ? c.phoneNumbers.map(\.value.stringValue) : []
            case "address":
                let organization = c.isKeyAvailable(CNContactOrganizationNameKey) ? c.organizationName : ""
                out["address"] = c.isKeyAvailable(CNContactPostalAddressesKey) ? c.postalAddresses.map { a -> [String: Any] in
                    let v = a.value
                    return ["addressLine": v.street.split(separator: "\n").map(String.init), "city": v.city, "country": v.isoCountryCode.uppercased(),
                            "dependentLocality": v.subLocality, "organization": organization, "phone": "", "postalCode": v.postalCode,
                            "recipient": Self.name(c) ?? "", "region": v.state, "sortingCode": ""]
                } : []
            case "icon":
                var paths: [String] = []
                if c.isKeyAvailable(CNContactThumbnailImageDataKey), let data = c.thumbnailImageData {
                    let path = "\(icons)/\(n)-0.\(data.starts(with: [0x89, 0x50]) ? "png" : "jpg")"
                    if let url = file(path) {
                        try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
                        if (try? data.write(to: url)) != nil { paths.append(path) }
                    }
                }
                out["icon"] = paths
            default: break
            }
        }
        return out
    }

    // MARK: the picker (D4)

    private func pick(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        guard let properties = Self.asked(args) else { return reply(["error": "TypeError", "message": "properties is empty or names one the picker has not"]) }
        guard picking == nil else { return reply(["error": "InvalidStateError", "message": "a picker is showing"]) }
        let multiple = args["multiple"] as? Bool == true
        let apple = args["apple"] as? [String: Any]
        picking = (properties, multiple, reply)
        #if canImport(UIKit)
        guard let top = Self.topController() else {
            picking = nil
            return reply(["error": "InvalidStateError", "message": "no window to show the picker in"])
        }
        let shown = CNContactPickerViewController()
        if multiple {
            shown.delegate = self
        } else {
            let one = SinglePick { [weak self] in
                self?.single = nil
                self?.picked($0)
            }
            single = one
            shown.delegate = one
        }
        if let predicate = apple?["enablingPredicate"] as? String { shown.predicateForEnablingContact = NSPredicate(format: predicate) }
        if let keys = apple?["displayedPropertyKeys"] as? [String] { shown.displayedPropertyKeys = keys }
        let delegate = shown.delegate
        ContactsModule.contactPicker?(shown)
        if shown.delegate !== delegate {
            say("the contactPicker hatch changed the delegate, which the module owns (LLP 1133.018 D10)")
            shown.delegate = delegate
        }
        top.present(shown, animated: true)
        #else
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow, let view = window.contentView else {
            picking = nil
            return reply(["error": "InvalidStateError", "message": "no window to show the picker in"])
        }
        let shown = CNContactPicker()
        shown.delegate = self
        if let keys = apple?["displayedPropertyKeys"] as? [String] { shown.displayedKeys = keys }
        ContactsModule.contactPicker?(shown)
        if shown.delegate !== self {
            say("the contactPicker hatch changed the delegate, which the module owns (LLP 1133.018 D10)")
            shown.delegate = self
        }
        shownPicker = shown
        // No anchor yet (D13 item 1): the window's centre.
        shown.showRelative(to: NSRect(x: view.bounds.midX, y: view.bounds.midY, width: 1, height: 1), of: view, preferredEdge: .maxY)
        #endif
        say("picker shown (\(properties.joined(separator: ","))\(multiple ? ", multiple" : ""))")
    }

    private func picked(_ contacts: [CNContact]) {
        guard let (properties, _, reply) = picking else { return }
        picking = nil
        picks += 1
        let out = contacts.enumerated().map { record($1, properties, id: false, icons: "app:/tmp/picked/contacts/\(picks)", n: $0) }
        say("picked \(contacts.count)")
        reply(["contacts": out])
    }

    // MARK: the store (D6)

    private func page(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        guard let properties = Self.asked(args) else { return reply(["error": "TypeError", "message": "properties is empty or names one the store has not"]) }
        guard ["full", "limited"].contains(access) else { return reply(["error": "NotAllowedError"]) }
        let ids = args["ids"] as? [String], text = (args["text"] as? String).flatMap { $0.isEmpty ? nil : $0.lowercased() }
        let after = args["after"] as? String
        let limit = max(1, min(1000, (args["limit"] as? NSNumber)?.intValue ?? 100))
        // Matching a number or an email reads those keys whatever was asked.
        let keys = Self.keys(Array(Set(properties + (text == nil ? [] : ["name", "email", "tel"]))))
        // This reader's icons from its last page go with that answer.
        let key = Self.key(args)
        if let old = directories.removeValue(forKey: key) { remove(old) }
        pages += 1
        let icons = "app:/tmp/contacts/\(pages)"
        if properties.contains("icon") { directories[key] = icons }
        reads.async { [self] in
            let began = Date()
            let request = CNContactFetchRequest(keysToFetch: keys)
            request.sortOrder = .userDefault
            request.unifyResults = true
            if let ids { request.predicate = CNContact.predicateForContacts(withIdentifiers: ids) }
            var shown: [CNContact] = [], passed = after == nil, more = false, failure: [String: Any]?
            do {
                try store.enumerateContacts(with: request) { c, stop in
                    if !passed {
                        passed = c.identifier == after
                        return
                    }
                    if let text, !Self.matches(c, text) { return }
                    if shown.count == limit { more = true; stop.pointee = true; return }
                    shown.append(c)
                }
            } catch {
                failure = ["error": (error as NSError).code == CNError.authorizationDenied.rawValue ? "NotAllowedError" : "UnknownError", "message": error.localizedDescription]
            }
            if failure == nil, !passed { failure = ["error": "InvalidStateError", "message": "cursor-stale"] }
            let ms = Int(Date().timeIntervalSince(began) * 1000)
            DispatchQueue.main.async { [self] in
                if let failure { return reply(failure) }
                let next: Any = more ? shown.last?.identifier ?? NSNull() : NSNull()
                say("page queryContacts \(shown.count) after \(after ?? "start") in \(ms) ms (\(access))")
                reply(["contacts": shown.enumerated().map { record($1, properties, id: true, icons: icons, n: $0) }, "next": next])
            }
        }
    }

    /// No reader reads with `args` any longer: its icons go.
    func release(args: [String: Any]) {
        if let dir = directories.removeValue(forKey: Self.key(args)) { remove(dir) }
    }

    /// Every store icon, as at a launch's first call or when access narrows to none.
    func emptyIcons() {
        remove("app:/tmp/contacts")
        directories = [:]
    }

    private func remove(_ dir: String) {
        if let url = file("\(dir)/x")?.deletingLastPathComponent() { try? FileManager.default.removeItem(at: url) }
    }

    private static func key(_ args: [String: Any]) -> String {
        (try? JSONSerialization.data(withJSONObject: args, options: [.sortedKeys])).map { String(decoding: $0, as: UTF8.self) } ?? ""
    }

    private static func matches(_ c: CNContact, _ needle: String) -> Bool {
        if let n = name(c), n.lowercased().contains(needle) { return true }
        if c.isKeyAvailable(CNContactEmailAddressesKey), c.emailAddresses.contains(where: { ($0.value as String).lowercased().contains(needle) }) { return true }
        if c.isKeyAvailable(CNContactPhoneNumbersKey), c.phoneNumbers.contains(where: { $0.value.stringValue.lowercased().contains(needle) }) { return true }
        return false
    }

    // MARK: the access picker (iOS 18)

    private func share(_ reply: @escaping ([String: Any]) -> Void) {
        #if os(iOS)
        guard #available(iOS 18, *) else { return reply(["error": "NotSupportedError"]) }
        guard access == "limited" else { return reply(["contacts": []]) }
        guard let top = Self.topController() else { return reply(["error": "InvalidStateError", "message": "no window to show the picker in"]) }
        var host: UIViewController?
        let view = AccessPicker { [weak self] ids in
            guard host != nil else { return }
            host?.dismiss(animated: false)
            host = nil
            self?.say("shared \(ids.count)")
            reply(["contacts": ids.map { ["id": $0, "name": NSNull(), "email": NSNull(), "tel": NSNull(), "address": NSNull(), "icon": NSNull()] }])
            if !ids.isEmpty { self?.changed() }
        }
        let shown = UIHostingController(rootView: view)
        shown.view.backgroundColor = .clear
        shown.modalPresentationStyle = .overFullScreen
        host = shown
        top.present(shown, animated: false)
        #else
        reply(["error": "NotSupportedError"])
        #endif
    }

    #if canImport(UIKit)
    static func topController() -> UIViewController? {
        let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        var top = scenes.flatMap(\.windows).first { $0.isKeyWindow }?.rootViewController
        while let presented = top?.presentedViewController { top = presented }
        return top
    }
    #endif
}

#if canImport(UIKit)
extension ContactStore: CNContactPickerDelegate {
    func contactPicker(_ picker: CNContactPickerViewController, didSelect contacts: [CNContact]) { picked(contacts) }
    func contactPickerDidCancel(_ picker: CNContactPickerViewController) { picked([]) }
}

/// UIKit's picker calls the one-contact method only when the delegate lacks the several-contact one.
final class SinglePick: NSObject, CNContactPickerDelegate {
    let done: ([CNContact]) -> Void
    init(_ done: @escaping ([CNContact]) -> Void) { self.done = done }
    func contactPicker(_ picker: CNContactPickerViewController, didSelect contact: CNContact) { done([contact]) }
    func contactPickerDidCancel(_ picker: CNContactPickerViewController) { done([]) }
}

/// iOS 18's access picker, shown at once over a clear host.
@available(iOS 18, *)
private struct AccessPicker: View {
    let done: ([String]) -> Void
    @State private var shown = true
    var body: some View {
        Color.clear.contactAccessPicker(isPresented: $shown) { ids in done(ids) }
            .onChange(of: shown) { _, now in if !now { done([]) } }
    }
}
#else
extension ContactStore: CNContactPickerDelegate {
    func contactPicker(_ picker: CNContactPicker, didSelect contact: CNContact) {
        shownPicker = nil
        picked([contact])
    }
    func contactPickerDidClose(_ picker: CNContactPicker) {
        shownPicker = nil
        picked([])
    }
}
#endif
