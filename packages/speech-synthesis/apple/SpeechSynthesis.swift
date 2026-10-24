// @exact/speech-synthesis on Apple platforms (LLP 1133.012). Stage 2 builds
// the synthesizer; until then this implementation says speech is not
// available here, so an app that imports the package builds and runs.
import ExactPackage
import Foundation

public final class SpeechSynthesisModule: ExactPackage {
    private let context: ExactPackageContext
    public init(context: ExactPackageContext) { self.context = context }

    private static let idle: [String: Any] = [
        "available": false, "speaking": false, "pending": false, "paused": false, "pauseReasons": [String](),
        "audioSessionType": "auto", "utterance": NSNull(), "charIndex": 0, "charLength": 0, "name": "",
        "elapsedTime": 0, "queue": [String](), "apple": NSNull(),
    ]

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        switch name {
        case "speechSynthesisState": reply(Self.idle)
        case "speechSynthesisVoices": reply(["voices": [[String: Any]]()])
        default: reply(["error": "NotSupportedError"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        switch name {
        case "speechSynthesisStateSync": return Self.idle
        case "speechSynthesisVoicesSync": return ["voices": [[String: Any]]()]
        default: return ["error": "NotSupportedError"]
        }
    }
}
