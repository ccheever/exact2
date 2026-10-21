// Extra Heavy feed benchmark, SwiftUI baseline (see ../SPEC.md, Decisions included).
// The most ordinary SwiftUI: a plain List + ForEach over an @State array, one switch on the row
// kind, system views for everything SwiftUI has (Map, Canvas, TimelineView, materials, layer
// effects, TextField) and NSViewRepresentable where it has none (macOS port: ../SPEC.md "macOS equivalences") (video, animated images, web view).
import Combine
import SwiftUI

// MARK: - Data

struct Photo: Codable, Hashable { let src: String; let w: Double; let h: Double }
struct Stroke: Codable, Hashable { let p: [Double]; let color: String; let width: Double }
struct Dot: Codable, Hashable { let x: Double; let y: Double; let r: Double; let color: String }
struct Tok: Codable, Hashable { let t: String; let k: String }
struct Block: Codable, Hashable { let text: String; let dir: String }
struct Card: Codable, Hashable { let image: String; let title: String; let meta: String }
struct FontSpec: Codable, Hashable { let file: String; let name: String; let size: Double; let family: String }

struct Row: Codable, Identifiable, Hashable {
    var id: String
    let index: Int
    let kind: String
    let author: String
    let handle: String
    let avatar: String
    let minutesAgo: Int
    // kind payloads (SPEC "Row kinds")
    var caption: String?
    var photo: Photo?
    var title: String?
    var thumbs: [String]?
    var duoA: String?
    var duoB: String?
    var image: String?
    var bg: String?
    var band: [String]?
    var strokes: [Stroke]?
    var dots: [Dot]?
    var icons: [String]?
    var chart: String?
    var art: String?
    var video: String?
    var place: String?
    var address: String?
    var lat: Double?
    var lon: Double?
    var md: String?
    var file: String?
    var lang: String?
    var lines: [[Tok]]?
    var blocks: [Block]?
    var quote: String?
    var by: String?
    var font: String?
    var cards: [Card]?
    var gif: String?
    var webp: String?
    var lottie: String?
    var subtitle: String?
    var rating: String?
    var endsInSec: Int?
    var rings: [Double]?
    var wave: [Double]?
    var updatedSec: Int?
    var text: String?
    var hue: Int?
    var bars: [Int]?
    // 18 filmstrip, 19 inbox: the inner list's items are derived from these (SPEC)
    var count: Int?
    var img0: Int?
    var imgStep: Int?
    var num0: Int?
    var m0: Int?
    var mStep: Int?
    var clock0: Int?
}

struct Message: Codable, Hashable { let name: String; let avatar: String; let text: String }

struct Feed: Codable {
    let version: Int
    let fonts: [String: FontSpec]
    var messages: [Message]?
    let rows: [Row]
}

let feed: Feed = {
    guard let url = Bundle.main.url(forResource: "feed", withExtension: "json"),
          let data = try? Data(contentsOf: url),
          let f = try? JSONDecoder().decode(Feed.self, from: data)
    else { return Feed(version: 0, fonts: [:], rows: []) }
    return f
}()

// MARK: - Environment, clocks

let env = ProcessInfo.processInfo.environment
let freeze = env["BENCH_FREEZE"] == "1"
let liveMode = !freeze && (env["BENCH_LIVE"] == "1" || env["BENCH_SCENARIO"] == "rest")
let startIndex = env["BENCH_START_INDEX"].flatMap(Int.init)
/// BENCH_KINDS=17: leave out the nested-list kinds (filmstrip, inbox), so every stack renders the
/// same 17 kinds (exact2 has no nested virtualized list yet). Unset: all 19.
let kinds17 = env["BENCH_KINDS"] == "17"
/// BENCH_KINDS=<kind>[,<kind>…]: a feed of only those kinds, with the 17-kind feed's row count
/// (their rows in feed order, cycled; a repeat's id gets `-<cycle>`).
let benchRows: [Row] = {
    let rows17 = feed.rows.filter { $0.kind != "filmstrip" && $0.kind != "inbox" }
    guard let k = env["BENCH_KINDS"], !k.isEmpty else { return feed.rows }
    if k == "17" { return rows17 }
    let want = Set(k.split(separator: ",").map { String($0).trimmingCharacters(in: .whitespaces) })
    let pick = feed.rows.filter { want.contains($0.kind) }
    guard !pick.isEmpty else { return rows17 }
    return (0..<rows17.count).map { i in
        var r = pick[i % pick.count]
        let cycle = i / pick.count
        if cycle > 0 { r.id += "-\(cycle)" }
        return r
    }
}()
let launchDate = Date()

/// The motion clock t (SPEC "Clocks"): seconds since launch.
func motionTime(_ date: Date) -> Double { date.timeIntervalSince(launchDate) }

// MARK: - Colours

extension Color {
    init(hex: UInt32) {
        self.init(.sRGB, red: Double((hex >> 16) & 0xFF) / 255, green: Double((hex >> 8) & 0xFF) / 255,
                  blue: Double(hex & 0xFF) / 255)
    }
    init(hexString s: String) { self.init(hex: UInt32(s.dropFirst(), radix: 16) ?? 0) }
    static let hairline = Color(hex: 0xE5E5EA)
    static let secondaryGray = Color(hex: 0x8E8E93)
    static let label2 = Color(hex: 0x3C3C43)
    static let xfill = Color(hex: 0xF2F2F7)
    static let border = Color(hex: 0xD1D1D6)
    static let xblue = Color(hex: 0x007AFF)
    static let ink = Color(hex: 0x1C1C1E)
}

func rgb(_ s: String) -> SIMD3<Float> {
    let v = UInt32(s.dropFirst(), radix: 16) ?? 0
    return SIMD3(Float((v >> 16) & 0xFF) / 255, Float((v >> 8) & 0xFF) / 255, Float(v & 0xFF) / 255)
}

// MARK: - App

/// The window's content size (macOS port): BENCH_WIN_W × BENCH_WIN_H points, default 1366 × 1000, the same
/// numbers exact2's app gets as EXACT_WINDOW_WIDTH/HEIGHT (the landscape iPad's width; the height fits a 1080 p display).
let winW = env["BENCH_WIN_W"].flatMap(Double.init) ?? 1366
let winH = env["BENCH_WIN_H"].flatMap(Double.init) ?? 1000

@main
struct XHeavyApp: App {
    var body: some Scene {
        WindowGroup { ContentView() }
            .defaultSize(width: winW, height: winH)
            .defaultPosition(.topLeading)
    }
}

/// App state that outlives a row's view: comment drafts and thread expansion, keyed by row id.
@Observable final class RowState {
    var drafts: [String: String] = [:]
    var expanded: [String: Bool] = [:]
}

struct ContentView: View {
    @State private var rows: [Row] = benchRows
    @State private var seconds = 0
    @State private var state = RowState()
    private let timer = Timer.publish(every: 1, on: .main, in: .common).autoconnect()

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Extra Heavy").font(.system(size: 17, weight: .semibold))
                Spacer()
                Text(liveMode ? "Live: on" : "Live: off").font(.system(size: 13)).foregroundStyle(Color.secondaryGray)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            Rectangle().fill(Color.hairline).frame(height: 0.5)

            GeometryReader { geo in
                let column = min(geo.size.width - 32, 600)
                ScrollViewReader { proxy in
                    List {
                        ForEach(rows) { row in
                            RowView(row: row, s: seconds, C: column, state: state)
                                .frame(maxWidth: .infinity)
                                .listRowInsets(EdgeInsets(top: 16, leading: 0, bottom: 16, trailing: 0))
                                .listRowSeparator(.visible)
                                .listRowSeparatorTint(.hairline)
                                .alignmentGuide(.listRowSeparatorLeading) { _ in 0 }
                                .alignmentGuide(.listRowSeparatorTrailing) { d in d.width }
                                .listRowBackground(Color.white)
                        }
                    }
                    .listStyle(.plain)
                    .task {
                        // List estimates unmeasured row heights, so one scrollTo lands near, not on, the row;
                        // repeat it once the rows around the target have been measured.
                        guard let startIndex, rows.indices.contains(startIndex) else { return }
                        for _ in 0..<3 {
                            proxy.scrollTo(rows[startIndex].id, anchor: .top)
                            try? await Task.sleep(for: .milliseconds(250))
                        }
                    }
                }
            }
        }
        .background(.white)
        .preferredColorScheme(.light)
        .onReceive(timer) { _ in
            guard liveMode else { return }
            seconds += 1
        }
    }
}

// MARK: - Row

func relativeTime(_ minutesAgo: Int, s: Int) -> String {
    let m = minutesAgo + s / 60
    if m == 0 { return "now" }
    if m < 60 { return "\(m)m" }
    if m < 1440 { return "\(m / 60)h" }
    return "\(m / 1440)d"
}

struct RowView: View {
    let row: Row
    let s: Int
    let C: CGFloat
    let state: RowState

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            header
            kindBody(row.kind)
        }
        .frame(width: C, alignment: .leading)
    }

    private var header: some View {
        HStack(spacing: 10) {
            BundleImage(name: row.avatar, size: CGSize(width: 36, height: 36)).clipShape(Circle())
            VStack(alignment: .leading, spacing: 2) {
                Text(row.author).font(.system(size: 15, weight: .semibold)).foregroundStyle(.black)
                Text("\(row.handle) · \(relativeTime(row.minutesAgo, s: s))")
                    .font(.system(size: 13)).foregroundStyle(Color.secondaryGray)
            }
            .lineLimit(1)
            Spacer(minLength: 8)
            Text(row.kind.uppercased()).font(.system(size: 11, weight: .semibold)).foregroundStyle(Color.secondaryGray)
        }
        .frame(height: 36)
    }

    @ViewBuilder private func kindBody(_ kind: String) -> some View {
        switch kind {
        case "photo": PhotoBody(row: row, C: C)
        case "thumbs": ThumbsBody(row: row, C: C)
        case "shader": ShaderBody(row: row, C: C)
        case "canvas": CanvasBody(row: row, C: C)
        case "svg": SvgBody(row: row, C: C)
        case "video": VideoBody(row: row, C: C)
        case "map": MapBody(row: row, C: C)
        case "markdown": MarkdownBody(md: row.md ?? "")
        case "code": CodeBody(row: row)
        case "intl": IntlBody(row: row)
        case "typeface": TypefaceBody(row: row)
        case "carousel": CarouselBody(row: row)
        case "motion": MotionBody(row: row, C: C)
        case "glass": GlassBody(row: row, C: C)
        case "live": LiveBody(row: row, s: s)
        case "thread": ThreadBody(row: row, s: s, state: state)
        case "webview": WebBody(row: row, C: C)
        case "filmstrip": FilmstripBody(row: row, C: C)
        case "inbox": InboxBody(row: row, C: C)
        default: EmptyView()
        }
    }
}
