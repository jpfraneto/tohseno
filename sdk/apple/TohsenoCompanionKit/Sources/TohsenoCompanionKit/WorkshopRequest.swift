import Foundation

/// Owner-private request history, encrypted with the existing Companion state.
/// This is a projection of signed commands and authenticated Mac reports, not
/// a new command path or evidence that a queued request has run.
public struct WorkshopRequest: Codable, Equatable, Sendable, Identifiable {
    public let commandID: String
    public let intention: String
    public let title: String
    public let createdAt: String
    public private(set) var shotID: String?
    public private(set) var receipt: CommandReceipt?
    public private(set) var execution: ExecutionSummary?
    public private(set) var activity: [WorkshopRequestActivity]
    public var id: String { commandID }
    public var awaitingMac: Bool { receipt == nil }
    public var isFinished: Bool {
        execution?.state.isTerminal == true || receipt?.state == .rejected || receipt?.state == .failed
    }

    init?(commandID: String, payload: CompanionCommandPayload, createdAt: String) {
        self.commandID = commandID
        self.createdAt = createdAt
        switch payload {
        case let .shotCreateRequest(name, intention, _):
            self.title = name ?? "New Shot"
            self.intention = intention
            self.shotID = nil
        case let .shotEvolveRequest(shotID, _, _, _, intention, _, _),
             let .projectEvolveRequest(shotID, _, intention, _, _):
            self.title = "App evolution"
            self.intention = intention
            self.shotID = shotID
        default: return nil
        }
        activity = [.init(id: "queued", occurredAt: createdAt, message: "Saved on this iPhone. Waiting for your Mac to accept it.")]
    }

    mutating func apply(_ receipt: CommandReceipt, at timestamp: String) {
        guard receipt.commandID == commandID else { return }
        self.receipt = receipt
        shotID = receipt.shotID ?? shotID
        let message: String = switch receipt.state {
        case .received: "Your Mac received the request."
        case .accepted: "Your Mac accepted the request. Waiting for a work report."
        case .completed: "Your Mac completed the command. Check the app’s build and delivery report."
        case .rejected: "Your Mac declined the request: \(receipt.rejectionCode ?? "unknown_reason")."
        case .failed: "Your Mac could not start this request. Check the workshop on your Mac."
        }
        append(.init(id: "receipt_\(receipt.state.rawValue)", occurredAt: timestamp, message: message))
    }

    mutating func apply(_ execution: ExecutionSummary) {
        // An old execution on the same app must never complete a new request.
        guard let executionID = receipt?.executionID, executionID == execution.executionID else { return }
        if let previous = self.execution {
            guard execution.updatedAt >= previous.updatedAt,
                  !previous.state.isTerminal || execution.state.isTerminal else { return }
        }
        self.execution = execution
        append(.init(id: "\(execution.executionID)_\(execution.state.rawValue)_\(execution.updatedAt)",
                     occurredAt: execution.updatedAt, message: Self.message(for: execution)))
    }

    private mutating func append(_ entry: WorkshopRequestActivity) {
        guard !activity.contains(where: { $0.id == entry.id }) else { return }
        activity.append(entry)
        activity = Array(activity.suffix(80))
    }

    public static func message(for execution: ExecutionSummary) -> String {
        switch execution.state {
        case .queued: "Queued in your Mac workshop."
        case .planning, .conception: "Your Mac is preparing the app work."
        case .materializing: "The coding harness is working on your Mac."
        case .building: "Your Mac is building the app with Xcode."
        case .testing: "Your Mac is running app checks."
        case .verifying: "Your Mac is verifying the build."
        case .repairing: "The coding harness is repairing the build."
        case .waitingForDevice: "The build is waiting for your intended iPhone."
        case .installing: "Your Mac is installing on your intended iPhone."
        case .launching: "Your Mac is opening the app on your iPhone."
        case .accepted: "App work completed. See the app’s delivery report for installation status."
        case .failed: "App work stopped: \(execution.failureCode ?? "execution_failed"). Check the workshop on your Mac."
        }
    }
}

public struct WorkshopRequestActivity: Codable, Equatable, Sendable, Identifiable {
    public let id: String
    public let occurredAt: String
    public let message: String
}

extension CompanionPersistentState {
    mutating func rememberRequest(commandID: String, payload: CompanionCommandPayload, createdAt: String) {
        guard !workshopRequests.contains(where: { $0.commandID == commandID }),
              let request = WorkshopRequest(commandID: commandID, payload: payload, createdAt: createdAt) else { return }
        workshopRequests.append(request)
        // Keep the latest 200 requests, always preserving unacknowledged outbox work.
        while workshopRequests.count > 200,
              let index = workshopRequests.firstIndex(where: { $0.isFinished })
                ?? workshopRequests.firstIndex(where: { !$0.awaitingMac }) {
            workshopRequests.remove(at: index)
        }
    }

    mutating func updateRequests(from event: WorkspaceEvent) {
        switch event.payload {
        case let .commandAcknowledged(receipt), let .commandRejected(receipt):
            for index in workshopRequests.indices {
                workshopRequests[index].apply(receipt, at: event.emittedAt)
            }
        case let .executionQueued(execution), let .executionStarted(execution),
             let .executionUpdated(execution), let .executionWaitingForDevice(execution),
             let .executionCompleted(execution), let .executionFailed(execution):
            for index in workshopRequests.indices { workshopRequests[index].apply(execution) }
        default: break
        }
        let snapshot: WorkspaceSnapshot?
        if case let .workspaceSnapshot(value) = event.payload { snapshot = value }
        else { snapshot = workspace }
        for execution in (snapshot?.shots.compactMap(\.execution) ?? []) + (snapshot?.activeExecutions ?? []) {
            for index in workshopRequests.indices { workshopRequests[index].apply(execution) }
        }
    }
}
