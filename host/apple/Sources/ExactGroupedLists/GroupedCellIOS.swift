import ExactKit
#if os(iOS) || os(tvOS)
import UIKit

final class GroupedCell: UICollectionViewListCell {
    var row: UInt32?
    /// The node of its row's switch or detail button.
    var target: UInt32?
    var height: CGFloat?
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
    #if os(iOS)
    /// A touch its row leaves to no control goes to the row's one field: the
    /// cell is the field (LLP 1116.005 D2).
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        NodeView.cellField(in: self, hit: super.hitTest(point, with: event))
    }
    #endif
    override func preferredLayoutAttributesFitting(_ attributes: UICollectionViewLayoutAttributes) -> UICollectionViewLayoutAttributes {
        guard let height else { return super.preferredLayoutAttributesFitting(attributes) }
        let fitted = attributes.copy() as! UICollectionViewLayoutAttributes
        fitted.size.height = height
        return fitted
    }
}
#endif
