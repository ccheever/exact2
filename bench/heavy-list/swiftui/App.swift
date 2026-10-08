// Heavy list benchmark, SwiftUI baseline (see ../SPEC.md).
// The most ordinary SwiftUI: a plain List + ForEach over an @State array,
// one Text(AttributedString) per paragraph, a task-driven thumbnail loader,
// a minimal custom Layout for the reaction chips.
import SwiftUI
import ImageIO
import UniformTypeIdentifiers

// MARK: - Data

struct Run: Codable, Hashable {
    let t: String
    let s: String?
}

struct Photo: Codable, Hashable {
    let src: String
    let w: Double
    let h: Double
}

struct LinkCard: Codable, Hashable {
    let thumb: String
    let title: String
    let description: String
    let site: String
}

struct Quote: Codable, Hashable {
    let id: String
    let author: String
    let excerpt: String
}

struct Reaction: Codable, Hashable {
    let emoji: String
    var count: Int
}

struct Message: Codable, Identifiable, Hashable {
    var id: String
    var index: Int
    let author: String
    let avatar: String
    var minutesAgo: Int
    let paragraphs: [[Run]]
    let photos: [Photo]?
    let link: LinkCard?
    let quote: Quote?
    var reactions: [Reaction]?
    /// Seconds after launch at which `minutesAgo` was true (live inserts).
    var bornAt: Int? = nil
}

private struct MessageFile: Codable {
    let version: Int
    let messages: [Message]
}

private func loadMessages() -> [Message] {
    guard let url = Bundle.main.url(forResource: "messages", withExtension: "json"),
          let data = try? Data(contentsOf: url),
          let file = try? JSONDecoder().decode(MessageFile.self, from: data)
    else { return [] }
    return file.messages
}

// MARK: - Colors

extension Color {
    init(hex: UInt32) {
        self.init(.sRGB,
                  red: Double((hex >> 16) & 0xFF) / 255,
                  green: Double((hex >> 8) & 0xFF) / 255,
                  blue: Double(hex & 0xFF) / 255)
    }
    static let hairline = Color(hex: 0xE5E5EA)
    static let secondaryGray = Color(hex: 0x8E8E93)
    static let quoteBar = Color(hex: 0xC7C7CC)
    static let fill = Color(hex: 0xF2F2F7)
    static let label2 = Color(hex: 0x3C3C43)
    static let cardBorder = Color(hex: 0xD1D1D6)
    static let linkBlue = Color(hex: 0x007AFF)
    static let tagPurple = Color(hex: 0x5856D6)
}

// MARK: - App

@main
struct HeavyBenchApp: App {
    var body: some Scene { WindowGroup { ContentView() } }
}

private let env = ProcessInfo.processInfo.environment
private let liveMode = env["BENCH_LIVE"] == "1"
private let startIndex = env["BENCH_START_INDEX"].flatMap(Int.init)

struct ContentView: View {
    @State private var messages: [Message] = loadMessages()
    @State private var seconds = 0      // seconds since launch; ticks only in live mode
    @State private var inserted = 0     // k of the last live insert

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Heavy list").font(.system(size: 17, weight: .semibold))
                Spacer()
                Text(liveMode ? "Live: on" : "Live: off")
                    .font(.system(size: 13))
                    .foregroundStyle(Color.secondaryGray)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            .background(.white)
            Divider()

            GeometryReader { geo in
                let contentWidth = geo.size.width - 16 - 40 - 12 - 16
                ScrollViewReader { proxy in
                    List {
                        ForEach($messages) { $message in
                            MessageRow(message: $message, seconds: seconds, contentWidth: contentWidth)
                                .listRowInsets(EdgeInsets(top: 12, leading: 16, bottom: 12, trailing: 16))
                                .listRowSeparatorTint(.hairline)
                                .alignmentGuide(.listRowSeparatorLeading) { _ in 68 - 16 }
                                .listRowBackground(Color.white)
                        }
                    }
                    .listStyle(.plain)
                    .task {
                        if let startIndex, messages.indices.contains(startIndex) {
                            proxy.scrollTo(messages[startIndex].id, anchor: .top)
                        }
                        await runLive()
                    }
                }
            }
        }
        .background(.white)
        .preferredColorScheme(.light)
    }

    /// Live mode: every 250 ms one insert at the top and one reaction bump; seconds tick every 4th step.
    private func runLive() async {
        guard liveMode else { return }
        var ticks = 0
        while !Task.isCancelled {
            try? await Task.sleep(for: .milliseconds(250))
            ticks += 1
            // Plain List: no position-keeping here. List keeps its content offset across an
            // insert above, so the visible rows move down one row per insert (see README).
            liveStep()
            if ticks % 4 == 0 { seconds += 1 }
        }
    }

    /// One 250 ms live step: insert at the top, bump one reaction.
    private func liveStep() {
        let k = inserted + 1
        inserted = k
        let base = messages[inserted - 1 + (k * 37) % 10_000]   // original message (k*37) % 10000
        var copy = base
        copy.id = "live-\(k)"
        copy.index = -k
        copy.minutesAgo = 0
        copy.bornAt = seconds
        messages.insert(copy, at: 0)

        // Bump: index (k*101) % 10000, or the next original message that has reactions.
        var i = k + (k * 101) % 10_000
        while i < messages.count, (messages[i].reactions ?? []).isEmpty { i += 1 }
        if i < messages.count, let n = messages[i].reactions?.count {
            messages[i].reactions![k % n].count += 1
        }
    }
}

func relativeTime(_ m: Message, seconds: Int) -> String {
    let minutes = m.minutesAgo + (seconds - (m.bornAt ?? 0)) / 60
    if minutes < 60 { return "\(minutes)m ago" }
    if minutes < 60 * 24 { return "\(minutes / 60)h ago" }
    return "\(minutes / (60 * 24))d ago"
}

// MARK: - Row

struct MessageRow: View {
    @Binding var message: Message
    let seconds: Int
    let contentWidth: CGFloat

    var body: some View {
        let time = relativeTime(message, seconds: seconds)
        HStack(alignment: .top, spacing: 12) {
            BundleImage(name: message.avatar, size: CGSize(width: 40, height: 40))
                .clipShape(Circle())

            VStack(alignment: .leading, spacing: 0) {
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Text(message.author).font(.system(size: 15, weight: .semibold)).foregroundStyle(.black)
                    Text(time).font(.system(size: 13)).foregroundStyle(Color.secondaryGray)
                }
                .lineLimit(1)

                if let quote = message.quote {
                    QuoteBox(quote: quote).padding(.top, 6)
                }

                VStack(alignment: .leading, spacing: 8) {
                    ForEach(message.paragraphs.indices, id: \.self) { i in
                        Text(paragraphText(message.paragraphs[i]))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                .padding(.top, 6)

                if let photos = message.photos, !photos.isEmpty {
                    PhotoGrid(photos: photos, width: contentWidth).padding(.top, 8)
                }
                if let link = message.link {
                    LinkCardView(link: link, width: contentWidth).padding(.top, 8)
                }
                if let reactions = message.reactions, !reactions.isEmpty {
                    FlowLayout(spacing: 6) {
                        ForEach(reactions.indices, id: \.self) { i in
                            Button {
                                message.reactions![i].count += 1
                            } label: {
                                HStack(spacing: 4) {
                                    Text(reactions[i].emoji).font(.system(size: 14))
                                    Text("\(reactions[i].count)")
                                        .font(.system(size: 13, weight: .semibold))
                                        .foregroundStyle(Color.label2)
                                }
                                .padding(.horizontal, 10)
                                .frame(height: 28)
                                .background(Color.fill, in: RoundedRectangle(cornerRadius: 14))
                            }
                            .buttonStyle(.borderless)
                        }
                    }
                    .padding(.top, 8)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("\(message.author), \(time)")
    }
}

/// One paragraph as one AttributedString: runs concatenated with per-run attributes.
func paragraphText(_ runs: [Run]) -> AttributedString {
    var out = AttributedString()
    for run in runs {
        var a = AttributedString(run.t)
        a.font = .system(size: 16)
        a.foregroundColor = .black
        switch run.s {
        case "bold": a.font = .system(size: 16, weight: .semibold)
        case "italic": a.font = .system(size: 16).italic()
        case "code":
            a.font = .system(size: 15, design: .monospaced)
            a.backgroundColor = .fill
        case "link":
            a.foregroundColor = .linkBlue
            a.underlineStyle = .single
        case "mention":
            a.font = .system(size: 16, weight: .semibold)
            a.foregroundColor = .linkBlue
        case "tag": a.foregroundColor = .tagPurple
        default: break
        }
        out += a
    }
    return out
}

struct QuoteBox: View {
    let quote: Quote
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(quote.author).font(.system(size: 13, weight: .semibold))
            Text(quote.excerpt).font(.system(size: 13)).lineLimit(2)
        }
        .foregroundStyle(Color.label2)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(8)
        .padding(.leading, 3)
        .background(alignment: .leading) {
            ZStack(alignment: .leading) {
                Color.fill
                Color.quoteBar.frame(width: 3)
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}

struct PhotoGrid: View {
    let photos: [Photo]
    let width: CGFloat

    var body: some View {
        Group {
            switch photos.count {
            case 1:
                let p = photos[0]
                BundleImage(name: p.src, size: CGSize(width: width, height: min(width * p.h / p.w, 320)))
            case 2:
                let s = (width - 4) / 2
                HStack(spacing: 4) {
                    ForEach(photos, id: \.self) { BundleImage(name: $0.src, size: CGSize(width: s, height: s)) }
                }
            case 3:
                let small = (width - 4) / 3
                let big = width - 4 - small
                HStack(spacing: 4) {
                    BundleImage(name: photos[0].src, size: CGSize(width: big, height: big))
                    VStack(spacing: 4) {
                        let h = (big - 4) / 2
                        BundleImage(name: photos[1].src, size: CGSize(width: small, height: h))
                        BundleImage(name: photos[2].src, size: CGSize(width: small, height: h))
                    }
                }
            default:
                let s = (width - 4) / 2
                VStack(spacing: 4) {
                    HStack(spacing: 4) {
                        BundleImage(name: photos[0].src, size: CGSize(width: s, height: s))
                        BundleImage(name: photos[1].src, size: CGSize(width: s, height: s))
                    }
                    HStack(spacing: 4) {
                        BundleImage(name: photos[2].src, size: CGSize(width: s, height: s))
                        BundleImage(name: photos[3].src, size: CGSize(width: s, height: s))
                    }
                }
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }
}

struct LinkCardView: View {
    let link: LinkCard
    let width: CGFloat
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            BundleImage(name: link.thumb, size: CGSize(width: width, height: 140))
            VStack(alignment: .leading, spacing: 0) {
                Text(link.site.uppercased()).font(.system(size: 12)).foregroundStyle(Color.secondaryGray)
                Text(link.title).font(.system(size: 15, weight: .semibold)).foregroundStyle(.black).lineLimit(2)
                Text(link.description).font(.system(size: 13)).foregroundStyle(Color.label2).lineLimit(2)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(10)
        }
        .clipShape(RoundedRectangle(cornerRadius: 12))
        .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Color.cardBorder, lineWidth: 0.5))
    }
}

// MARK: - Flow layout (reaction chips)

struct FlowLayout: Layout {
    var spacing: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let rows = arrange(width: proposal.width ?? .infinity, subviews: subviews)
        return CGSize(width: proposal.width ?? rows.maxWidth, height: rows.height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let rows = arrange(width: bounds.width, subviews: subviews)
        for (i, p) in rows.origins.enumerated() {
            subviews[i].place(at: CGPoint(x: bounds.minX + p.x, y: bounds.minY + p.y), proposal: .unspecified)
        }
    }

    private func arrange(width: CGFloat, subviews: Subviews) -> (origins: [CGPoint], height: CGFloat, maxWidth: CGFloat) {
        var origins: [CGPoint] = []
        var x: CGFloat = 0, y: CGFloat = 0, lineHeight: CGFloat = 0, maxWidth: CGFloat = 0
        for sub in subviews {
            let size = sub.sizeThatFits(.unspecified)
            if x > 0 && x + size.width > width {
                x = 0
                y += lineHeight + spacing
                lineHeight = 0
            }
            origins.append(CGPoint(x: x, y: y))
            x += size.width + spacing
            lineHeight = max(lineHeight, size.height)
            maxWidth = max(maxWidth, x - spacing)
        }
        return (origins, y + lineHeight, maxWidth)
    }
}

// MARK: - Images: downsampled off the main thread, small in-memory cache

final class ThumbnailCache: @unchecked Sendable {
    static let shared = ThumbnailCache()
    private let cache: NSCache<NSString, UIImage> = {
        let c = NSCache<NSString, UIImage>()
        c.countLimit = 300
        return c
    }()

    func cached(_ key: String) -> UIImage? { cache.object(forKey: key as NSString) }

    /// Decode `name` from the bundle so that it aspect-fills `pixels`.
    func load(_ name: String, pixels: CGSize, key: String) async -> UIImage? {
        if let hit = cached(key) { return hit }
        let image = await Task.detached(priority: .userInitiated) { () -> UIImage? in
            guard let url = Bundle.main.url(forResource: name, withExtension: nil),
                  let src = CGImageSourceCreateWithURL(url as CFURL, [kCGImageSourceShouldCache: false] as CFDictionary),
                  let props = CGImageSourceCopyPropertiesAtIndex(src, 0, nil) as? [CFString: Any],
                  let iw = props[kCGImagePropertyPixelWidth] as? CGFloat,
                  let ih = props[kCGImagePropertyPixelHeight] as? CGFloat
            else { return nil }
            let fill = max(pixels.width / iw, pixels.height / ih)
            let maxPixel = ceil(max(iw, ih) * min(fill, 1))
            let opts: [CFString: Any] = [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceShouldCacheImmediately: true,
                kCGImageSourceThumbnailMaxPixelSize: maxPixel,
            ]
            guard let cg = CGImageSourceCreateThumbnailAtIndex(src, 0, opts as CFDictionary) else { return nil }
            return UIImage(cgImage: cg)
        }.value
        if let image { cache.setObject(image, forKey: key as NSString) }
        return image
    }
}

struct BundleImage: View {
    let name: String
    let size: CGSize
    @Environment(\.displayScale) private var scale
    @State private var image: UIImage?

    private var key: String { "\(name)@\(Int(size.width * scale))x\(Int(size.height * scale))" }

    var body: some View {
        ZStack {
            Color.hairline
            if let image {
                Image(uiImage: image).resizable().scaledToFill()
            }
        }
        .frame(width: size.width, height: size.height)
        .clipped()
        .task(id: key) {
            if let hit = ThumbnailCache.shared.cached(key) { image = hit; return }
            image = nil
            image = await ThumbnailCache.shared.load(
                name, pixels: CGSize(width: size.width * scale, height: size.height * scale), key: key)
        }
    }
}
