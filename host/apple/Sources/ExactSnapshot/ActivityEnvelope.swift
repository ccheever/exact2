// @ref LLP 1126.001 WP8 — one Codable contract for every app and state shape.
import Foundation
#if os(iOS)
import ActivityKit
#endif

public indirect enum ActivityJSON: Codable, Hashable, Sendable {
    case null, bool(Bool), number(Double), string(String)
    case array([ActivityJSON]), object([String: ActivityJSON])

    public init(from decoder: Decoder) throws {
        let value = try decoder.singleValueContainer()
        if value.decodeNil() { self = .null }
        else if let bool = try? value.decode(Bool.self) { self = .bool(bool) }
        else if let number = try? value.decode(Double.self), number.isFinite { self = .number(number) }
        else if let string = try? value.decode(String.self) { self = .string(string) }
        else if let array = try? value.decode([ActivityJSON].self) { self = .array(array) }
        else { self = .object(try value.decode([String: ActivityJSON].self)) }
    }

    public func encode(to encoder: Encoder) throws {
        var value = encoder.singleValueContainer()
        switch self {
        case .null: try value.encodeNil()
        case let .bool(v): try value.encode(v)
        case let .number(v): try value.encode(v)
        case let .string(v): try value.encode(v)
        case let .array(v): try value.encode(v)
        case let .object(v): try value.encode(v)
        }
    }
}

public struct ExactActivityAttributes: Codable, Hashable, Sendable {
    public struct ContentState: Codable, Hashable, Sendable {
        public var shape: String
        public var state: ActivityJSON
        public init(shape: String, state: ActivityJSON) { self.shape = shape; self.state = state }
    }

    public var kind: String
    public var generation: String
    public var shape: String
    public init(kind: String, generation: String, shape: String) {
        self.kind = kind; self.generation = generation; self.shape = shape
    }

    public struct Failure: Error, CustomStringConvertible {
        public let description: String
    }

    /// Called before a start, update or end send reaches ActivityKit.
    /// `isStale` remains ActivityKit's fact; a decode failure is the driver's outdated state.
    @discardableResult public func validate(_ content: ContentState) throws -> Int {
        func digest(_ value: String) -> Bool {
            value.utf8.count == 64 && value.utf8.allSatisfy { (48...57).contains($0) || (97...102).contains($0) }
        }
        guard digest(generation), digest(shape), content.shape == shape else {
            throw Failure(description: "activity generation or state shape digest does not match")
        }
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        let bytes = try encoder.encode(self).count + encoder.encode(content).count
        guard bytes <= 4096 else {
            throw Failure(description: "encoded attributes and state use \(bytes) bytes; the limit is 4096 bytes; shorten the state")
        }
        return bytes
    }
}

#if os(iOS)
extension ExactActivityAttributes: ActivityAttributes {}
#endif
