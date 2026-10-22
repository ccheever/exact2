import Foundation

/// Process facts outlive every module/helper. This lock protects memory only, never preferences.
/// Captured seal eligibility is immutable for this root and this actual process.
enum T3QuestionProcess {
    static let id = UUID().uuidString.lowercased()
    struct Seal: Equatable { let process: String; let revision: Int }
    private struct Root { var initial: [String: Seal]?; var confirmed: [String: Seal] = [:]; var wake = 0 }
    private static let lock = NSLock()
    private static var roots: [String: Root] = [:]
    static func key(_ root: URL) -> String { root.standardizedFileURL.resolvingSymlinksInPath().path }
    static func capture(_ key: String, seals: [String: Seal]) {
        lock.lock(); defer { lock.unlock() }
        var root = roots[key] ?? Root()
        if root.initial == nil { root.initial = seals }; roots[key] = root
    }
    static func confirm(_ key: String, seals: [String: Seal]) {
        lock.lock(); defer { lock.unlock() }
        var root = roots[key] ?? Root()
        for (epoch, seal) in root.initial ?? [:] where seal.process != id && seals[epoch] == seal { root.confirmed[epoch] = seal }
        roots[key] = root
    }
    static func eligible(_ key: String) -> [String: Seal] { lock.lock(); defer { lock.unlock() }; return roots[key]?.confirmed ?? [:] }
    static func wake(_ key: String) -> Int { lock.lock(); defer { lock.unlock() }; return roots[key]?.wake ?? 0 }
    static func changed(_ key: String) { lock.lock(); defer { lock.unlock() }; var root = roots[key] ?? Root(); root.wake += 1; roots[key] = root }
}

/// Native receipt evidence compared against the private inventory, never a JS pin assertion.
struct T3QuestionPinReceipt: Equatable {
    let command: String; let id: String; let digest: String; let payload: String; let record: String
    let request: String; let revision: Int; let scope: T3QuestionScope; let questions: [String: String]
}
