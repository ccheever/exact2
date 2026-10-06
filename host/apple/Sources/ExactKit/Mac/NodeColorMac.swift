// A node's colour rows as AppKit and Core Graphics draw them, each in its own
// space (LLP 1100 D2). Moved out of NodeViewMac.swift (the 1,500-line cap).
#if os(macOS)
import AppKit

extension NodeView {
    func textChannels(_ key: String, dark: Bool? = nil) -> [Double]? { style[key]?.textChannels(dark: dark ?? drawsDark, contrast: drawsHighContrast) }
    func color(_ key: String, _ fallback: NSColor) -> NSColor { cgColor(key).flatMap { NSColor(cgColor: $0) } ?? fallback }
    /// A colour row as Core Graphics draws it (LLP 1100 D2).
    func cgColor(_ key: String, dark: Bool? = nil) -> CGColor? { style[key]?.cgColor(dark: dark ?? drawsDark, contrast: drawsHighContrast) }
}
#endif
