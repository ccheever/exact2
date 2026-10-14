import CExact
import CoreGraphics

/// A style owns its environment snapshot; resizing must not grow a global cache.
package final class CalculatedLength: Equatable {
    let handle: UInt32
    let environment: [Double]
    private let context: UnsafeMutableRawPointer

    init?(_ object: [String: BatchValue]) {
        guard let number = object["calc"]?.number, let handle = UInt32(exactly: number),
              let environment = object["env"]?.numbers,
              let context = environment.withUnsafeBufferPointer({ exact_calc_create(handle, $0.baseAddress, $0.count) }) else { return nil }
        self.handle = handle
        self.environment = environment
        self.context = context
    }
    /// Interpret the private length wire only in a style, never app payloads.
    static func decode(_ style: NodeStyle) -> NodeStyle {
        var resolved = style
        for (key, value) in style {
            guard case .object(let object) = value,
                  let calculation = CalculatedLength(object) else { continue }
            resolved[key] = .calculation(calculation)
        }
        return resolved
    }
    deinit { exact_calc_destroy(context) }
    func length(basis: CGFloat) -> CGFloat { CGFloat(exact_calc_resolve(context, Double(basis))) }
    package static func == (a: CalculatedLength, b: CalculatedLength) -> Bool {
        a === b || (a.handle == b.handle && a.environment == b.environment)
    }
    var value: BatchValue {
        .object(["calc": .number(Double(handle)), "env": .array(environment.map(BatchValue.number))])
    }
}
