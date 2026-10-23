import Foundation

// Which of Discord's official clients this build identifies as (Excord LLP 0001 §2): the data module
// cannot tell its host, and each host claims its own client.
final class Host: ExactModule {
    override func call(_ request: [String: Any]) throws -> [String: Any] {
        guard request["op"] as? String == "host" else { throw ExactNativeRefusal("host answers op host") }
        #if os(iOS)
        return ["host": "ios"]
        #else
        return ["host": "macos"]
        #endif
    }
}
let exactModule: ExactModule.Type = Host.self
