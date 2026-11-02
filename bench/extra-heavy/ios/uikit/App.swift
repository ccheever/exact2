// Extra Heavy feed benchmark, UIKit (see ../SPEC.md, Decisions "UIKit").
// Hand-written UIKit as a performance-minded iOS engineer writes it: one UICollectionView with a
// compositional layout and a diffable data source, one reusable cell class per row kind with manual
// frame layout and self-sizing, prefetching + off-main ImageIO decode, and system views/layers for
// media (AVPlayerLayer, MKMapView, WKWebView, MTKView, Core Graphics, Core Animation, Lottie).
import UIKit

// MARK: - Data (the SwiftUI app's model, field for field)

struct Photo: Codable, Hashable { let src: String; let w: Double; let h: Double }
struct Stroke: Codable, Hashable { let p: [Double]; let color: String; let width: Double }
struct Dot: Codable, Hashable { let x: Double; let y: Double; let r: Double; let color: String }
struct Tok: Codable, Hashable { let t: String; let k: String }
struct Block: Codable, Hashable { let text: String; let dir: String }
struct Card: Codable, Hashable { let image: String; let title: String; let meta: String }
struct FontSpec: Codable, Hashable { let file: String; let name: String; let size: Double; let family: String }

struct Row: Codable, Hashable {
    var id: String
    let index: Int
    let kind: String
    let author: String
    let handle: String
    let avatar: String
    let minutesAgo: Int
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

// MARK: - Environment, clocks (as the SwiftUI app)

let env = ProcessInfo.processInfo.environment
let freeze = env["BENCH_FREEZE"] == "1"
let liveMode = !freeze && (env["BENCH_LIVE"] == "1" || env["BENCH_SCENARIO"] == "rest")
let startIndex = env["BENCH_START_INDEX"].flatMap(Int.init)
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
let launchTime = CACurrentMediaTime()
/// The motion clock t (SPEC "Clocks"): seconds since launch.
func motionTime() -> Double { CACurrentMediaTime() - launchTime }

/// Live clock s (whole seconds, 1 Hz while live) and the per-row app state that outlives a cell.
enum AppState {
    static var seconds = 0
    static var drafts: [String: String] = [:]
    static var expanded: [String: Bool] = [:]
    static var innerOffsets: [String: CGFloat] = [:]
}

var screenScale: CGFloat = 2

// MARK: - Colours, fonts

extension UIColor {
    convenience init(hex: UInt32, alpha: CGFloat = 1) {
        self.init(red: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
                  blue: CGFloat(hex & 0xFF) / 255, alpha: alpha)
    }
    convenience init(hexString s: String) { self.init(hex: UInt32(s.dropFirst(), radix: 16) ?? 0) }
    static let hairline = UIColor(hex: 0xE5E5EA)
    static let secondaryGray = UIColor(hex: 0x8E8E93)
    static let label2 = UIColor(hex: 0x3C3C43)
    static let xfill = UIColor(hex: 0xF2F2F7)
    static let border = UIColor(hex: 0xD1D1D6)
    static let xblue = UIColor(hex: 0x007AFF)
    static let ink = UIColor(hex: 0x1C1C1E)
    static let gray3 = UIColor(hex: 0xC7C7CC)
}

func sys(_ size: CGFloat, _ weight: UIFont.Weight = .regular) -> UIFont { .systemFont(ofSize: size, weight: weight) }
func italic(_ f: UIFont) -> UIFont {
    f.fontDescriptor.withSymbolicTraits(f.fontDescriptor.symbolicTraits.union(.traitItalic)).map { UIFont(descriptor: $0, size: f.pointSize) } ?? f
}
func monoDigits(_ f: UIFont) -> UIFont { .monospacedDigitSystemFont(ofSize: f.pointSize, weight: .regular) }
let spaceMono = { (size: CGFloat) in UIFont(name: "SpaceMono-Regular", size: size) ?? .monospacedSystemFont(ofSize: size, weight: .regular) }

func label(_ font: UIFont, _ color: UIColor = .black, lines: Int = 1) -> UILabel {
    let l = UILabel()
    l.font = font
    l.textColor = color
    l.numberOfLines = lines
    l.lineBreakMode = .byTruncatingTail
    return l
}

/// Height of a label's text at width w (respects numberOfLines).
func fit(_ l: UILabel, _ w: CGFloat) -> CGFloat { ceil(l.sizeThatFits(CGSize(width: w, height: .greatestFiniteMagnitude)).height) }

// MARK: - App, scene

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    func application(_ application: UIApplication,
                     didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?) -> Bool { true }
}

@objc(SceneDelegate) final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options: UIScene.ConnectionOptions) {
        guard let ws = scene as? UIWindowScene else { return }
        screenScale = ws.screen.scale
        let w = UIWindow(windowScene: ws)
        w.overrideUserInterfaceStyle = .light
        w.rootViewController = FeedViewController()
        w.makeKeyAndVisible()
        window = w
    }
}

// MARK: - Relative time

func relativeTime(_ minutesAgo: Int, s: Int) -> String {
    let m = minutesAgo + s / 60
    if m == 0 { return "now" }
    if m < 60 { return "\(m)m" }
    if m < 1440 { return "\(m / 60)h" }
    return "\(m / 1440)d"
}
