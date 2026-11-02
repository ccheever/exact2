// packages/shared/src/desktopBootstrapToken.ts (9c161462c9; MIT, see LICENSE-T3).
import Foundation
import CryptoKit

enum T3DesktopBootstrap {
    static let windowMs: Double = 12 * 60 * 60 * 1000

    static func token(secret: String, nowMs: Double) -> String {
        let window = Int64(floor(nowMs / windowMs))
        let message = Data("t3-desktop-bootstrap:\(window)".utf8)
        return HMAC<SHA256>.authenticationCode(for: message, using: SymmetricKey(data: Data(secret.utf8)))
            .map { String(format: "%02x", $0) }.joined()
    }
}
