import ExactKit
#if os(iOS) || os(tvOS)
import UIKit

// Keep the detail button while UIKit refreshes the cell for accessibility.
final class GroupedCell: UICollectionViewListCell {
    #if os(iOS)
    weak var menuSource: NodeView?
    private var detailButton: UIButton?
    private var detailAction: (() -> Void)?

    func detailAccessory(enabled: Bool, action: @escaping () -> Void) -> UICellAccessory {
        detailAction = action
        let button = detailButton ?? {
            let button = UIButton(type: .detailDisclosure)
            button.addAction(UIAction { [weak self] _ in self?.detailAction?() }, for: .touchUpInside)
            button.sizeToFit()
            detailButton = button
            return button
        }()
        button.isEnabled = enabled
        button.removeFromSuperview()
        return .customView(configuration: .init(customView: button, placement: .trailing(displayed: .always)))
    }

    #endif

    var row: UInt32?
    /// Whether `interact` put `.notEnabled` in its traits.
    var markedNotEnabled = false
    /// The node of its row's switch or detail button.
    var target: UInt32?
    var height: CGFloat?
    /// An authored `min-height` for a cell UIKit's configuration sizes.
    var minimumHeight: CGFloat?
    /// Its standard row's svg-only icon path token, drawn again at a new
    /// Dynamic Type size (stage 9 A2); nil for an SF Symbol or none.
    var pathSymbol: String? {
        didSet {
            guard pathSymbol != nil, !watchesSize else { return }
            watchesSize = true
            registerForTraitChanges([UITraitPreferredContentSizeCategory.self]) { (cell: GroupedCell, _: UITraitCollection) in cell.redrawPath() }
        }
    }
    private var watchesSize = false
    /// A path's side: the body text's size under these traits, as a symbol
    /// with no configuration in a list cell takes.
    static func pathPoints(_ traits: UITraitCollection) -> CGFloat {
        UIFont.preferredFont(forTextStyle: .body, compatibleWith: traits).pointSize
    }
    func redrawPath() {
        guard let pathSymbol, var c = contentConfiguration as? UIListContentConfiguration else { return }
        c.image = FaceSymbol.image(pathSymbol, points: Self.pathPoints(traitCollection))
        contentConfiguration = c
    }
    /// Its standard row's content. UIKit's list content view hides a label
    /// a new configuration drops but keeps its text, which UIKit's
    /// accessibility still reads: a reused cell's Privacy Policy row read
    /// the Font row's "Georgia" to VoiceOver (iOS 27). A cell that drops a
    /// text starts from a new content view.
    func show(_ c: UIListContentConfiguration) {
        if let old = contentConfiguration as? UIListContentConfiguration,
           old.secondaryText != nil && c.secondaryText == nil || old.text != nil && c.text == nil {
            contentConfiguration = nil
        }
        contentConfiguration = c
    }
    #if os(iOS)
    /// A touch its row leaves to no control goes to the row's one field: the
    /// cell is the field (LLP 1116.005 D2).
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        NodeView.cellField(in: self, hit: super.hitTest(point, with: event))
    }
    #endif
    override func preferredLayoutAttributesFitting(_ attributes: UICollectionViewLayoutAttributes) -> UICollectionViewLayoutAttributes {
        if let height {
            let fitted = attributes.copy() as! UICollectionViewLayoutAttributes
            fitted.size.height = height
            return fitted
        }
        let fitted = super.preferredLayoutAttributesFitting(attributes)
        guard let least = minimumHeight, fitted.size.height < least else { return fitted }
        let raised = fitted.copy() as! UICollectionViewLayoutAttributes
        raised.size.height = least
        return raised
    }
}

extension UICollectionViewListCell {
    /// A row's or header's margins are the list layout's, never the safe
    /// area's: a cell under the home indicator (a fling past the end)
    /// otherwise grew by the inset, moved, and shrank a pixel a layout pass
    /// until UIKit's feedback-loop check stopped the app. The layout's
    /// sections already keep clear of the sides' safe area (its
    /// `contentInsetsReference`). Again after each configuration, which may
    /// replace the content view.
    func marginsIgnoreSafeArea() {
        insetsLayoutMarginsFromSafeArea = false
        contentView.insetsLayoutMarginsFromSafeArea = false
    }
}
#endif

#if os(iOS)
extension GroupedCell: GroupedMenuSurface {}
#endif
