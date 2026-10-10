// Kinds 18 and 19 (SPEC "Row kinds"): a list row that holds its own virtualized list.
// The ordinary SwiftUI: a ScrollView over a lazy stack inside the List row, its offset kept per row
// id through ScrollPosition + onScrollGeometryChange (iOS 18; on iOS 17 a strip starts at 0).
import SwiftUI

/// Inner-list offsets by row id. App state, not observed: nothing redraws when it changes.
final class ScrollMemory {
    static let shared = ScrollMemory()
    var offsets: [String: CGFloat] = [:]
}

/// Restores the inner scroll view's offset (x or y) for `id` when the row appears, and records it as it scrolls.
struct KeepsOffset: ViewModifier {
    let id: String
    let horizontal: Bool
    func body(content: Content) -> some View {
        if #available(iOS 18.0, *) { content.modifier(KeepsOffset18(id: id, horizontal: horizontal)) } else { content }
    }
}

@available(iOS 18.0, *)
struct KeepsOffset18: ViewModifier {
    let id: String
    let horizontal: Bool
    @State private var position: ScrollPosition

    init(id: String, horizontal: Bool) {
        self.id = id
        self.horizontal = horizontal
        let saved = ScrollMemory.shared.offsets[id] ?? 0
        _position = State(initialValue: horizontal ? ScrollPosition(x: saved) : ScrollPosition(y: saved))
    }

    func body(content: Content) -> some View {
        content
            .scrollPosition($position)
            .onScrollGeometryChange(for: CGFloat.self) { g in
                horizontal ? g.contentOffset.x + g.contentInsets.leading : g.contentOffset.y + g.contentInsets.top
            } action: { _, v in
                ScrollMemory.shared.offsets[id] = v
            }
            .onAppear {
                let saved = ScrollMemory.shared.offsets[id] ?? 0
                if horizontal { position.scrollTo(x: saved) } else { position.scrollTo(y: saved) }
            }
    }
}

// MARK: 18 filmstrip

func filmImage(_ row: Row, _ j: Int) -> String {
    String(format: "small-%02d.jpg", (row.img0! + j * row.imgStep!) % 48)
}

struct FilmstripBody: View {
    let row: Row
    let C: CGFloat
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(row.title ?? "").font(.system(size: 15, weight: .semibold)).foregroundStyle(.black)
            ScrollView(.horizontal, showsIndicators: false) {
                LazyHStack(alignment: .top, spacing: 8) {
                    ForEach(0..<(row.count ?? 0), id: \.self) { j in
                        VStack(alignment: .leading, spacing: 4) {
                            BundleImage(name: filmImage(row, j), size: CGSize(width: 112, height: 112))
                                .clipShape(RoundedRectangle(cornerRadius: 8))
                            Text(verbatim: "IMG_\(row.num0! + j)").font(.system(size: 12)).foregroundStyle(Color.label2)
                                .lineLimit(1)
                        }
                        .frame(width: 112, height: 136, alignment: .topLeading)
                    }
                }
            }
            .modifier(KeepsOffset(id: row.id, horizontal: true))
            .frame(width: C, height: 136)
        }
    }
}

// MARK: 19 inbox

func clockLabel(_ row: Row, _ j: Int) -> String {
    let m = ((row.clock0! - 7 * j) % 1440 + 1440) % 1440
    return String(format: "%02d:%02d", m / 60, m % 60)
}

struct InboxBody: View {
    let row: Row
    let C: CGFloat
    var body: some View {
        let pool = feed.messages ?? []
        VStack(alignment: .leading, spacing: 10) {
            Text(row.title ?? "").font(.system(size: 15, weight: .semibold)).foregroundStyle(.black)
            ScrollView(.vertical) {
                LazyVStack(spacing: 0) {
                    ForEach(0..<(pool.isEmpty ? 0 : row.count ?? 0), id: \.self) { j in
                        MessageRow(m: pool[(row.m0! + j * row.mStep!) % pool.count], time: clockLabel(row, j))
                    }
                }
            }
            .modifier(KeepsOffset(id: row.id, horizontal: false))
            .frame(width: C, height: 400)
            .background(Color.white)
            .clipShape(RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Color.border, lineWidth: 0.5))
        }
    }
}

struct MessageRow: View {
    let m: Message
    let time: String
    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            BundleImage(name: m.avatar, size: CGSize(width: 32, height: 32)).clipShape(Circle())
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 8) {
                    Text(m.name).font(.system(size: 14, weight: .semibold)).foregroundStyle(.black).lineLimit(1)
                    Spacer(minLength: 0)
                    Text(time).font(.system(size: 12)).foregroundStyle(Color.secondaryGray).monospacedDigit()
                }
                Text(m.text).font(.system(size: 13)).foregroundStyle(Color.label2).lineLimit(2)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .padding(.vertical, 10)
        .padding(.horizontal, 12)
        .overlay(alignment: .bottom) {
            Rectangle().fill(Color.hairline).frame(height: 0.5).padding(.leading, 54)
        }
    }
}
