// The Interface font size for the native views (T3 Code appearanceFonts.ts: the root font size
// every rem follows). The data module sends it with the device presentation (T3Module+Window.swift
// `devicePresentation`, client.ts); a view sizes its rem metrics by `rem(_:)` and redraws on
// `T3RootFont.changed`. Text that follows its own px setting (the prompt) does not use it.
import Foundation
import AppKit

enum T3RootFont {
    static let changed = Notification.Name("T3RootFontChanged")
    /// The root font size in px (12–20; 16 until the preferences arrive).
    private(set) static var size: CGFloat = 16
    /// `px` at the root size 16, at the current root size (N/16 rem).
    static func rem(_ px: CGFloat) -> CGFloat { px * size / 16 }
    /// Main thread. Clamps as clampInterfaceFontSize; a change notifies the views.
    static func set(_ value: Any?) {
        let number = (value as? NSNumber)?.doubleValue ?? 16
        let next = CGFloat(number.isFinite ? min(20, max(12, number.rounded())) : 16)
        guard next != size else { return }
        size = next
        NotificationCenter.default.post(name: changed, object: nil)
    }
}
