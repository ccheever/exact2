// @exact/speech-recognition on Apple platforms (LLP 1133.012). Stage 2 builds
// the recognizer; until then this implementation says recognition is not
// available here, so an app that imports the package builds and runs.
import ExactPackage
import Foundation

public final class SpeechRecognitionModule: ExactPackage {
    private let context: ExactPackageContext
    public init(context: ExactPackageContext) { self.context = context }

    private static let unavailable: [String: Any] = [
        "available": false, "state": "unknown", "inputs": [String](), "local": false, "apple": NSNull(), "android": NSNull(),
    ]
    private static let none: [String: Any] = [
        "id": "", "state": "ended", "audio": false, "sound": false, "speech": false, "results": [[String: Any]](),
        "finalCount": 0, "interim": NSNull(), "leavesDevice": false, "error": NSNull(), "message": NSNull(),
    ]

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        switch name {
        case "speechRecognitionStatus": reply(Self.unavailable)
        case "speechRecognitionAvailability": reply(["availability": "unavailable"])
        case "speechRecognitionState" where (args["recognition"] as? String ?? "").isEmpty: reply(Self.none)
        default: reply(["error": "NotSupportedError"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "speechRecognitionStatusSync" ? Self.unavailable : ["error": "NotSupportedError"]
    }
}
