import Foundation

public struct WorkshopDeviceID: RawRepresentable, Codable, Hashable, Sendable {
    public let rawValue: String

    public init?(rawValue: String) {
        guard rawValue.count <= 128,
              rawValue.range(of: #"^[A-Za-z0-9_-]+$"#, options: .regularExpression) != nil
        else { return nil }
        self.rawValue = rawValue
    }
}

public struct WorkshopSessionID: RawRepresentable, Codable, Hashable, Sendable {
    public let rawValue: String

    public init?(rawValue: String) {
        guard rawValue.hasPrefix("workshop_"), rawValue.count <= 128,
              rawValue.range(of: #"^[A-Za-z0-9_-]+$"#, options: .regularExpression) != nil
        else { return nil }
        self.rawValue = rawValue
    }
}

public enum WorkshopPlatform: String, Codable, CaseIterable, Sendable {
    case macOS = "macos"
    case iPhone = "iphone"
    case iPad = "ipad"
    case appleTV = "apple_tv"
    case visionOS = "visionos"
    case unknown
}

public enum WorkshopCapability: String, Codable, CaseIterable, Sendable {
    case display
    case touch
    case keyboard
    case camera
    case microphone
    case motion
    case location
    case filesystem
    case compute
    case audio
    case intelligence
}

public enum WorkshopAvailability: String, Codable, Sendable {
    case unknown
    case available
    case unavailable
}

public enum WorkshopPermission: String, Codable, Sendable {
    case notApplicable = "not_applicable"
    case notRequested = "not_requested"
    case authorized
    case denied
    case restricted
    case unknown
}

public enum WorkshopConnectionState: String, Codable, Sendable {
    case unavailable
    case discovering
    case authenticating
    case connected
    case reconnecting
    case rejected
}

/// Pure transition rules used by both products when a live transport ends.
/// Durable pairing is deliberately not represented here.
public enum WorkshopConnectionFlow {
    public static func afterTransportLoss(recoveryAvailable: Bool) -> WorkshopConnectionState {
        recoveryAvailable ? .reconnecting : .unavailable
    }
}

public struct WorkshopCapabilityState: Codable, Equatable, Sendable {
    public let capability: WorkshopCapability
    public let declared: Bool
    public let hardware: WorkshopAvailability
    public let permission: WorkshopPermission
    public let reachable: Bool
    public let authorized: Bool

    public init(
        capability: WorkshopCapability,
        declared: Bool = true,
        hardware: WorkshopAvailability,
        permission: WorkshopPermission = .notApplicable,
        reachable: Bool,
        authorized: Bool
    ) {
        self.capability = capability
        self.declared = declared
        self.hardware = hardware
        self.permission = permission
        self.reachable = reachable
        self.authorized = authorized
    }

    public var ready: Bool {
        declared
            && hardware == .available
            && reachable
            && authorized
            && (permission == .authorized || permission == .notApplicable)
    }

    public var explanation: String {
        guard declared else { return "Not declared by this device" }
        switch hardware {
        case .unknown: return "Hardware availability is unknown"
        case .unavailable: return "Hardware is unavailable"
        case .available: break
        }
        guard reachable else { return "Device is not currently reachable" }
        guard authorized else { return "Workshop access is not authorized" }
        switch permission {
        case .notRequested: return "Permission not requested"
        case .denied: return "Permission denied"
        case .restricted: return "Permission is restricted"
        case .unknown: return "Permission state is unknown"
        case .authorized, .notApplicable: return "Ready"
        }
    }
}

public struct WorkshopDevice: Codable, Equatable, Identifiable, Sendable {
    public let id: WorkshopDeviceID
    public let name: String
    public let platform: WorkshopPlatform
    public let connection: WorkshopConnectionState
    public let capabilities: [WorkshopCapabilityState]

    public init(
        id: WorkshopDeviceID,
        name: String,
        platform: WorkshopPlatform,
        connection: WorkshopConnectionState,
        capabilities: [WorkshopCapabilityState]
    ) {
        self.id = id
        self.name = name
        self.platform = platform
        self.connection = connection
        self.capabilities = capabilities
    }

    public func capability(_ capability: WorkshopCapability) -> WorkshopCapabilityState? {
        capabilities.first { $0.capability == capability }
    }
}

public struct WorkshopSurfaceDeclaration: Codable, Equatable, Sendable {
    public let role: String
    public let platform: WorkshopPlatform
    public let required: [WorkshopCapability]
    public let preferred: [WorkshopCapability]

    public init(
        role: String,
        platform: WorkshopPlatform,
        required: [WorkshopCapability],
        preferred: [WorkshopCapability] = []
    ) {
        self.role = role
        self.platform = platform
        self.required = required
        self.preferred = preferred
    }
}

public struct WorkshopSessionDeclaration: Codable, Equatable, Sendable {
    public let realtime: Bool

    public init(realtime: Bool) { self.realtime = realtime }
}

/// Optional source-visible metadata for Shots that intentionally use more than
/// their existing Apple target. A project with no declaration remains focused.
public struct WorkshopShotDeclaration: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.workshop-shot/1"

    public let schema: String
    public let surfaces: [WorkshopSurfaceDeclaration]
    public let session: WorkshopSessionDeclaration?

    public init(
        schema: String = schemaV1,
        surfaces: [WorkshopSurfaceDeclaration],
        session: WorkshopSessionDeclaration? = nil
    ) {
        self.schema = schema
        self.surfaces = surfaces
        self.session = session
    }
}

public enum WorkshopShotMode: String, Codable, Sendable {
    case focused
    case multisurface
}

public struct WorkshopSurfaceResolution: Codable, Equatable, Sendable {
    public let role: String
    public let deviceID: WorkshopDeviceID?
    public let missingRequiredCapabilities: [WorkshopCapability]
}

public struct WorkshopShotResolution: Codable, Equatable, Sendable {
    public let mode: WorkshopShotMode
    public let runnable: Bool
    public let surfaces: [WorkshopSurfaceResolution]

    public static let focused = WorkshopShotResolution(
        mode: .focused,
        runnable: true,
        surfaces: []
    )
}

public enum WorkshopResolver {
    public static func resolve(
        declaration: WorkshopShotDeclaration?,
        devices: [WorkshopDevice]
    ) -> WorkshopShotResolution {
        guard let declaration else { return .focused }
        guard declaration.schema == WorkshopShotDeclaration.schemaV1 else {
            return WorkshopShotResolution(mode: .multisurface, runnable: false, surfaces: [])
        }
        let resolutions = declaration.surfaces.map { surface in
            let candidates = devices.filter {
                $0.platform == surface.platform && $0.connection == .connected
            }
            let selected = candidates.first { device in
                surface.required.allSatisfy { device.capability($0)?.ready == true }
            }
            let missing = selected == nil ? surface.required.filter { capability in
                !candidates.contains { $0.capability(capability)?.ready == true }
            } : []
            return WorkshopSurfaceResolution(
                role: surface.role,
                deviceID: selected?.id,
                missingRequiredCapabilities: missing
            )
        }
        return WorkshopShotResolution(
            mode: .multisurface,
            runnable: !resolutions.isEmpty && resolutions.allSatisfy {
                $0.deviceID != nil && $0.missingRequiredCapabilities.isEmpty
            },
            surfaces: resolutions
        )
    }
}

public struct WorkshopEvent: Codable, Equatable, Sendable {
    public static let maximumPayloadBytes = 1024 * 1024
    private static let forbiddenPrefixes = [
        "authority.", "claim.", "ship.", "update.", "install.",
        "payment.", "publication.", "registry.", "device.revoke",
    ]

    public let type: String
    public let payload: Data
    public let timestamp: String?

    public init(type: String, payload: Data = Data(), timestamp: String? = nil) throws {
        guard type.count <= 128,
              type.range(of: #"^[a-z][a-z0-9_-]*(\.[a-z0-9_-]+)+$"#, options: .regularExpression) != nil,
              !Self.forbiddenPrefixes.contains(where: type.hasPrefix),
              payload.count <= Self.maximumPayloadBytes
        else { throw WorkshopRuntimeError.invalidEvent }
        self.type = type
        self.payload = payload
        self.timestamp = timestamp
    }

    /// The `workshop.*` namespace is reserved for transport-owned state such as
    /// capability snapshots and pulses. Shot-facing callers use app namespaces.
    public var isApplicationEvent: Bool {
        !type.hasPrefix("workshop.")
    }
}

public struct WorkshopEnvelope: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.workshop-envelope/1"

    public let schema: String
    public let sessionID: WorkshopSessionID
    public let senderDeviceID: WorkshopDeviceID
    public let sequence: UInt64
    public let event: WorkshopEvent

    public init(
        schema: String = schemaV1,
        sessionID: WorkshopSessionID,
        senderDeviceID: WorkshopDeviceID,
        sequence: UInt64,
        event: WorkshopEvent
    ) {
        self.schema = schema
        self.sessionID = sessionID
        self.senderDeviceID = senderDeviceID
        self.sequence = sequence
        self.event = event
    }

    public func validate(expectedSessionID: WorkshopSessionID? = nil) throws {
        guard schema == Self.schemaV1,
              sequence > 0,
              expectedSessionID == nil || sessionID == expectedSessionID
        else { throw WorkshopRuntimeError.unsupportedEnvelope }
        _ = try WorkshopEvent(type: event.type, payload: event.payload, timestamp: event.timestamp)
    }
}

public struct WorkshopPulseState: Equatable, Sendable {
    public let connection: WorkshopConnectionState
    public let peerName: String?
    public let lastEventAge: TimeInterval?
    public let lastRoundTrip: TimeInterval?

    public init(
        connection: WorkshopConnectionState,
        peerName: String? = nil,
        lastEventAge: TimeInterval? = nil,
        lastRoundTrip: TimeInterval? = nil
    ) {
        self.connection = connection
        self.peerName = peerName
        self.lastEventAge = lastEventAge
        self.lastRoundTrip = lastRoundTrip
    }

    public var headline: String {
        switch connection {
        case .connected: "Connected to \(peerName ?? "Workshop peer")"
        case .discovering: "Looking for the nearby Workshop"
        case .authenticating: "Checking the paired Workshop"
        case .reconnecting: "Workshop connection was interrupted; reconnecting"
        case .rejected: "This device is not authorized for the Workshop"
        case .unavailable: "Nearby Workshop unavailable"
        }
    }
}

public enum WorkshopRuntimeError: Error, LocalizedError, Equatable, Sendable {
    case invalidIdentity
    case invalidCredential
    case expiredCredential
    case unpairedDevice
    case revokedDevice
    case invalidEvent
    case unsupportedEnvelope
    case replayedEnvelope
    case transportUnavailable
    case disconnected

    public var errorDescription: String? {
        switch self {
        case .invalidIdentity: "Workshop device identity is invalid."
        case .invalidCredential: "The nearby Workshop could not be authenticated."
        case .expiredCredential: "The nearby Workshop credential expired."
        case .unpairedDevice: "This device is not paired with that Workshop."
        case .revokedDevice: "This device no longer has Workshop access."
        case .invalidEvent: "That event is not allowed on the ephemeral Session plane."
        case .unsupportedEnvelope: "The Workshop event version is unsupported."
        case .replayedEnvelope: "The Workshop event was repeated or arrived out of order."
        case .transportUnavailable: "The nearby Workshop transport is unavailable."
        case .disconnected: "The Workshop Session is disconnected."
        }
    }
}
