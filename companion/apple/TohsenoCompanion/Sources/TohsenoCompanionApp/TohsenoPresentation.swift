import Foundation
import TohsenoCompanionKit

/// The phone's half of the one presentation projection.
///
/// The Mac publishes the same six states from
/// `application/src/presentation.rs`; the frozen companion snapshot schema does
/// not carry them, so the phone derives them from the same execution states.
/// `Resources/presentation-v1.json` is the shared table both sides assert
/// against, so the Mac and the phone can never describe one app differently.
///
/// The copy differs on purpose: the Mac speaks to the person about their
/// iPhone, and the iPhone speaks about itself.
public enum TohsenoPresentedState: String, Sendable, CaseIterable {
    case waiting
    case building
    case readyForPhone = "ready_for_phone"
    case installing
    case installed
    case failed

    public static func from(_ execution: ExecutionStatus) -> Self {
        switch execution {
        case .queued: .waiting
        case .planning, .conception, .materializing, .building, .testing, .verifying, .repairing:
            .building
        case .waitingForDevice: .readyForPhone
        case .installing, .launching: .installing
        case .accepted: .installed
        case .failed: .failed
        }
    }

    /// True while TOHSENO is doing work the person should simply wait through.
    public var inFlight: Bool {
        self == .waiting || self == .building || self == .installing
    }
}

/// What one app looks like on the phone.
public struct TohsenoPresentation: Equatable, Sendable {
    public let state: TohsenoPresentedState
    public let headline: String
    public let detail: String?
    public let isWorking: Bool

    public init(state: TohsenoPresentedState, headline: String, detail: String? = nil, isWorking: Bool? = nil) {
        self.state = state
        self.headline = headline
        self.detail = detail
        self.isWorking = isWorking ?? state.inFlight
    }

    /// Derive the presentation for one app in the synchronized snapshot.
    public static func of(_ shot: ShotSummary) -> Self {
        if shot.execution == nil, shot.kind != .factoryShot {
            return Self(
                state: .waiting,
                headline: "Source on Mac · installation unconfirmed",
                detail: "This app is in your workshop. This snapshot does not confirm a device build or installation on this iPhone.",
                isWorking: false
            )
        }
        let state = shot.execution.map { TohsenoPresentedState.from($0.state) }
            ?? (shot.latestVersionID != nil ? .installed : .waiting)
        return forState(state, appName: shot.displayName)
    }

    public static func forState(_ state: TohsenoPresentedState, appName: String) -> Self {
        switch state {
        case .waiting:
            Self(state: state, headline: "Waiting to build on Mac", detail: "Your Mac has not reported a completed build yet.")
        case .building:
            Self(state: state, headline: "Building \(appName)…", detail: "Your Mac is preparing the app. Installation follows when the paired iPhone is reachable.")
        case .readyForPhone:
            Self(
                state: state,
                headline: "Built on Mac · ready to install",
                detail: "The build is saved. Keep this iPhone unlocked and reachable by your Mac over USB or Xcode Wi-Fi; installation resumes automatically."
            )
        case .installing:
            Self(state: state, headline: "Installing on iPhone…", detail: "Keep the paired iPhone connected and unlocked until installation finishes.")
        case .installed:
            Self(state: state, headline: "Installed on iPhone · last confirmed", detail: "Your Mac confirmed this version was installed. Open it from the iPhone’s Home Screen or App Library. Removing it later is not reflected automatically here.")
        case .failed:
            Self(state: state, headline: "Needs attention on Mac", detail: "The latest attempt did not finish. Open this app on your Mac to see what stopped and continue. An older installed version may still be on your iPhone.")
        }
    }

    /// Nothing has reached the Mac yet, so nothing can be claimed about it.
    public static func waitingForMac(appName: String) -> Self {
        Self(
            state: .waiting,
            headline: "Waiting for your Mac…",
            detail: "Saved on this iPhone. Keep Companion open and online until your Mac accepts it; after that, the Mac works independently."
        )
    }
}

/// The shared Rust/Swift table, read from the checked-in fixture.
enum PresentationFixture {
    static func executionStates() throws -> [String: String] {
        guard let url = Bundle.module.url(forResource: "presentation-v1", withExtension: "json"),
              let data = try? Data(contentsOf: url),
              let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              object["schema"] as? String == "tohseno.presentation-projection/1",
              let states = object["execution_states"] as? [String: String]
        else {
            throw TohsenoCompanionError.invalidEncoding("presentation projection fixture")
        }
        return states
    }
}
