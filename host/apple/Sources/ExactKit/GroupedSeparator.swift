// @ref LLP 1084 D7 — the platform's separator under a grouped list's row
// where Exact draws the sheet (the Mac's form, tvOS): decoration, not a
// border. The kernel says where and in what colour (`groupedSeparator`, a
// prop the Rust host derives from `Kernel::grouped_separator`); here it is a
// layer under the row's children, which may stand below the row's box, in
// the gap between two rows where the Mac's form draws it. iOS's list is
// UIKit's, which draws its own.
import ExactCore
import Foundation
import QuartzCore
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// The separator's layer, found among a row's sublayers by its type.
final class GroupedSeparatorLayer: CALayer {
    var color: BatchValue = .null
    override func action(forKey event: String) -> CAAction? { NSNull() }
}

extension NodeView {
    /// The row's layer: AppKit's may not exist yet; UIKit's always does.
    private var separatorHost: CALayer? { layer }

    private var separatorLayer: GroupedSeparatorLayer? {
        separatorHost?.sublayers?.lazy.compactMap { $0 as? GroupedSeparatorLayer }.first
    }

    /// Places, colours or removes the separator after `groupedSeparator`
    /// changed: `{"rect":[x,y,w,h],"color":…}` in the row's own space.
    func applyGroupedSeparator() {
        guard let text = props["groupedSeparator"], let data = text.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let rect = json["rect"] as? [NSNumber], rect.count == 4 else {
            separatorLayer?.removeFromSuperlayer()
            return
        }
        #if os(macOS)
        wantsLayer = true
        #endif
        guard let host = separatorHost else { return }
        let line = separatorLayer ?? GroupedSeparatorLayer()
        if line.superlayer == nil { host.insertSublayer(line, at: 0) }
        let r = rect.map { CGFloat($0.doubleValue) }
        line.frame = CGRect(x: r[0], y: r[1], width: r[2], height: r[3])
        line.color = Self.batchValue(json["color"])
        refreshGroupedSeparator()
    }

    /// Whether the separator is drawn: the Mac's split hides those beside a
    /// painted list's selected row (SplitMac.swift `mark`).
    func hideGroupedSeparator(_ hidden: Bool) {
        if let line = separatorLayer, line.isHidden != hidden { line.isHidden = hidden }
    }

    /// Resolves the separator's colour again in this view's appearance.
    func refreshGroupedSeparator() {
        guard let line = separatorLayer else { return }
        #if os(macOS)
        line.backgroundColor = line.color.cgColor(dark: drawsDark, contrast: drawsHighContrast)
        #else
        line.backgroundColor = line.color.cgColor(dark: drawsDark, contrast: drawsHighContrast, elevated: drawsElevated)
        #endif
    }

    private static func batchValue(_ value: Any?) -> BatchValue {
        switch value {
        case let value as String: return .string(value)
        case let value as NSNumber: return .number(value.doubleValue)
        case let value as [Any]: return .array(value.map(batchValue))
        case let value as [String: Any]: return .object(value.mapValues(batchValue))
        default: return .null
        }
    }
}
