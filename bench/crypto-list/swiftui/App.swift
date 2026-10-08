// Crypto list benchmark, SwiftUI baseline (see ../SPEC.md, "Decisions" included).
// The most ordinary SwiftUI: a plain List + ForEach over an @State array; the sparkline is
// a Path shape drawn in with .trim, the pulse a .repeatForever animation, the price flash
// a keyframe animation triggered by each update.
import SwiftUI
import Combine

// MARK: - Data

struct Coin: Codable, Identifiable, Hashable {
    let id: String
    let name: String
    let ticker: String
    let color: String
    var price: Double
    var change24h: Double
    var series: [Double]
    /// Bumped on every live update; the price flashes when it changes.
    var flashSeq: Int = 0
    var flashUp: Bool = true

    enum CodingKeys: String, CodingKey { case id, name, ticker, color, price, change24h, series }
}

private struct CoinFile: Codable {
    let version: Int
    let coins: [Coin]
}

private func loadCoins() -> [Coin] {
    guard let url = Bundle.main.url(forResource: "coins", withExtension: "json"),
          let data = try? Data(contentsOf: url),
          let file = try? JSONDecoder().decode(CoinFile.self, from: data)
    else { return [] }
    return file.coins
}

// MARK: - Colors and formats

extension Color {
    init(hex: UInt32) {
        self.init(.sRGB,
                  red: Double((hex >> 16) & 0xFF) / 255,
                  green: Double((hex >> 8) & 0xFF) / 255,
                  blue: Double(hex & 0xFF) / 255)
    }
    init(hexString s: String) {
        self.init(hex: UInt32(s.dropFirst(), radix: 16) ?? 0)
    }
    static let hairline = Color(hex: 0xE5E5EA)
    static let secondaryGray = Color(hex: 0x8E8E93)
    static let up = Color(hex: 0x16A34A)
    static let down = Color(hex: 0xDC2626)
}

private let grouped: NumberFormatter = {
    let f = NumberFormatter()
    f.locale = Locale(identifier: "en_US")
    f.numberStyle = .decimal
    f.minimumFractionDigits = 2
    f.maximumFractionDigits = 2
    return f
}()

/// `$61,234.56` at or above $1; four significant digits below (`$0.0009697`).
func formatPrice(_ p: Double) -> String {
    if p >= 1 { return "$" + (grouped.string(from: NSNumber(value: p)) ?? "") }
    let decimals = max(0, 3 - Int(floor(log10(p))))
    return "$" + String(format: "%.*f", decimals, p)
}

func formatChange(_ c: Double) -> String {
    (c >= 0 ? "+" : "-") + String(format: "%.2f%%", abs(c))
}

// MARK: - App

@main
struct CryptoBenchApp: App {
    var body: some Scene { WindowGroup { ContentView() } }
}

private let env = ProcessInfo.processInfo.environment
private let freeze = env["BENCH_FREEZE"] == "1"
private let liveMode = !freeze && (env["BENCH_LIVE"] == "1" || env["BENCH_SCENARIO"] == "rest")
private let startIndex = env["BENCH_START_INDEX"].flatMap(Int.init)

struct ContentView: View {
    @State private var coins: [Coin] = loadCoins()
    @State private var tick = 0
    private let timer = Timer.publish(every: 0.1, on: .main, in: .common).autoconnect()

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Markets").font(.system(size: 17, weight: .semibold))
                Spacer()
                Text(liveMode ? "Live: on" : "Live: off")
                    .font(.system(size: 13))
                    .foregroundStyle(Color.secondaryGray)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            Rectangle().fill(Color.hairline).frame(height: 0.5)

            ScrollViewReader { proxy in
                List {
                    ForEach(coins) { coin in
                        CoinRow(coin: coin)
                            .listRowInsets(EdgeInsets())
                            .listRowSeparatorTint(.hairline)
                            .alignmentGuide(.listRowSeparatorLeading) { _ in 0 }
                            .alignmentGuide(.listRowSeparatorTrailing) { d in d.width }
                            .listRowBackground(Color.white)
                    }
                }
                .listStyle(.plain)
                .environment(\.defaultMinListRowHeight, 64)
                .onAppear {
                    if let startIndex, coins.indices.contains(startIndex) {
                        proxy.scrollTo(coins[startIndex].id, anchor: .top)
                    }
                }
            }
        }
        .background(.white)
        .preferredColorScheme(.light)
        .onReceive(timer) { _ in
            guard liveMode else { return }
            tick += 1
            liveTick(tick)
        }
    }

    /// Tick k updates 40 coins (SPEC "Live ticks").
    private func liveTick(_ k: Int) {
        let n = coins.count
        guard n > 0 else { return }
        for j in 0..<40 {
            let i = (k * 7919 + j * 104_729) % n
            let delta = Double((k * 31 + j * 17) % 201 - 100) / 10_000
            var c = coins[i]
            c.price *= 1 + delta
            c.series.removeFirst()
            c.series.append(c.price)
            c.change24h += delta * 100
            c.flashSeq += 1
            c.flashUp = delta >= 0
            coins[i] = c
        }
    }
}

// MARK: - Row

struct CoinRow: View {
    let coin: Coin

    var body: some View {
        let up = coin.change24h >= 0
        HStack(spacing: 12) {
            ZStack {
                Circle().fill(Color(hexString: coin.color))
                Text(String(coin.ticker.prefix(1)))
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(.white)
            }
            .frame(width: 32, height: 32)

            VStack(alignment: .leading, spacing: 2) {
                Text(coin.name).font(.system(size: 16, weight: .semibold)).foregroundStyle(.black)
                Text(coin.ticker).font(.system(size: 13)).foregroundStyle(Color.secondaryGray)
            }
            .lineLimit(1)
            .frame(maxWidth: .infinity, alignment: .leading)

            Sparkline(series: coin.series, color: up ? .up : .down)
                .frame(width: 96, height: 32)

            VStack(alignment: .trailing, spacing: 4) {
                Text(formatPrice(coin.price))
                    .font(.system(size: 16, weight: .semibold))
                    .monospacedDigit()
                    .lineLimit(1)
                    .keyframeAnimator(initialValue: 0.0, trigger: coin.flashSeq) { content, flash in
                        content.foregroundStyle(flashColor(up: coin.flashUp, amount: flash))
                    } keyframes: { _ in
                        MoveKeyframe(1.0)
                        // CSS `ease`
                        LinearKeyframe(0.0, duration: 0.4,
                                       timingCurve: UnitCurve.bezier(startControlPoint: UnitPoint(x: 0.25, y: 0.1),
                                                                     endControlPoint: UnitPoint(x: 0.25, y: 1)))
                    }
                Text(formatChange(coin.change24h))
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(.white)
                    .padding(.vertical, 2)
                    .padding(.horizontal, 6)
                    .background(up ? Color.up : Color.down, in: RoundedRectangle(cornerRadius: 4))
            }
            .frame(width: 104, alignment: .trailing)
        }
        .padding(.horizontal, 16)
        .frame(height: 64)
    }
}

/// #000 mixed toward the flash colour by `amount` (1 = the flash colour).
func flashColor(up: Bool, amount: Double) -> Color {
    let (r, g, b): (Double, Double, Double) = up ? (0x16, 0xA3, 0x4A) : (0xDC, 0x26, 0x26)
    return Color(.sRGB, red: r / 255 * amount, green: g / 255 * amount, blue: b / 255 * amount)
}

// MARK: - Sparkline

/// The series as a polyline in its frame: min at the bottom, max at the top.
struct SparkShape: Shape {
    let series: [Double]

    func path(in rect: CGRect) -> Path {
        Path { p in
            for (i, pt) in points(series, in: rect.size).enumerated() {
                if i == 0 { p.move(to: pt) } else { p.addLine(to: pt) }
            }
        }
    }
}

func points(_ series: [Double], in size: CGSize) -> [CGPoint] {
    guard let lo = series.min(), let hi = series.max(), series.count > 1 else { return [] }
    let n = Double(series.count - 1)
    return series.enumerated().map { i, v in
        let y = hi > lo ? size.height - size.height * (v - lo) / (hi - lo) : size.height / 2
        return CGPoint(x: size.width * Double(i) / n, y: y)
    }
}

struct Sparkline: View {
    let series: [Double]
    let color: Color

    @State private var drawn: CGFloat = freeze ? 1 : 0
    @State private var pulsing = false

    var body: some View {
        let last = points(series, in: CGSize(width: 96, height: 32)).last ?? .zero
        ZStack(alignment: .topLeading) {
            SparkShape(series: series)
                .trim(from: 0, to: drawn)
                .stroke(color, style: StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round))
            if freeze {
                Circle().fill(color).opacity(0.25).frame(width: 12, height: 12).position(last)
                Circle().fill(color).frame(width: 6, height: 6).position(last)
            } else if drawn == 1 {
                // The ring: an 18 pt disc scaled 1/3 → 1 (radius 3 → 9), opacity 0.5 → 0.
                Circle().fill(color)
                    .frame(width: 18, height: 18)
                    .scaleEffect(pulsing ? 1 : 1.0 / 3)
                    .opacity(pulsing ? 0 : 0.5)
                    .animation(.easeOut(duration: 1.2).repeatForever(autoreverses: false), value: pulsing)
                    .position(last)
                Circle().fill(color).frame(width: 6, height: 6).position(last)
            }
        }
        .onAppear {
            guard !freeze else { return }
            withAnimation(.easeOut(duration: 0.6)) {
                drawn = 1
            } completion: {
                pulsing = true
            }
        }
        .onDisappear {
            guard !freeze else { return }
            drawn = 0
            pulsing = false
        }
    }
}
