// The module's failure, as its replies carry it (`error`: kind, message, …) and the data module reads it.
import Foundation

struct T3Failure: Error {
    let kind: String
    let message: String
    var uncertain = false
    /// A typed error's own fields (the embedded server's exit code is a `detail`).
    var reason = ""
    var detail = ""
    var json: [String: Any] { ["kind": kind, "message": message, "uncertain": uncertain, "reason": reason, "detail": detail] }
}
