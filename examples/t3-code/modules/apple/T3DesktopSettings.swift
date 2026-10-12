// The desktop settings file, `<T3 home>/userdata/desktop-settings.json` (decision U7, 2026-10-08: as the
// original app), after T3 Code (MIT, see LICENSE-T3; reference 1e2ecbd975):
// apps/desktop/src/settings/DesktopAppSettings.ts (the document schema, `normalizeDesktopSettingsDocument`,
// `toDesktopSettingsDocument`, `readSettings`, `writeSettings`), packages/shared/src/schemaJson.ts
// (`fromLenientJson`: comments and trailing commas stripped before parsing), packages/shared/src/symlink.ts
// (`resolveSymlinkTarget`) and apps/desktop/src/app/DesktopStatePaths.ts (`<T3 home>/userdata`).
//
// The clone and T3 Code share the T3 home, so they share this file: the Local environment switch, Network
// access and Tailscale HTTPS set in one app are what the other starts with. Read once when the first session
// attaches (the reference's `load` before bootstrap); a missing, unreadable or invalid file is every default,
// and one key of the wrong type makes the whole document invalid (the schema decode fails). A write keeps
// only the values that differ from the defaults (sparse), in the schema's key order, as compact JSON and a
// newline, through `<target>.<pid>.<uuid>.tmp` and a rename onto the symlink chain's target; a failed write
// leaves the settings in memory as they were. Keys this app never changes (window bounds, the Linux password
// store, WSL) are carried through as the reference normalizes them. The update channel is General › About's
// Update track: saved as the reference's no-feed select saves it (`setUpdateChannel`), read by no feed.
//
// One-time carry-over: before this file, the clone kept the four keys at the top level of its own
// `t3-code.json`. At the first attach, a key that file still has and the desktop document does not is
// adopted (an absent key is the reference's default, so the clone's own choice wins), then written here;
// client.ts drops the old keys from `t3-code.json` on its next save.
import Foundation

struct T3WindowBounds: Equatable {
    var x: Int, y: Int, width: Int, height: Int
}

struct T3DesktopSettings: Equatable {
    var localEnvironmentEnabled = true
    var linuxPasswordStore = "auto"
    var mainWindowBounds: T3WindowBounds? = nil
    var mainWindowMaximized = false
    var serverExposureMode = "local-only"
    var tailscaleServeEnabled = false
    var tailscaleServePort = 443
    var updateChannel = "latest"
    var updateChannelConfiguredByUser = false
    var wslBackendEnabled = false
    var wslDistro: String? = nil
    var wslOnly = false

    /// The desktop app this client mirrors (version-skew.ts `CLIENT_VERSION`): its version picks the
    /// default update channel (`resolveDefaultDesktopSettings`).
    static let appVersion = "0.0.46-nightly.20261004.1"
    static let fileName = "desktop-settings.json"
    /// The keys the clone kept in `t3-code.json` before U7 was decided.
    static let legacyKeys = ["localEnvironmentEnabled", "serverExposureMode", "tailscaleServeEnabled", "tailscaleServePort"]

    /// `desktopSettingsPath`: `<T3 home>/userdata/desktop-settings.json`.
    static func path(home: URL) -> URL {
        home.appendingPathComponent("userdata", isDirectory: true).appendingPathComponent(fileName)
    }

    /// `resolveDefaultDesktopUpdateChannel`: nightly for a `x-nightly.YYYYMMDD.N` version.
    static func defaultUpdateChannel(_ version: String) -> String {
        version.range(of: #"^[^-+]+-nightly\.\d{8}\.\d+$"#, options: .regularExpression) != nil ? "nightly" : "latest"
    }
    static func defaults(appVersion: String = appVersion) -> T3DesktopSettings {
        var settings = T3DesktopSettings(); settings.updateChannel = defaultUpdateChannel(appVersion); return settings
    }

    // MARK: Decoding (`readSettings`)

    /// The file's text as settings: every default when it is not a valid document.
    static func read(_ raw: String?, appVersion: String = appVersion) -> T3DesktopSettings {
        guard let raw, let document = document(raw) else { return defaults(appVersion: appVersion) }
        return normalize(document, appVersion: appVersion)
    }

    /// `fromLenientJson(DesktopSettingsDocument)`: the parsed object when every present key has its schema's
    /// type, else nil. Unknown keys are dropped (the reference's Struct decode ignores them).
    static func document(_ raw: String) -> [String: Any]? {
        guard let data = stripLenient(raw).data(using: .utf8),
              let object = (try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])) as? [String: Any] else { return nil }
        let isBool = { (value: Any) in (value as? NSNumber).map { CFGetTypeID($0) == CFBooleanGetTypeID() } ?? false }
        let isNumber = { (value: Any) in (value as? NSNumber).map { CFGetTypeID($0) != CFBooleanGetTypeID() } ?? false }
        let isString = { (value: Any) in value is String }
        let isNull = { (value: Any) in value is NSNull }
        let literal = { (allowed: Set<String>) in { (value: Any) in (value as? String).map(allowed.contains) ?? false } }
        let bounds = { (value: Any) -> Bool in
            if isNull(value) { return true }
            guard let box = value as? [String: Any] else { return false }
            return ["x", "y", "width", "height"].allSatisfy { box[$0].map(isNumber) ?? false }
        }
        let schema: [String: (Any) -> Bool] = [
            "localEnvironmentEnabled": isBool, "linuxPasswordStore": { _ in true }, "mainWindowBounds": bounds,
            "mainWindowMaximized": isBool, "serverExposureMode": literal(["local-only", "network-accessible"]),
            "tailscaleServeEnabled": isBool, "tailscaleServePort": isNumber, "updateChannel": literal(["latest", "nightly"]),
            "updateChannelConfiguredByUser": isBool, "wslBackendEnabled": isBool, "wslMode": literal(["local", "wsl"]),
            "wslDistro": { isNull($0) || isString($0) }, "wslOnly": isBool,
        ]
        var document: [String: Any] = [:]
        for (key, check) in schema {
            guard let value = object[key] else { continue }
            guard check(value) else { return nil }
            document[key] = value
        }
        return document
    }

    /// `normalizeDesktopSettingsDocument`.
    static func normalize(_ parsed: [String: Any], appVersion: String = appVersion) -> T3DesktopSettings {
        let base = defaults(appVersion: appVersion)
        let bool = { (key: String) in (parsed[key] as? NSNumber).map { CFGetTypeID($0) == CFBooleanGetTypeID() ? $0.boolValue : nil } ?? nil }
        var settings = T3DesktopSettings()
        settings.localEnvironmentEnabled = bool("localEnvironmentEnabled") != false
        settings.linuxPasswordStore = normalizeLinuxPasswordStore(parsed["linuxPasswordStore"])
        settings.mainWindowBounds = normalizeBounds(parsed["mainWindowBounds"])
        settings.mainWindowMaximized = settings.mainWindowBounds != nil && bool("mainWindowMaximized") == true
        settings.serverExposureMode = parsed["serverExposureMode"] as? String == "network-accessible" ? "network-accessible" : "local-only"
        settings.tailscaleServeEnabled = bool("tailscaleServeEnabled") == true
        settings.tailscaleServePort = normalizePort(parsed["tailscaleServePort"])
        let parsedChannel = parsed["updateChannel"] as? String
        let legacy = parsed["updateChannelConfiguredByUser"] == nil
        let configured = bool("updateChannelConfiguredByUser") == true || (legacy && parsedChannel == "nightly")
        settings.updateChannel = configured ? (parsedChannel ?? base.updateChannel) : base.updateChannel
        settings.updateChannelConfiguredByUser = configured
        settings.wslBackendEnabled = bool("wslBackendEnabled") == true || (parsed["wslBackendEnabled"] == nil && parsed["wslMode"] as? String == "wsl")
        settings.wslDistro = (parsed["wslDistro"] as? String).flatMap { $0.range(of: #"^\w(?:[\w \-.]*\w)?$"#, options: .regularExpression) != nil && isAsciiWord($0) ? $0 : nil }
        settings.wslOnly = bool("wslOnly") == true
        return settings
    }

    /// JavaScript's `\w` is ASCII only; NSRegularExpression's is Unicode.
    private static func isAsciiWord(_ value: String) -> Bool { value.unicodeScalars.allSatisfy { $0.isASCII } }

    /// `normalizeTailscaleServePort`: an integer from 1 to 65535, else 443.
    static func normalizePort(_ value: Any?) -> Int {
        guard let number = value as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID() else { return 443 }
        let double = number.doubleValue
        return double.isFinite && double.rounded() == double && double >= 1 && double <= 65_535 ? Int(double) : 443
    }
    /// `normalizeLinuxPasswordStorePreference`.
    static func normalizeLinuxPasswordStore(_ value: Any?) -> String {
        guard let value = value as? String, ["gnome-libsecret", "kwallet", "kwallet5", "kwallet6"].contains(value) else { return "auto" }
        return value
    }
    /// `normalizeMainWindowBounds` (`DesktopWindowBoundsSchema`): safe integers, at least 840 × 620.
    static func normalizeBounds(_ value: Any?) -> T3WindowBounds? {
        guard let box = value as? [String: Any] else { return nil }
        func int(_ key: String) -> Int? {
            guard let number = box[key] as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID() else { return nil }
            let double = number.doubleValue
            return double.isFinite && double.rounded() == double && abs(double) <= 9_007_199_254_740_991 ? Int(double) : nil
        }
        guard let x = int("x"), let y = int("y"), let width = int("width"), let height = int("height"), width >= 840, height >= 620 else { return nil }
        return T3WindowBounds(x: x, y: y, width: width, height: height)
    }

    /// The lenient parse's three passes: `//` comments, `/* */` comments, then trailing commas before `}`
    /// or `]`; each pass keeps double-quoted strings intact.
    static func stripLenient(_ input: String) -> String {
        let string = #"("(?:[^"\\]|\\.)*")"#
        var text = replace(input, pattern: string + #"|//[^\n]*"#) { _ in "" }
        text = replace(text, pattern: string + #"|/\*[\s\S]*?\*/"#) { _ in "" }
        text = replace(text, pattern: string + #"|,(\s*[}\]])"#) { $0 }
        return text
    }
    /// A global replace where a match of group 1 (a string literal) stays, and any other match becomes
    /// `other(group 2)` (group 2 empty when it did not take part).
    private static func replace(_ input: String, pattern: String, other: (String) -> String) -> String {
        guard let regex = try? NSRegularExpression(pattern: pattern) else { return input }
        let source = input as NSString
        var output = "", cursor = 0
        for match in regex.matches(in: input, range: NSRange(location: 0, length: source.length)) {
            output += source.substring(with: NSRange(location: cursor, length: match.range.location - cursor))
            if match.range(at: 1).location != NSNotFound { output += source.substring(with: match.range) }
            else { output += other(match.numberOfRanges > 2 && match.range(at: 2).location != NSNotFound ? source.substring(with: match.range(at: 2)) : "") }
            cursor = match.range.location + match.range.length
        }
        return output + source.substring(from: cursor)
    }

    // MARK: Encoding (`toDesktopSettingsDocument`, then `JSON.stringify`)

    /// The sparse document as compact JSON, in the schema's key order (no trailing newline).
    static func encode(_ settings: T3DesktopSettings, defaults: T3DesktopSettings) -> String {
        var parts: [String] = []
        func add(_ key: String, _ json: String) { parts.append("\(quote(key)):\(json)") }
        if settings.localEnvironmentEnabled != defaults.localEnvironmentEnabled { add("localEnvironmentEnabled", "\(settings.localEnvironmentEnabled)") }
        if settings.linuxPasswordStore != defaults.linuxPasswordStore { add("linuxPasswordStore", quote(settings.linuxPasswordStore)) }
        if let bounds = settings.mainWindowBounds { add("mainWindowBounds", #"{"x":\#(bounds.x),"y":\#(bounds.y),"width":\#(bounds.width),"height":\#(bounds.height)}"#) }
        if settings.mainWindowMaximized { add("mainWindowMaximized", "true") }
        if settings.serverExposureMode != defaults.serverExposureMode { add("serverExposureMode", quote(settings.serverExposureMode)) }
        if settings.tailscaleServeEnabled != defaults.tailscaleServeEnabled { add("tailscaleServeEnabled", "\(settings.tailscaleServeEnabled)") }
        if settings.tailscaleServePort != defaults.tailscaleServePort { add("tailscaleServePort", "\(settings.tailscaleServePort)") }
        if settings.updateChannel != defaults.updateChannel { add("updateChannel", quote(settings.updateChannel)) }
        if settings.updateChannelConfiguredByUser != defaults.updateChannelConfiguredByUser { add("updateChannelConfiguredByUser", "\(settings.updateChannelConfiguredByUser)") }
        if settings.wslBackendEnabled != defaults.wslBackendEnabled { add("wslBackendEnabled", "\(settings.wslBackendEnabled)") }
        if settings.wslDistro != defaults.wslDistro { add("wslDistro", settings.wslDistro.map(quote) ?? "null") }
        if settings.wslOnly != defaults.wslOnly { add("wslOnly", "\(settings.wslOnly)") }
        return "{\(parts.joined(separator: ","))}"
    }
    /// `JSON.stringify` of a string.
    static func quote(_ value: String) -> String {
        var out = "\""
        for unit in value.unicodeScalars {
            switch unit {
            case "\"": out += "\\\""
            case "\\": out += "\\\\"
            case "\u{08}": out += "\\b"
            case "\u{0C}": out += "\\f"
            case "\n": out += "\\n"
            case "\r": out += "\\r"
            case "\t": out += "\\t"
            case _ where unit.value < 0x20: out += String(format: "\\u%04x", unit.value)
            default: out.unicodeScalars.append(unit)
            }
        }
        return out + "\""
    }

    /// What TypeScript reads (`localBackendStatus.desktopSettings`): the five keys this app changes.
    var statusValue: [String: Any] {
        ["localEnvironmentEnabled": localEnvironmentEnabled, "serverExposureMode": serverExposureMode,
         "tailscaleServeEnabled": tailscaleServeEnabled, "tailscaleServePort": tailscaleServePort, "updateChannel": updateChannel]
    }

    /// `setUpdateChannel`: a new channel is the user's choice from now on (`updateChannelConfiguredByUser`);
    /// the same channel changes nothing.
    static func settingUpdateChannel(_ settings: T3DesktopSettings, _ channel: String) -> T3DesktopSettings {
        guard settings.updateChannel != channel else { return settings }
        var next = settings; next.updateChannel = channel; next.updateChannelConfiguredByUser = true
        return next
    }

    /// The carry-over from `t3-code.json`: each old key the desktop document lacks, decoded as the clone
    /// decoded it (any value but `false` is on; only `network-accessible` widens; only `true` serves).
    static func adoptingLegacy(_ settings: T3DesktopSettings, legacy: [String: Any], present: Set<String>) -> T3DesktopSettings {
        var next = settings
        let bool = { (value: Any?) in (value as? NSNumber).map { CFGetTypeID($0) == CFBooleanGetTypeID() ? $0.boolValue : nil } ?? nil }
        if let value = legacy["localEnvironmentEnabled"], !present.contains("localEnvironmentEnabled") { next.localEnvironmentEnabled = bool(value) != false }
        if let value = legacy["serverExposureMode"], !present.contains("serverExposureMode") {
            next.serverExposureMode = value as? String == "network-accessible" ? "network-accessible" : "local-only"
        }
        if let value = legacy["tailscaleServeEnabled"], !present.contains("tailscaleServeEnabled") { next.tailscaleServeEnabled = bool(value) == true }
        if let value = legacy["tailscaleServePort"], !present.contains("tailscaleServePort") { next.tailscaleServePort = normalizePort(value) }
        return next
    }
}

/// `DesktopSettingsWriteError`: "Desktop settings write failed during <operation> at <path>."
struct T3DesktopSettingsWriteError: Error, Equatable {
    let operation: String, path: String
    var message: String { "Desktop settings write failed during \(operation) at \(path)." }
}

/// The app's one settings document (`DesktopAppSettings`): `load` once, `get`, and `persist` (serialized).
final class T3DesktopSettingsStore: @unchecked Sendable {
    private let lock = NSLock()
    private var current: T3DesktopSettings
    private(set) var path: URL?
    let appVersion: String
    let defaults: T3DesktopSettings
    /// The file system seam (tests make a step fail).
    var fileManager = FileManager.default
    var failStep: String? = nil

    init(appVersion: String = T3DesktopSettings.appVersion) {
        self.appVersion = appVersion
        defaults = T3DesktopSettings.defaults(appVersion: appVersion)
        current = defaults
    }

    var settings: T3DesktopSettings { lock.lock(); defer { lock.unlock() }; return current }

    /// `load`: the file at `path`, read once. A nil path (a refused development build has no T3 home)
    /// keeps the defaults in memory for this run. Returns the keys the document carried.
    @discardableResult
    func load(path: URL?) -> Set<String> {
        let raw = path.flatMap { try? String(contentsOf: $0, encoding: .utf8) }
        let document = raw.flatMap(T3DesktopSettings.document)
        let settings = document.map { T3DesktopSettings.normalize($0, appVersion: appVersion) } ?? defaults
        lock.lock(); self.path = path; current = settings; lock.unlock()
        return Set(document?.keys.map { $0 } ?? [])
    }

    /// `persist`: applies `update`; when it changed something, writes the file, then keeps the new value.
    /// A failed write throws and leaves the settings as they were.
    @discardableResult
    func persist(_ update: (T3DesktopSettings) -> T3DesktopSettings) throws -> (settings: T3DesktopSettings, changed: Bool) {
        lock.lock(); defer { lock.unlock() }
        let next = update(current)
        if next == current { return (current, false) }
        if let path { try write(next, to: path) }
        current = next
        return (next, true)
    }

    /// `writeSettings`.
    private func write(_ settings: T3DesktopSettings, to settingsPath: URL) throws {
        let fail = { (operation: String, path: String) in T3DesktopSettingsWriteError(operation: operation, path: path) }
        guard failStep != "resolve-symlink", let target = Self.resolveSymlinkTarget(settingsPath.path) else { throw fail("resolve-symlink", settingsPath.path) }
        let directory = (target as NSString).deletingLastPathComponent
        let suffix = UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased()
        let tempPath = "\(target).\(getpid()).\(suffix).tmp"
        let encoded = T3DesktopSettings.encode(settings, defaults: defaults)
        do { guard failStep != "create-directory" else { throw CocoaError(.fileWriteUnknown) }
             try fileManager.createDirectory(atPath: directory, withIntermediateDirectories: true) } catch { throw fail("create-directory", directory) }
        do { guard failStep != "write-temporary-file" else { throw CocoaError(.fileWriteUnknown) }
             try Data("\(encoded)\n".utf8).write(to: URL(fileURLWithPath: tempPath)) } catch { throw fail("write-temporary-file", tempPath) }
        guard failStep != "replace-settings-file", rename(tempPath, target) == 0 else { throw fail("replace-settings-file", settingsPath.path) }
    }

    /// `resolveSymlinkTarget`: follows a chain of links to the file it names (which may not exist yet), at
    /// most 40 hops; nil on a cycle or an unreadable link.
    static func resolveSymlinkTarget(_ filePath: String) -> String? {
        var current = URL(fileURLWithPath: filePath).standardizedFileURL.path
        for _ in 0..<40 {
            guard let link = try? FileManager.default.destinationOfSymbolicLink(atPath: current) else {
                // Not a link (or missing): this is the target, unless it exists and could not be read as one.
                return current
            }
            // A relative target is relative to where the link really lives (`realPath(dirname)`).
            let parent = (current as NSString).deletingLastPathComponent
            let realParent = realpath(parent, nil).map { pointer in defer { free(pointer) }; return String(cString: pointer) } ?? parent
            current = link.hasPrefix("/") ? URL(fileURLWithPath: link).standardizedFileURL.path
                : URL(fileURLWithPath: realParent).appendingPathComponent(link).standardizedFileURL.path
        }
        return nil
    }
}
