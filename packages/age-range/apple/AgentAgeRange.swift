// @exact/age-range's agent path (LLP 1133.028 §7): the sheet's answer is
// the `age-range` ticket's, as the call's `preferred`. `type @t <age>`
// answers the range that age falls in among the app's gates, as Apple's
// sheet does; `tap @t decline` answers `declined`; `tap @t unavailable`
// fails NotSupportedError. `unverified` is Play's, which Apple never
// answers. The status is laid over this path's reading, available on iOS
// and macOS 26 and later.
import ExactPackage
import Foundation

final class AgentAgeRange {
    private let context: ExactPackageContext
    private var overlay: [String: Any] = [:]

    init(context: ExactPackageContext) { self.context = context }

    func preferred(_ values: [String: Any]) { overlay = values["ageRangeStatus"] as? [String: Any] ?? [:] }

    func status() -> [String: Any] {
        var out: [String: Any] = AgeRangeModule.unavailable
        if #available(iOS 26.0, macOS 26.0, *) {
            out = ["available": true, "regulated": false, "apple": ["requiredRegulatoryFeatures": [String]()]]
        }
        for (k, v) in overlay {
            if var inner = out[k] as? [String: Any], let over = v as? [String: Any] {
                inner.merge(over) { $1 }
                out[k] = inner
            } else {
                out[k] = v
            }
        }
        return out
    }

    func request(_ gates: [Int], _ sheet: [String: Any]?) -> [String: Any] {
        guard status()["available"] as? Bool == true else { return ["error": "NotSupportedError"] }
        let required = ((status()["apple"] as? [String: Any])?["requiredRegulatoryFeatures"] as? [String] ?? []).contains("declaredAgeRangeRequired")
        if let age = (sheet?["typed"] as? [String])?.first.flatMap({ Int($0.trimmingCharacters(in: .whitespaces)) }) {
            let lower = gates.last { $0 <= age } ?? 0
            let upper = gates.first { $0 > age }.map { $0 - 1 }
            context.log("age-range: shared, in the gates \(gates.map(String.init).joined(separator: ","))")
            return AgeRangeModule.range(status: "sharing", lower: lower, upper: upper, declaration: "self",
                                        apple: ["activeParentalControls": [String](), "ageRangeDeclaration": "selfDeclared"])
        }
        switch sheet?["tap"] as? String {
        case "unavailable": return ["error": "NotSupportedError", "apple": ["code": "notAvailable"]]
        case "unverified": return ["error": "OperationError", "message": "unverified is Play's answer; Apple's sheet has none"]
        // Where the region requires the signal Apple's sheet has no decline,
        // which the runner refuses on the ticket; a cancel there is an error.
        case "cancel" where required: return ["error": "InvalidStateError", "message": "the sheet has no decline where the region requires it"]
        default:
            context.log("age-range: declined")
            return AgeRangeModule.range(status: "declined", lower: 0, upper: nil, declaration: "unknown", apple: NSNull())
        }
    }
}
