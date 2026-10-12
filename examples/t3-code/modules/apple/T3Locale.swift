// The Mac's locale for timestamps (task desktop-shell-details). Reference
// apps/desktop/src/electron/ElectronApp.ts getSystemLocale (1e2ecbd975; MIT, see LICENSE-T3):
// Electron reads [NSLocale currentLocale] on macOS and the bridge reports it with `_` → `-`.
// The client reads it from the status presentation (`systemLocale`) and resolves it with
// resolveTimestampLocale (timestamp-format.ts), which falls back for a tag Intl rejects.
import Foundation

enum T3Locale {
    static func systemLocale(_ locale: Locale = .current) -> String {
        locale.identifier.replacingOccurrences(of: "_", with: "-")
    }
}
