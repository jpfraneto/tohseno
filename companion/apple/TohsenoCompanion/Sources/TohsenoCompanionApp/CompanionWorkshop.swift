import Foundation
import SwiftUI
import TohsenoCompanionKit

enum PocketWorkshopMotion {
    static func activity(reduceMotion: Bool, active: Bool) -> Animation? {
        reduceMotion || !active
            ? nil
            : .easeInOut(duration: 1.1).repeatForever(autoreverses: true)
    }
}

public enum CompanionWorkshopChapter: String, Sendable {
    case takeShot = "take_shot"
    case waitingForMac = "waiting_for_mac"
    case building
    case readyForPhone = "ready_for_phone"
    case installing
    case installed
    case attention

    public var title: String {
        switch self {
        case .takeShot: "Take one clear Shot"
        case .waitingForMac: "Waiting for the Mac workshop"
        case .building: "The Mac factory is building"
        case .readyForPhone: "An app is ready for this iPhone"
        case .installing: "An app is arriving"
        case .installed: "Your workshop is connected"
        case .attention: "An app needs your attention"
        }
    }
}

public enum CompanionWorkshopThreshold: String, Sendable {
    case unknown
    case checked
    case unavailable
}

public struct CompanionWorkshopApp: Equatable, Identifiable, Sendable {
    public let id: String
    public let name: String
    public let state: TohsenoPresentedState
    public let headline: String

    public init(id: String, name: String, state: TohsenoPresentedState, headline: String) {
        self.id = id
        self.name = name
        self.state = state
        self.headline = headline
    }
}

/// The phone-side scene projection. It contains no factory state and makes no
/// inference about publication or installation beyond the facts already
/// delivered by the Mac and public clients.
public struct CompanionWorkshopProjection: Equatable, Sendable {
    public let chapter: CompanionWorkshopChapter
    public let apps: [CompanionWorkshopApp]
    public let macConnection: CompanionConnectionState
    public let keeperAvailable: Bool
    public let threshold: CompanionWorkshopThreshold
    public let unreadUpdates: Int

    public init(
        apps: [CompanionWorkshopApp],
        macConnection: CompanionConnectionState,
        keeperAvailable: Bool,
        publicEvidenceObserved: Bool?,
        unreadUpdates: Int
    ) {
        self.apps = apps
        self.macConnection = macConnection
        self.keeperAvailable = keeperAvailable
        threshold = switch publicEvidenceObserved {
        case true: .checked
        case false: .unavailable
        case nil: .unknown
        }
        self.unreadUpdates = unreadUpdates
        chapter = Self.chapter(apps: apps, connection: macConnection)
    }

    private static func chapter(
        apps: [CompanionWorkshopApp],
        connection: CompanionConnectionState
    ) -> CompanionWorkshopChapter {
        if apps.isEmpty { return .takeShot }
        if apps.contains(where: { $0.state == .failed }) { return .attention }
        if apps.contains(where: { $0.state == .installing }) { return .installing }
        if apps.contains(where: { $0.state == .readyForPhone }) { return .readyForPhone }
        if apps.contains(where: { $0.state == .building }) { return .building }
        if apps.contains(where: { $0.state == .waiting }) || connection != .connected {
            return .waitingForMac
        }
        return .installed
    }
}
