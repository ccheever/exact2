// @exact/age-range on Apple (LLP 1133.028 D7-D11): Declared Age Range,
// iOS and macOS 26. Under the agent it asks the framework for nothing: the
// runner holds the sheet as an `age-range` ticket, whose answer reaches the
// implementation as the call's `preferred` (AgentAgeRange.swift, §7);
// `--bind age-range` asks the framework.
//
// - The answer is a range in the app's gates, or the region's regulatory
//   gates where a law requires them; both bounds inclusive (D8). Apple's nil
//   lower bound, below the lowest gate, is 0.
// - One sheet at a time (D10): a second request waits for the first, and
//   the platform then answers it from its cache.
// - The sheet needs a window: the key window's top view controller on iOS,
//   the key window on the Mac. With none the request fails NotAllowedError
//   at once.
// - The status is read again at each return to the foreground: whether the
//   region requires the signal can change while the app is away.
import ExactPackage
import Foundation
#if canImport(DeclaredAgeRange)
import DeclaredAgeRange
#endif
#if os(macOS)
import AppKit
#else
import UIKit
#endif

public final class AgeRangeModule: ExactPackage {
    let context: ExactPackageContext
    private lazy var agent = AgentAgeRange(context: context)
    /// Requests waiting for the sheet in front of them.
    private var waiting: [(@escaping () -> Void) -> Void] = []
    private var asking = false

    public init(context: ExactPackageContext) { self.context = context }

    private func say(_ line: String) { context.log("age-range: \(line)") }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        switch name {
        case "ageRangeStatus":
            if context.agent { return reply(agent.status()) }
            Self.status(reply)
        case "requestAgeRange":
            let gates = (args["ageGates"] as? [NSNumber])?.map(\.doubleValue) ?? []
            if let why = Self.refused(gates) { return reply(["error": "TypeError", "message": why]) }
            let ages = gates.map { Int($0) }
            if context.agent { return reply(agent.request(ages, preferred as? [String: Any])) }
            queue { [self] done in request(ages) { reply($0); done() } }
        default: reply(["error": "NotSupportedError"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] { ["error": "NotSupportedError"] }

    public func preferred(_ values: [String: Any]) { agent.preferred(values) }

    public func foreground(_ shown: Bool) {
        if shown { context.changed("age-range.ageRangeStatus") }
    }

    /// Apple's rule for gates (D8): one to three whole ages, ascending, at least two years apart.
    static func refused(_ gates: [Double]) -> String? {
        guard (1...3).contains(gates.count), gates.allSatisfy({ $0 >= 0 && $0 == $0.rounded() }) else {
            return "ageGates are one to three whole ages"
        }
        return zip(gates, gates.dropFirst()).allSatisfy({ $1 - $0 >= 2 }) ? nil : "ageGates ascend at least two years apart"
    }

    private func queue(_ work: @escaping (@escaping () -> Void) -> Void) {
        waiting.append(work)
        next()
    }

    private func next() {
        guard !asking, !waiting.isEmpty else { return }
        asking = true
        let work = waiting.removeFirst()
        work { [weak self] in
            DispatchQueue.main.async {
                self?.asking = false
                self?.next()
            }
        }
    }

    // MARK: Declared Age Range

    static func status(_ reply: @escaping ([String: Any]) -> Void) {
        #if canImport(DeclaredAgeRange)
        guard #available(iOS 26.0, macOS 26.0, *) else { return reply(unavailable) }
        Task { @MainActor in
            var regulated: Any = NSNull(), features: [String] = []
            #if os(macOS)
            regulated = false
            #else
            if #available(iOS 26.2, *) { regulated = (try? await AgeRangeService.shared.isEligibleForAgeFeatures) ?? NSNull() }
            #endif
            if #available(iOS 26.4, macOS 26.4, *), let set = try? await AgeRangeService.shared.requiredRegulatoryFeatures {
                features = set.map { "\($0)" }.sorted()
            }
            reply(["available": true, "regulated": regulated, "apple": ["requiredRegulatoryFeatures": features]])
        }
        #else
        reply(unavailable)
        #endif
    }

    static let unavailable: [String: Any] = ["available": false, "regulated": NSNull(), "apple": NSNull()]

    private func request(_ gates: [Int], _ reply: @escaping ([String: Any]) -> Void) {
        #if canImport(DeclaredAgeRange)
        guard #available(iOS 26.0, macOS 26.0, *) else { return reply(["error": "NotSupportedError"]) }
        #if os(macOS)
        guard let anchor = NSApp.keyWindow else { return reply(["error": "NotAllowedError", "message": "no key window"]) }
        #else
        guard let anchor = Self.topController() else { return reply(["error": "NotAllowedError", "message": "no key window"]) }
        #endif
        Task { @MainActor [self] in
            do {
                let response = try await AgeRangeService.shared.requestAgeRange(
                    ageGates: gates[0], gates.count > 1 ? gates[1] : nil, gates.count > 2 ? gates[2] : nil, in: anchor)
                switch response {
                case .declinedSharing:
                    say("declined")
                    reply(Self.range(status: "declined", lower: 0, upper: nil, declaration: "unknown", apple: NSNull()))
                case .sharing(let range):
                    say("shared")
                    reply(Self.range(status: "sharing", lower: range.lowerBound ?? 0, upper: range.upperBound,
                                     declaration: Self.word(range.ageRangeDeclaration), apple: Self.apple(range)))
                @unknown default:
                    reply(["error": "OperationError", "message": "an answer this build does not know"])
                }
            } catch let error as AgeRangeService.Error {
                reply(Self.failure(error))
            } catch {
                reply(["error": "OperationError", "message": error.localizedDescription])
            }
        }
        #else
        reply(["error": "NotSupportedError"])
        #endif
    }

    static func range(status: String, lower: Int, upper: Int?, declaration: String, apple: Any) -> [String: Any] {
        ["status": status, "lowerBound": lower, "upperBound": upper.map { $0 as Any } ?? NSNull(), "declaration": declaration,
         "apple": apple, "android": NSNull()]
    }

    #if canImport(DeclaredAgeRange)
    @available(iOS 26.0, macOS 26.0, *)
    static func word(_ declaration: AgeRangeService.AgeRangeDeclaration?) -> String {
        switch declaration {
        case .none: "unknown"
        case .selfDeclared: "self"
        case .guardianDeclared: "guardian"
        // `confirmed`, and the payment and ID cases it replaces.
        default: "confirmed"
        }
    }

    @available(iOS 26.0, macOS 26.0, *)
    static func apple(_ range: AgeRangeService.AgeRange) -> [String: Any] {
        ["activeParentalControls": range.activeParentalControls.contains(.communicationLimits) ? ["communicationLimits"] : [],
         "ageRangeDeclaration": range.ageRangeDeclaration.map { "\($0)" as Any } ?? NSNull()]
    }

    @available(iOS 26.0, macOS 26.0, *)
    static func failure(_ error: AgeRangeService.Error) -> [String: Any] {
        let name = switch error {
        case .notAvailable: "NotSupportedError"
        case .declinedOnboarding, .invalidAccount: "NotAllowedError"
        case .network: "NetworkError"
        default: "OperationError"
        }
        return ["error": name, "message": error.localizedDescription, "apple": ["code": "\(error)"]]
    }
    #endif

    #if canImport(UIKit)
    static func topController() -> UIViewController? {
        let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        var top = scenes.flatMap(\.windows).first { $0.isKeyWindow }?.rootViewController
        while let presented = top?.presentedViewController { top = presented }
        return top
    }
    #endif
}
