import CryptoKit
import Foundation

public struct ClaimMarkPoint: Codable, Equatable, Sendable {
    public let x: Double
    public let y: Double

    public init(x: Double, y: Double) {
        self.x = x
        self.y = y
    }
}

public enum ClaimMarkError: Error, Equatable, Sendable {
    case invalidCanvas
    case invalidPoint
    case tooShort
    case openStroke
    case doesNotEncloseArtifact
    case invalidEncoding
}

public struct ClaimMark: Equatable, Sendable {
    public enum Kind: UInt8, Equatable, Sendable {
        case drawn = 0
        case accessibilityHold = 1
    }

    public struct QuantizedPoint: Equatable, Sendable {
        public let x: UInt16
        public let y: UInt16

        public init(x: UInt16, y: UInt16) {
            self.x = x
            self.y = y
        }
    }

    private static let domain = Data("TOHSENO-CLAIM-MARK-V1\0".utf8)
    private static let pointCount = 64
    private static let minimumArcLength = 0.70
    private static let minimumEnclosureSpan = 0.24
    private static let minimumCenterMargin = 0.075
    private static let maximumEndpointDistance = 0.22

    public let kind: Kind
    public let quantizedPoints: [QuantizedPoint]

    public init(stroke: [ClaimMarkPoint], canvasWidth: Double, canvasHeight: Double) throws {
        guard canvasWidth.isFinite, canvasHeight.isFinite,
              canvasWidth > 0, canvasHeight > 0
        else { throw ClaimMarkError.invalidCanvas }
        guard stroke.count >= 4 else { throw ClaimMarkError.tooShort }

        var normalized: [ClaimMarkPoint] = []
        normalized.reserveCapacity(stroke.count)
        for point in stroke {
            guard point.x.isFinite, point.y.isFinite,
                  point.x >= 0, point.y >= 0,
                  point.x <= canvasWidth, point.y <= canvasHeight
            else { throw ClaimMarkError.invalidPoint }
            let next = ClaimMarkPoint(x: point.x / canvasWidth, y: point.y / canvasHeight)
            if normalized.last.map({ Self.distance($0, next) > Double.ulpOfOne }) ?? true {
                normalized.append(next)
            }
        }
        guard normalized.count >= 4 else { throw ClaimMarkError.tooShort }
        let total = Self.arcLength(normalized)
        guard total >= Self.minimumArcLength else { throw ClaimMarkError.tooShort }
        guard let first = normalized.first, let last = normalized.last,
              Self.distance(first, last) <= Self.maximumEndpointDistance
        else { throw ClaimMarkError.openStroke }
        guard Self.substantiallyEnclosesCenter(normalized)
        else { throw ClaimMarkError.doesNotEncloseArtifact }

        kind = .drawn
        quantizedPoints = Self.resampleAndQuantize(normalized, total: total)
    }

    private init(kind: Kind, quantizedPoints: [QuantizedPoint]) {
        self.kind = kind
        self.quantizedPoints = quantizedPoints
    }

    public static func accessibilityHold() -> ClaimMark {
        ClaimMark(kind: .accessibilityHold, quantizedPoints: accessibilityHoldPoints)
    }

    public init(canonicalBytes: Data) throws {
        let expectedCount = Self.domain.count + 3 + Self.pointCount * 4
        guard canonicalBytes.count == expectedCount,
              canonicalBytes.prefix(Self.domain.count) == Self.domain
        else { throw ClaimMarkError.invalidEncoding }
        var offset = Self.domain.count
        guard let kind = Kind(rawValue: canonicalBytes[offset])
        else { throw ClaimMarkError.invalidEncoding }
        offset += 1
        let count = UInt16(canonicalBytes[offset]) << 8 | UInt16(canonicalBytes[offset + 1])
        offset += 2
        guard count == Self.pointCount else { throw ClaimMarkError.invalidEncoding }
        var points: [QuantizedPoint] = []
        points.reserveCapacity(Self.pointCount)
        for _ in 0 ..< Self.pointCount {
            let x = UInt16(canonicalBytes[offset]) << 8 | UInt16(canonicalBytes[offset + 1])
            let y = UInt16(canonicalBytes[offset + 2]) << 8 | UInt16(canonicalBytes[offset + 3])
            points.append(QuantizedPoint(x: x, y: y))
            offset += 4
        }
        if kind == .accessibilityHold, points != Self.accessibilityHoldPoints {
            throw ClaimMarkError.invalidEncoding
        }
        self.kind = kind
        quantizedPoints = points
    }

    public var canonicalBytes: Data {
        var data = Self.domain
        data.append(kind.rawValue)
        Self.append(UInt16(Self.pointCount), to: &data)
        for point in quantizedPoints {
            Self.append(point.x, to: &data)
            Self.append(point.y, to: &data)
        }
        return data
    }

    public var gestureCommitment: Data {
        Data(SHA256.hash(data: canonicalBytes))
    }

    public var normalizedPoints: [ClaimMarkPoint] {
        quantizedPoints.map {
            ClaimMarkPoint(
                x: Double($0.x) / Double(UInt16.max),
                y: Double($0.y) / Double(UInt16.max)
            )
        }
    }

    private static func resampleAndQuantize(
        _ points: [ClaimMarkPoint],
        total: Double
    ) -> [QuantizedPoint] {
        var cumulative = [0.0]
        cumulative.reserveCapacity(points.count)
        for index in 1 ..< points.count {
            cumulative.append(cumulative[index - 1] + distance(points[index - 1], points[index]))
        }
        var result: [QuantizedPoint] = []
        result.reserveCapacity(pointCount)
        var segment = 0
        for index in 0 ..< pointCount {
            let target = total * Double(index) / Double(pointCount - 1)
            while segment + 1 < cumulative.count - 1, cumulative[segment + 1] < target {
                segment += 1
            }
            let start = points[segment]
            let end = points[segment + 1]
            let span = cumulative[segment + 1] - cumulative[segment]
            let fraction = span <= Double.ulpOfOne ? 0 : (target - cumulative[segment]) / span
            result.append(QuantizedPoint(
                x: quantize(start.x + (end.x - start.x) * fraction),
                y: quantize(start.y + (end.y - start.y) * fraction)
            ))
        }
        return result
    }

    private static func substantiallyEnclosesCenter(_ points: [ClaimMarkPoint]) -> Bool {
        let xs = points.map(\.x)
        let ys = points.map(\.y)
        guard let minX = xs.min(), let maxX = xs.max(),
              let minY = ys.min(), let maxY = ys.max(),
              maxX - minX >= minimumEnclosureSpan,
              maxY - minY >= minimumEnclosureSpan,
              minX <= 0.5 - minimumCenterMargin,
              maxX >= 0.5 + minimumCenterMargin,
              minY <= 0.5 - minimumCenterMargin,
              maxY >= 0.5 + minimumCenterMargin,
              var prior = points.last
        else { return false }

        var inside = false
        let center = ClaimMarkPoint(x: 0.5, y: 0.5)
        for point in points {
            let crosses = (point.y > center.y) != (prior.y > center.y)
                && center.x < (prior.x - point.x) * (center.y - point.y)
                / (prior.y - point.y) + point.x
            if crosses { inside.toggle() }
            prior = point
        }
        return inside
    }

    private static func arcLength(_ points: [ClaimMarkPoint]) -> Double {
        zip(points, points.dropFirst()).reduce(0) { $0 + distance($1.0, $1.1) }
    }

    private static func distance(_ a: ClaimMarkPoint, _ b: ClaimMarkPoint) -> Double {
        hypot(a.x - b.x, a.y - b.y)
    }

    private static func quantize(_ value: Double) -> UInt16 {
        UInt16((min(1, max(0, value)) * Double(UInt16.max) + 0.5).rounded(.down))
    }

    private static func append(_ value: UInt16, to data: inout Data) {
        data.append(UInt8(value >> 8))
        data.append(UInt8(value & 0xff))
    }

    // Fixed canonical geometry for the accessibility alternative. It is a
    // representation of a held ring, never fabricated hand geometry.
    private static let accessibilityHoldPoints: [QuantizedPoint] = [
        (57343,32768),(57222,30359),(56871,27974),(56287,25636),
        (55476,23368),(54444,21192),(53199,19130),(51754,17201),
        (50122,15426),(48320,13823),(46364,12411),(44272,11203),
        (42067,10211),(39769,9444),(37399,8908),(34981,8610),
        (32543,8549),(30110,8728),(27706,9144),(25354,9793),
        (23072,10670),(20882,11767),(18806,13073),(16864,14576),
        (15077,16261),(13462,18111),(12035,20108),(10812,22232),
        (9812,24462),(9042,26776),(8508,29151),(8213,31568),
        (8153,34005),(8333,36438),(8749,38842),(9398,41194),
        (10275,43476),(11372,45666),(12678,47742),(14181,49684),
        (15866,51471),(17716,53086),(19713,54513),(21837,55736),
        (24067,56736),(26381,57506),(28756,58040),(31173,58335),
        (33610,58395),(36043,58215),(38447,57799),(40799,57150),
        (43081,56273),(45271,55176),(47347,53870),(49289,52367),
        (51076,50682),(52691,48832),(54118,46835),(55341,44711),
        (56341,42481),(57111,40167),(57645,37792),(57343,32768),
    ].map { QuantizedPoint(x: $0.0, y: $0.1) }
}
