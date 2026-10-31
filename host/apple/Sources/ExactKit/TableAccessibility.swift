// HTML's table and description list to the platform's accessibility (LLP
// 1153 D7.3): the roles WebKit serves the same elements as (HTML-AAM).
#if os(macOS)
import AppKit
#else
import UIKit
#endif

extension NodeView {
    #if os(macOS)
    /// The AppKit role and subrole of a table's and a description list's
    /// elements: `AXTable`, `AXRow` and `AXCell` (a `th` too, its `scope`
    /// making it a column's or a row's header in ARIA, which AppKit says by
    /// a table's header elements, not a role); a row group is no element of
    /// its own. A `dl` is a list with WebKit's description-list subrole, its
    /// `dt` and `dd` groups with the term's and the description's.
    var tableRole: (role: NSAccessibility.Role, subrole: NSAccessibility.Subrole?)? {
        switch props["semanticTag"] ?? "" {
        case "table": return (.table, nil)
        case "tr": return (.row, nil)
        case "th", "td": return (.cell, nil)
        case "dl": return (.list, NSAccessibility.Subrole(rawValue: "AXDescriptionList"))
        case "dt": return (.group, NSAccessibility.Subrole(rawValue: "AXTerm"))
        case "dd": return (.group, NSAccessibility.Subrole(rawValue: "AXDescription"))
        default: return nil
        }
    }
    #else
    /// UIKit's container type for a table (a data table) and a description
    /// list (a list), which VoiceOver announces on entering it.
    var tableContainerType: UIAccessibilityContainerType? {
        switch props["semanticTag"] ?? "" {
        case "table": return .dataTable
        case "dl": return .list
        default: return nil
        }
    }
    #endif
}
