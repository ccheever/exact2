#if os(iOS)
import UIKit

/// Draw bounded reader history without adding a frame source.
final class FrameHealthChart: UIView {
    private(set) var points: [FrameHealthTrend.Point] = []
    init() {
        super.init(frame: .zero)
        isOpaque = true
        backgroundColor = .secondarySystemBackground
        isUserInteractionEnabled = false
        isAccessibilityElement = true
        accessibilityLabel = "Callback cadence chart"
        accessibilityIdentifier = "exact.host.frame-health-chart"
        contentMode = .redraw
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    func update(_ health: FrameHealth) {
        points = health.window.trend.points
        if let average = points.last?.average {
            accessibilityValue = String(format: "Average %.1f callbacks per second. Reference 120 hertz. Blue shows the average; orange shows a conservative rate for the longest gap. History covers 30 seconds.", average)
        } else { accessibilityValue = "No comparable callback intervals are available. Reference 120 hertz." }
        setNeedsDisplay()
    }
    override func draw(_ rect: CGRect) {
        guard let context = UIGraphicsGetCurrentContext() else { return }
        (backgroundColor ?? .secondarySystemBackground).setFill()
        context.fill(bounds)
        let font = UIFontMetrics(forTextStyle: .caption2).scaledFont(for: .monospacedDigitSystemFont(ofSize: 11, weight: .regular))
        let label = "120 Hz" as NSString
        let labelWidth = label.size(withAttributes: [.font: font]).width
        let plot = bounds.inset(by: UIEdgeInsets(top: font.lineHeight / 2 + 3, left: 1, bottom: 3, right: labelWidth + 7))
        guard plot.width > 0, plot.height > 0 else { return }
        let maximum = max(144, ceil((points.compactMap(\.average).max() ?? 120) / 60) * 60)
        func y(_ value: Double) -> CGFloat { plot.maxY - CGFloat(min(max(value, 0), maximum) / maximum) * plot.height }
        let reference = y(120)
        context.setStrokeColor(UIColor.tertiaryLabel.cgColor)
        context.setLineWidth(1 / traitCollection.displayScale)
        context.setLineDash(phase: 0, lengths: [4, 3])
        context.move(to: CGPoint(x: plot.minX, y: reference))
        context.addLine(to: CGPoint(x: plot.maxX, y: reference))
        context.strokePath()
        label.draw(at: CGPoint(x: plot.maxX + 6, y: reference - font.lineHeight / 2),
            withAttributes: [.font: font, .foregroundColor: UIColor.secondaryLabel])
        context.setLineDash(phase: 0, lengths: [])
        guard let latest = points.last?.time else { return }
        func drawLine(_ value: (FrameHealthTrend.Point) -> Double?, color: UIColor, width: CGFloat) {
            let path = UIBezierPath()
            var joined = false
            for point in points {
                guard let rate = value(point), latest >= point.time, latest - point.time <= 30_000_000_000 else { joined = false; continue }
                let x = plot.maxX - CGFloat(Double(latest - point.time) / 30e9) * plot.width
                let position = CGPoint(x: x, y: y(rate))
                if joined { path.addLine(to: position) } else { path.move(to: position) }
                joined = true
            }
            color.setStroke()
            path.lineWidth = width
            path.lineJoinStyle = .round
            path.stroke()
        }
        drawLine({ $0.slowest }, color: .systemOrange, width: 1)
        drawLine({ $0.average }, color: .systemBlue, width: 2)
    }
}
#endif
