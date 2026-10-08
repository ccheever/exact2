// The 17 row kinds (SPEC "Row kinds"), SwiftUI side.
import MapKit
import SwiftUI

// MARK: 1 photo

struct PhotoBody: View {
    let row: Row
    let C: CGFloat
    var body: some View {
        let p = row.photo!
        VStack(alignment: .leading, spacing: 10) {
            Text(row.caption ?? "").font(.system(size: 15)).foregroundStyle(.black)
                .fixedSize(horizontal: false, vertical: true)
            BundleImage(name: p.src, size: CGSize(width: C, height: min(C * p.h / p.w, 1.25 * C)))
                .clipShape(RoundedRectangle(cornerRadius: 12))
        }
    }
}

// MARK: 2 thumbs (skeleton shimmer, then the images)

struct ThumbsBody: View {
    let row: Row
    let C: CGFloat
    @State private var loaded = freeze
    @State private var phase: CGFloat = 0

    var body: some View {
        let side = (C - 12) / 4
        let grid = VStack(spacing: 4) {
            ForEach(0..<3, id: \.self) { r in
                HStack(spacing: 4) {
                    ForEach(0..<4, id: \.self) { c in
                        RoundedRectangle(cornerRadius: 8).frame(width: side, height: side)
                    }
                }
            }
        }
        VStack(alignment: .leading, spacing: 10) {
            Text(row.title ?? "").font(.system(size: 15, weight: .semibold)).foregroundStyle(.black)
            ZStack(alignment: .topLeading) {
                if loaded {
                    VStack(spacing: 4) {
                        ForEach(0..<3, id: \.self) { r in
                            HStack(spacing: 4) {
                                ForEach(0..<4, id: \.self) { c in
                                    BundleImage(name: row.thumbs![r * 4 + c], size: CGSize(width: side, height: side))
                                        .clipShape(RoundedRectangle(cornerRadius: 8))
                                }
                            }
                        }
                    }
                    .transition(.opacity.animation(.linear(duration: 0.2)))
                } else {
                    ZStack(alignment: .topLeading) {
                        Color.hairline
                        LinearGradient(stops: [.init(color: .white.opacity(0), location: 0),
                                               .init(color: .white.opacity(0.65), location: 0.5),
                                               .init(color: .white.opacity(0), location: 1)],
                                       startPoint: .leading, endPoint: .trailing)
                            .frame(width: C * 0.4)
                            .offset(x: -C * 0.4 + phase * C * 1.4)
                    }
                    .frame(width: C, height: side * 3 + 8)
                    .mask(grid)
                    .onAppear {
                        withAnimation(.linear(duration: 1).repeatForever(autoreverses: false)) { phase = 1 }
                    }
                }
            }
            .frame(width: C, height: side * 3 + 8, alignment: .topLeading)
        }
        .task(id: row.id) {
            guard !freeze else { return }
            loaded = false
            phase = 0
            try? await Task.sleep(for: .milliseconds(1200))
            if !Task.isCancelled { withAnimation(.linear(duration: 0.2)) { loaded = true } }
        }
    }
}

// MARK: 3 shader (a Metal layer effect over the photo)

struct ShaderBody: View {
    let row: Row
    let C: CGFloat
    var body: some View {
        let size = CGSize(width: C, height: (C * 9 / 16).rounded())
        VStack(alignment: .leading, spacing: 10) {
            Text(row.caption ?? "").font(.system(size: 15)).foregroundStyle(.black)
                .fixedSize(horizontal: false, vertical: true)
            if freeze {
                filtered(size: size, t: 1.25)
            } else {
                TimelineView(.animation) { ctx in filtered(size: size, t: motionTime(ctx.date)) }
            }
        }
    }

    private func filtered(size: CGSize, t: Double) -> some View {
        let d = 0.012 * size.width
        return BundleImage(name: row.photo!.src, size: size)
            .layerEffect(ShaderLibrary.xheavyWave(.float(Float(t)), .float2(Float(size.width), Float(size.height)),
                                                  .float3(rgb(row.duoA!)), .float3(rgb(row.duoB!))),
                         maxSampleOffset: CGSize(width: d, height: d))
            .clipShape(RoundedRectangle(cornerRadius: 12))
    }
}

extension Shader.Argument {
    static func float3(_ v: SIMD3<Float>) -> Shader.Argument { .float3(v.x, v.y, v.z) }
}

// MARK: 4 canvas

struct CanvasBody: View {
    let row: Row
    let C: CGFloat
    @Environment(\.displayScale) private var scale
    @State private var image: NSImage?

    var body: some View {
        Canvas { ctx, size in
            let W = size.width, H = size.height
            let band = Path(roundedRect: CGRect(x: 16, y: H - 60, width: 0.4 * W, height: 44), cornerRadius: 10)
            ctx.fill(band, with: .linearGradient(Gradient(colors: row.band!.map { Color(hexString: $0) }),
                                                 startPoint: CGPoint(x: 16, y: 0), endPoint: CGPoint(x: 16 + 0.4 * W, y: 0)))
            for s in row.strokes! {
                let p = s.p
                var path = Path()
                path.move(to: CGPoint(x: p[0] * W, y: p[1] * H))
                path.addCurve(to: CGPoint(x: p[6] * W, y: p[7] * H), control1: CGPoint(x: p[2] * W, y: p[3] * H),
                              control2: CGPoint(x: p[4] * W, y: p[5] * H))
                ctx.stroke(path, with: .color(Color(hexString: s.color)),
                           style: StrokeStyle(lineWidth: s.width, lineCap: .round, lineJoin: .round))
            }
            for d in row.dots! {
                ctx.fill(Path(ellipseIn: CGRect(x: d.x * W - d.r, y: d.y * H - d.r, width: d.r * 2, height: d.r * 2)),
                         with: .color(Color(hexString: d.color).opacity(0.6)))
            }
            if let image {
                let r = CGRect(x: W - 80, y: 16, width: 64, height: 64)
                ctx.drawLayer { l in
                    l.clip(to: Path(ellipseIn: r))
                    l.draw(Image(nsImage: image), in: r)
                }
            }
            ctx.draw(Text(row.title ?? "").font(.system(size: 20, weight: .semibold)).foregroundStyle(Color.ink),
                     at: CGPoint(x: 16, y: 16), anchor: .topLeading)
        }
        .frame(width: C, height: 220)
        .background(Color(hexString: row.bg!))
        .clipShape(RoundedRectangle(cornerRadius: 12))
        .task(id: row.id) {
            image = await ThumbnailCache.shared.load(row.image!, pixels: CGSize(width: 64 * scale, height: 64 * scale),
                                                     key: "\(row.image!)@\(Int(64 * scale))")
        }
    }
}

// MARK: 5 svg (asset-catalog SVGs, drawn as vectors)

struct SvgBody: View {
    let row: Row
    let C: CGFloat
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 16) {
                ForEach(row.icons!, id: \.self) { Image("icon-\($0)").resizable().frame(width: 28, height: 28) }
            }
            Image(row.chart!).resizable().frame(width: C, height: C * 160 / 600)
            Image(row.art!).resizable().frame(width: C, height: C / 2).clipShape(RoundedRectangle(cornerRadius: 12))
        }
    }
}

// MARK: 6 video

struct VideoBody: View {
    let row: Row
    let C: CGFloat
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(row.caption ?? "").font(.system(size: 15)).foregroundStyle(.black)
                .fixedSize(horizontal: false, vertical: true)
            LoopingVideo(name: row.video!)
                .frame(width: C, height: (C * 9 / 16).rounded())
                .clipShape(RoundedRectangle(cornerRadius: 12))
                .overlay(alignment: .bottomLeading) {
                    Text("0:04").font(.system(size: 12, weight: .semibold)).foregroundStyle(.white)
                        .padding(.vertical, 3).padding(.horizontal, 7)
                        .background(Color.black.opacity(0.55), in: RoundedRectangle(cornerRadius: 6))
                        .padding(8)
                }
        }
    }
}

// MARK: 7 map

struct MapBody: View {
    let row: Row
    let C: CGFloat
    var body: some View {
        let c = CLLocationCoordinate2D(latitude: row.lat!, longitude: row.lon!)
        VStack(alignment: .leading, spacing: 10) {
            VStack(alignment: .leading, spacing: 2) {
                Text(row.place ?? "").font(.system(size: 15, weight: .semibold)).foregroundStyle(.black)
                Text(row.address ?? "").font(.system(size: 13)).foregroundStyle(Color.secondaryGray)
            }
            Map(initialPosition: .region(MKCoordinateRegion(center: c, span: MKCoordinateSpan(latitudeDelta: 0.02, longitudeDelta: 0.02))),
                interactionModes: []) {
                Marker(row.place ?? "", coordinate: c)
            }
            .mapStyle(.standard)
            .frame(width: C, height: 200)
            .clipShape(RoundedRectangle(cornerRadius: 12))
        }
    }
}

// MARK: 8 markdown

let spaceMono14 = Font.custom("SpaceMono-Regular", fixedSize: 14)

func inlineMarkdown(_ s: String) -> AttributedString {
    var a = (try? AttributedString(markdown: s, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)))
        ?? AttributedString(s)
    for run in a.runs {
        if let intent = run.inlinePresentationIntent, intent.contains(.code) {
            a[run.range].font = spaceMono14
            a[run.range].backgroundColor = Color.xfill
        }
        if run.link != nil { a[run.range].foregroundColor = Color.xblue }
    }
    return a
}

struct MarkdownBody: View {
    let md: String
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(Array(md.components(separatedBy: "\n\n").enumerated()), id: \.offset) { _, block in
                if block.hasPrefix("## ") {
                    Text(inlineMarkdown(String(block.dropFirst(3)))).font(.system(size: 20, weight: .semibold))
                } else if block.hasPrefix("- ") {
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(Array(block.components(separatedBy: "\n").enumerated()), id: \.offset) { _, item in
                            HStack(alignment: .firstTextBaseline, spacing: 8) {
                                Text("•")
                                Text(inlineMarkdown(String(item.dropFirst(2))))
                            }
                            .font(.system(size: 16))
                        }
                    }
                } else if block.hasPrefix("> ") {
                    Text(inlineMarkdown(String(block.dropFirst(2)))).font(.system(size: 16).italic())
                        .foregroundStyle(Color.label2)
                        .padding(.leading, 10)
                        .overlay(alignment: .leading) { Rectangle().fill(Color(hex: 0xC7C7CC)).frame(width: 3) }
                } else {
                    Text(inlineMarkdown(block)).font(.system(size: 16))
                }
            }
        }
        .foregroundStyle(.black)
        .tint(Color.xblue)
        .fixedSize(horizontal: false, vertical: true)
    }
}

// MARK: 9 code

let tokenColors: [String: Color] = [
    "keyword": Color(hex: 0xFF7B72), "string": Color(hex: 0xA5D6FF), "number": Color(hex: 0x79C0FF),
    "comment": Color(hex: 0x8B949E), "function": Color(hex: 0xD2A8FF), "type": Color(hex: 0xFFA657),
    "plain": Color(hex: 0xE6EDF3),
]

struct CodeBody: View {
    let row: Row
    var body: some View {
        let mono13 = Font.custom("SpaceMono-Regular", fixedSize: 13)
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(row.file ?? "").font(.custom("SpaceMono-Regular", fixedSize: 12))
                Spacer()
                Text(["ts": "TS", "rs": "RUST", "py": "PYTHON"][row.lang ?? ""] ?? "").font(.system(size: 11, weight: .semibold))
            }
            .foregroundStyle(Color(hex: 0x8B949E))
            VStack(alignment: .leading, spacing: 0) {
                ForEach(Array(row.lines!.enumerated()), id: \.offset) { n, toks in
                    HStack(spacing: 12) {
                        Text("\(n + 1)").font(mono13).foregroundStyle(Color(hex: 0x6E7681)).frame(width: 24, alignment: .trailing)
                        Text(codeLine(toks)).font(mono13).lineLimit(1).fixedSize()
                    }
                    .frame(height: 20)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .clipped()
        }
        .padding(12)
        .background(Color(hex: 0x0D1117), in: RoundedRectangle(cornerRadius: 12))
    }

    private func codeLine(_ toks: [Tok]) -> AttributedString {
        var a = AttributedString()
        for t in toks {
            var run = AttributedString(t.t)
            run.foregroundColor = tokenColors[t.k] ?? tokenColors["plain"]
            a += run
        }
        return a
    }
}

// MARK: 10 intl

struct IntlBody: View {
    let row: Row
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(Array(row.blocks!.enumerated()), id: \.offset) { _, b in
                // Base direction comes from the text (first strong character); RTL paragraphs right-aligned.
                let rtl = b.dir == "rtl"
                Text(b.text).font(.system(size: 17)).foregroundStyle(.black)
                    .multilineTextAlignment(.leading)  // natural: an RTL paragraph aligns right
                    .frame(maxWidth: .infinity, alignment: rtl ? .trailing : .leading)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

// MARK: 11 typeface

struct TypefaceBody: View {
    let row: Row
    var body: some View {
        let f = feed.fonts[row.font ?? ""]
        VStack(alignment: .leading, spacing: 10) {
            Text(row.quote ?? "").font(.custom(f?.name ?? "", fixedSize: f?.size ?? 20)).foregroundStyle(Color.ink)
                .fixedSize(horizontal: false, vertical: true)
            Text("— \(row.by ?? "") · \(f?.family ?? "")").font(.system(size: 13)).foregroundStyle(Color.label2)
        }
        .padding(20)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color(hexString: row.bg!), in: RoundedRectangle(cornerRadius: 12))
    }
}

// MARK: 12 carousel

struct CarouselBody: View {
    let row: Row
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(row.title ?? "").font(.system(size: 17, weight: .semibold)).foregroundStyle(.black)
            ScrollView(.horizontal, showsIndicators: false) {
                LazyHStack(spacing: 10) {
                    ForEach(Array(row.cards!.enumerated()), id: \.offset) { _, card in
                        VStack(alignment: .leading, spacing: 0) {
                            BundleImage(name: card.image, size: CGSize(width: 140, height: 100))
                                .clipShape(RoundedRectangle(cornerRadius: 10))
                            Text(card.title).font(.system(size: 13, weight: .semibold)).foregroundStyle(.black)
                                .lineLimit(2).frame(width: 140, height: 34, alignment: .topLeading)
                                .padding(.top, 6)
                            Text(card.meta).font(.system(size: 12)).foregroundStyle(Color.secondaryGray)
                                .lineLimit(1).padding(.top, 4)
                        }
                        .frame(width: 140, height: 170, alignment: .topLeading)
                    }
                }
            }
            .frame(height: 170)
        }
    }
}

// MARK: 13 motion (GIF, WebP, Lottie)

struct MotionBody: View {
    let row: Row
    let C: CGFloat
    var body: some View {
        let T = (C - 24) / 3
        VStack(alignment: .leading, spacing: 10) {
            Text(row.caption ?? "").font(.system(size: 15)).foregroundStyle(.black)
                .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: 12) {
                tile(AnimatedImage(name: row.gif!), T, "GIF")
                tile(AnimatedImage(name: row.webp!), T, "WebP")
                tile(LottieTile(name: row.lottie!), T, "Lottie")
            }
        }
    }

    private func tile(_ v: some View, _ T: CGFloat, _ label: String) -> some View {
        VStack(spacing: 4) {
            v.frame(width: T, height: T).background(Color.xfill).clipShape(RoundedRectangle(cornerRadius: 12))
            Text(label).font(.system(size: 11, weight: .semibold)).foregroundStyle(Color.secondaryGray)
        }
    }
}

// MARK: 14 glass

struct GlassBody: View {
    let row: Row
    let C: CGFloat
    var body: some View {
        BundleImage(name: row.photo!.src, size: CGSize(width: C, height: (C * 3 / 4).rounded()))
            .clipShape(RoundedRectangle(cornerRadius: 16))
            .overlay(alignment: .bottom) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(row.title ?? "").font(.system(size: 16, weight: .semibold)).foregroundStyle(.black)
                    Text(row.subtitle ?? "").font(.system(size: 13)).foregroundStyle(Color.label2)
                }
                .lineLimit(1)
                .padding(.horizontal, 12)
                .frame(maxWidth: .infinity, minHeight: 64, maxHeight: 64, alignment: .leading)
                .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 14))
                .padding(12)
            }
            .overlay(alignment: .topTrailing) {
                Text(row.rating ?? "").font(.system(size: 13, weight: .semibold)).foregroundStyle(.black)
                    .padding(.vertical, 6).padding(.horizontal, 10)
                    .background(.ultraThinMaterial, in: Capsule())
                    .padding(12)
            }
    }
}

// MARK: 15 live

let ringColors = [Color(hex: 0xFF3B30), Color(hex: 0x34C759), Color(hex: 0x007AFF)]

func hms(_ v: Int) -> String { String(format: "%02d:%02d:%02d", v / 3600, v / 60 % 60, v % 60) }

struct LiveBody: View {
    let row: Row
    let s: Int
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text(row.title ?? "").font(.system(size: 15, weight: .semibold)).foregroundStyle(.black)
                Spacer()
                Text("Updated \((row.updatedSec! + s) % 60)s ago").font(.system(size: 12)).foregroundStyle(Color.secondaryGray)
            }
            HStack(alignment: .firstTextBaseline, spacing: 0) {
                Text("Ends in ").font(.system(size: 15)).foregroundStyle(Color.label2)
                Text(hms(max(0, row.endsInSec! - s))).font(.system(size: 22, weight: .semibold)).monospacedDigit()
                    .foregroundStyle(.black)
            }
            .padding(.top, 10)
            Group {
                if freeze { motion(t: nil) } else { TimelineView(.animation) { ctx in motion(t: motionTime(ctx.date)) } }
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Color.border, lineWidth: 0.5))
    }

    /// The rings and the waveform, at motion time t (nil = frozen).
    private func motion(t: Double?) -> some View {
        let q = t.map { $0.truncatingRemainder(dividingBy: 12) / 12 } ?? 0.4
        return VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 16) {
                ForEach(0..<3, id: \.self) { i in
                    let base = row.rings![i]
                    let p = t.map { min(1, max(0, base + 0.2 * sin(2 * .pi * ($0 / 4 + Double(i) / 3)))) } ?? base
                    ZStack {
                        Circle().inset(by: 3).stroke(Color.hairline, lineWidth: 6)
                        Circle().inset(by: 3).trim(from: 0, to: p)
                            .stroke(ringColors[i], style: StrokeStyle(lineWidth: 6, lineCap: .round))
                            .rotationEffect(.degrees(-90))
                        Text("\(Int((base * 100).rounded()))%").font(.system(size: 12, weight: .semibold))
                    }
                    .frame(width: 56, height: 56)
                }
            }
            HStack(spacing: 10) {
                ZStack {
                    Circle().fill(Color.xblue)
                    PlayTriangle().fill(.white).frame(width: 10, height: 12)
                }
                .frame(width: 32, height: 32)
                HStack(spacing: 2) {
                    ForEach(0..<48, id: \.self) { j in
                        RoundedRectangle(cornerRadius: 1.5)
                            .fill(Double(j) < 48 * q ? Color.xblue : Color(hex: 0xC7C7CC))
                            .frame(width: 3, height: row.wave![j])
                    }
                }
                .frame(height: 40)
                Text(String(format: "0:%02d / 0:12", s % 12)).font(.system(size: 12)).monospacedDigit()
                    .foregroundStyle(Color.secondaryGray)
            }
        }
        .padding(.top, 12)
    }
}

struct PlayTriangle: Shape {
    func path(in r: CGRect) -> Path {
        Path { p in
            p.move(to: CGPoint(x: r.minX, y: r.minY))
            p.addLine(to: CGPoint(x: r.maxX, y: r.midY))
            p.addLine(to: CGPoint(x: r.minX, y: r.maxY))
            p.closeSubpath()
        }
    }
}

// MARK: 16 thread (show more + comment box)

struct ThreadBody: View {
    let row: Row
    let s: Int
    @Bindable var state: RowState

    var body: some View {
        let expanded = state.expanded[row.id] ?? false
        let flip = (s / 3 + row.index) % 2 == 1
        VStack(alignment: .leading, spacing: 0) {
            Text(row.text ?? "").font(.system(size: 15)).foregroundStyle(.black)
                .lineLimit(expanded ? nil : 3)
                .fixedSize(horizontal: false, vertical: true)
            Button {
                withAnimation(.easeInOut(duration: 0.25)) { state.expanded[row.id] = !expanded }
            } label: {
                Text(expanded ? "Show less" : "Show more").font(.system(size: 15, weight: .semibold)).foregroundStyle(Color.xblue)
            }
            .buttonStyle(.borderless)
            .padding(.top, 4)
            let draft = state.drafts[row.id] ?? ""
            HStack(spacing: 8) {
                TextField("", text: Binding(get: { state.drafts[row.id] ?? "" }, set: { state.drafts[row.id] = $0 }),
                          prompt: Text("Add a comment…").foregroundStyle(Color.secondaryGray))
                    .font(.system(size: 15))
                    .textFieldStyle(.plain)  // macOS: iOS's default TextField has no bezel; AppKit's does
                    .padding(.horizontal, 14)
                    .frame(height: 40)
                    .background(Color.xfill, in: RoundedRectangle(cornerRadius: 20))
                Text("Send").font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(draft.isEmpty ? Color(hex: 0xC7C7CC) : Color.xblue)
            }
            .padding(.top, 12)
        }
        .onChange(of: flip) { _, now in
            guard liveMode else { return }
            withAnimation(.easeInOut(duration: 0.25)) { state.expanded[row.id] = now }
        }
        .onAppear { if liveMode { state.expanded[row.id] = flip } }
    }
}

// MARK: 17 webview

let embedTemplate: String = {
    guard let url = Bundle.main.url(forResource: "embed", withExtension: "html"),
          let s = try? String(contentsOf: url, encoding: .utf8) else { return "" }
    return s
}()

func embedHTML(_ row: Row) -> String {
    let bars = row.bars!.map { "<div class=\"bar\" style=\"height:\($0)%\"></div>" }.joined()
    return embedTemplate.replacingOccurrences(of: "{{HUE}}", with: "\(row.hue!)")
        .replacingOccurrences(of: "{{TITLE}}", with: row.title ?? "")
        .replacingOccurrences(of: "{{N}}", with: "\(row.index)")
        .replacingOccurrences(of: "{{PLAY}}", with: freeze ? "paused" : "running")
        .replacingOccurrences(of: "{{BARS}}", with: bars)
}

struct WebBody: View {
    let row: Row
    let C: CGFloat
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(row.caption ?? "").font(.system(size: 15)).foregroundStyle(.black)
                .fixedSize(horizontal: false, vertical: true)
            HTMLView(html: embedHTML(row))
                .frame(width: C, height: 220)
                .clipShape(RoundedRectangle(cornerRadius: 12))
                .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Color.border, lineWidth: 0.5))
        }
    }
}
