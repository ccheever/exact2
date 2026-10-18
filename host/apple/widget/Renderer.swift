import Foundation
import SwiftUI
import WidgetKit

/// WP4's driver replaces only `widgetSnapshotView`. The stand-in draws kernel
/// frames and lines; it is deliberately limited to Caltrain's text and time.
public struct WidgetSnapshotEnvironment {
    public let family: WidgetFamily
    public let placeholder: Bool
    public let assetRoot: URL
    public let entryDate: Date
    public let linkBase: URL?
    public let imageRoots: [URL]
    public init(family: WidgetFamily, placeholder: Bool, assetRoot: URL, entryDate: Date, linkBase: URL? = nil, imageRoots: [URL] = []) {
        self.family = family; self.placeholder = placeholder; self.assetRoot = assetRoot; self.entryDate = entryDate; self.linkBase = linkBase; self.imageRoots = imageRoots
    }
}
public func widgetSnapshotView(snapshot: HeadlessSession.Snapshot, family: WidgetFamily, environment: WidgetSnapshotEnvironment) -> some View {
    SnapshotStandIn(snapshot: snapshot).privacySensitive(!environment.placeholder)
}
private struct SnapshotStandIn: View {
    let snapshot: HeadlessSession.Snapshot
    struct Leaf: Identifiable { let node: HeadlessSession.Node; let origin: CGPoint; var id: UInt32 { node.id } }
    var leaves: [Leaf] {
        var out: [Leaf] = []
        func walk(_ id: UInt32, _ parent: CGPoint) {
            guard let node = snapshot.nodes[id] else { return }
            let origin = CGPoint(x: parent.x + node.frame.minX, y: parent.y + node.frame.minY)
            if node.kind == "text" || node.kind == "time" { out.append(Leaf(node: node, origin: origin)) }
            for child in node.children { walk(child, origin) }
        }
        for root in snapshot.roots { walk(root, .zero) }
        return out
    }
    var body: some View {
        ZStack(alignment: .topLeading) {
            ForEach(leaves) { leaf in
                if let instant = leaf.node.props["datetime"].flatMap(Double.init) {
                    Text(Date(timeIntervalSince1970: instant / 1000), style: .timer)
                        .monospacedDigit().frame(width: leaf.node.frame.width, alignment: .leading)
                        .offset(x: leaf.origin.x, y: leaf.origin.y)
                } else {
                    ForEach(Array(leaf.node.lines.enumerated()), id: \.offset) { _, line in
                        Text(line.text).font(leaf.node.runs.first.map { Font($0.font) } ?? .body)
                            .lineLimit(1).fixedSize()
                            .offset(x: leaf.origin.x + leaf.node.contentBox.minX + line.origin.x,
                                    y: leaf.origin.y + leaf.node.contentBox.minY + line.origin.y - line.ascent)
                    }
                }
            }
        }.frame(width: snapshot.size.width, height: snapshot.size.height, alignment: .topLeading)
    }
}
