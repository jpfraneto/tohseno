import Foundation

public struct CompanionReferenceDescriptor: Codable, Equatable, Sendable {
    public let blobID: String
    public let originName: String
    public let mediaType: String
    public let byteLength: UInt64
    public let sha256: String

    enum CodingKeys: String, CodingKey {
        case blobID = "blob_id"
        case originName = "origin_name"
        case mediaType = "media_type"
        case byteLength = "byte_length"
        case sha256
    }

    public init(blobID: String, originName: String, mediaType: String, byteLength: UInt64, sha256: String) {
        self.blobID = blobID
        self.originName = originName
        self.mediaType = mediaType
        self.byteLength = byteLength
        self.sha256 = sha256
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, ["blob_id", "origin_name", "media_type", "byte_length", "sha256"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        blobID = try container.decode(String.self, forKey: .blobID)
        originName = try container.decode(String.self, forKey: .originName)
        mediaType = try container.decode(String.self, forKey: .mediaType)
        byteLength = try container.decode(UInt64.self, forKey: .byteLength)
        sha256 = try container.decode(String.self, forKey: .sha256)
    }

    func validate() throws {
        try requireIdentifier(blobID, field: "reference.blob_id")
        try requireBoundedText(originName, field: "reference.origin_name", maximum: 512)
        guard !originName.contains("/"), !originName.contains("\\") else {
            throw TohsenoCompanionError.invalidEncoding("reference origin must not be a path")
        }
        guard mediaType == "image/png" || mediaType == "image/jpeg" else {
            throw TohsenoCompanionError.invalidEncoding("reference media type is invalid")
        }
        guard (1 ... UInt64(CompanionReferenceBlob.maximumByteLength)).contains(byteLength) else {
            throw TohsenoCompanionError.invalidEncoding("reference size is invalid")
        }
        _ = try Base64URL.decode(sha256, expectedBytes: 32)
    }

    func canonicalValue() -> CanonicalValue {
        .object([
            "blob_id": .string(blobID), "byte_length": .unsigned(byteLength),
            "media_type": .string(mediaType), "origin_name": .string(originName),
            "sha256": .string(sha256),
        ])
    }
}

public enum CompanionCommandPayload: Codable, Equatable, Sendable {
    case workspaceSnapshotRequest
    case feedbackSubmit(
        shotID: String,
        expressionID: String,
        versionID: String,
        versionOrdinal: UInt64,
        body: String
    )
    case marketingSubmit(noteID: String, shotID: String, body: String)
    case shotEvolveRequest(
        shotID: String,
        baseExpressionID: String,
        baseVersionID: String,
        baseVersionOrdinal: UInt64,
        intention: String,
        selectedFeedbackActionCommitments: [String],
        references: [CompanionReferenceDescriptor]
    )
    case shotCreateRequest(
        suggestedName: String?,
        intention: String,
        references: [CompanionReferenceDescriptor]
    )

    public var requiredCapability: CompanionCapability {
        switch self {
        case .workspaceSnapshotRequest: .workspaceRead
        case .feedbackSubmit: .feedbackWrite
        case .marketingSubmit: .marketingWrite
        case .shotEvolveRequest: .shotEvolve
        case .shotCreateRequest: .shotCreate
        }
    }

    private enum Keys: String, CodingKey {
        case commandKind = "command_kind"
        case shotID = "shot_id"
        case expressionID = "expression_id"
        case versionID = "version_id"
        case versionOrdinal = "version_ordinal"
        case body
        case noteID = "note_id"
        case baseExpressionID = "base_expression_id"
        case baseVersionID = "base_version_id"
        case baseVersionOrdinal = "base_version_ordinal"
        case intention
        case selectedFeedbackActionCommitments = "selected_feedback_action_commitments"
        case references
        case suggestedName = "suggested_name"
    }

    private enum Kind: String, Codable {
        case feedback = "feedback.submit"
        case marketing = "marketing.submit"
        case evolve = "shot.evolve.request"
        case create = "shot.create.request"
        case workspaceSnapshot = "workspace.snapshot.request"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        switch try container.decode(Kind.self, forKey: .commandKind) {
        case .workspaceSnapshot:
            try requireExactKeys(decoder, ["command_kind"])
            self = .workspaceSnapshotRequest
        case .feedback:
            try requireExactKeys(decoder, [
                "command_kind", "shot_id", "expression_id", "version_id", "version_ordinal", "body",
            ])
            self = try .feedbackSubmit(
                shotID: container.decode(String.self, forKey: .shotID),
                expressionID: container.decode(String.self, forKey: .expressionID),
                versionID: container.decode(String.self, forKey: .versionID),
                versionOrdinal: container.decode(UInt64.self, forKey: .versionOrdinal),
                body: container.decode(String.self, forKey: .body)
            )
        case .marketing:
            try requireExactKeys(decoder, ["command_kind", "note_id", "shot_id", "body"])
            self = try .marketingSubmit(
                noteID: container.decode(String.self, forKey: .noteID),
                shotID: container.decode(String.self, forKey: .shotID),
                body: container.decode(String.self, forKey: .body)
            )
        case .evolve:
            try requireExactKeys(decoder, [
                "command_kind", "shot_id", "base_expression_id", "base_version_id",
                "base_version_ordinal", "intention", "selected_feedback_action_commitments", "references",
            ])
            self = try .shotEvolveRequest(
                shotID: container.decode(String.self, forKey: .shotID),
                baseExpressionID: container.decode(String.self, forKey: .baseExpressionID),
                baseVersionID: container.decode(String.self, forKey: .baseVersionID),
                baseVersionOrdinal: container.decode(UInt64.self, forKey: .baseVersionOrdinal),
                intention: container.decode(String.self, forKey: .intention),
                selectedFeedbackActionCommitments: container.decode(
                    [String].self,
                    forKey: .selectedFeedbackActionCommitments
                ),
                references: container.decode([CompanionReferenceDescriptor].self, forKey: .references)
            )
        case .create:
            var expected: Set<String> = ["command_kind", "intention", "references"]
            if container.contains(.suggestedName) { expected.insert("suggested_name") }
            try requireExactKeys(decoder, expected)
            self = try .shotCreateRequest(
                suggestedName: container.decodeIfPresent(String.self, forKey: .suggestedName),
                intention: container.decode(String.self, forKey: .intention),
                references: container.decode([CompanionReferenceDescriptor].self, forKey: .references)
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: Keys.self)
        switch self {
        case .workspaceSnapshotRequest:
            try container.encode(Kind.workspaceSnapshot, forKey: .commandKind)
        case let .feedbackSubmit(shotID, expressionID, versionID, versionOrdinal, body):
            try container.encode(Kind.feedback, forKey: .commandKind)
            try container.encode(shotID, forKey: .shotID)
            try container.encode(expressionID, forKey: .expressionID)
            try container.encode(versionID, forKey: .versionID)
            try container.encode(versionOrdinal, forKey: .versionOrdinal)
            try container.encode(body, forKey: .body)
        case let .marketingSubmit(noteID, shotID, body):
            try container.encode(Kind.marketing, forKey: .commandKind)
            try container.encode(noteID, forKey: .noteID)
            try container.encode(shotID, forKey: .shotID)
            try container.encode(body, forKey: .body)
        case let .shotEvolveRequest(
            shotID, expressionID, versionID, ordinal, intention, commitments, references
        ):
            try container.encode(Kind.evolve, forKey: .commandKind)
            try container.encode(shotID, forKey: .shotID)
            try container.encode(expressionID, forKey: .baseExpressionID)
            try container.encode(versionID, forKey: .baseVersionID)
            try container.encode(ordinal, forKey: .baseVersionOrdinal)
            try container.encode(intention, forKey: .intention)
            try container.encode(commitments, forKey: .selectedFeedbackActionCommitments)
            try container.encode(references, forKey: .references)
        case let .shotCreateRequest(suggestedName, intention, references):
            try container.encode(Kind.create, forKey: .commandKind)
            try container.encodeIfPresent(suggestedName, forKey: .suggestedName)
            try container.encode(intention, forKey: .intention)
            try container.encode(references, forKey: .references)
        }
    }

    func validate() throws {
        switch self {
        case .workspaceSnapshotRequest:
            break
        case let .feedbackSubmit(shotID, expressionID, versionID, ordinal, body):
            try requireIdentifier(shotID, field: "shot_id")
            try requireIdentifier(expressionID, field: "expression_id")
            try requireIdentifier(versionID, field: "version_id")
            guard ordinal > 0 else { throw TohsenoCompanionError.invalidEncoding("Version ordinal") }
            try requireBoundedText(body, field: "feedback.body", maximum: 256 * 1024)
        case let .marketingSubmit(noteID, shotID, body):
            try requireIdentifier(noteID, field: "note_id")
            try requireIdentifier(shotID, field: "shot_id")
            try requireBoundedText(body, field: "marketing.body", maximum: 256 * 1024)
        case let .shotEvolveRequest(
            shotID, expressionID, versionID, ordinal, intention, commitments, references
        ):
            try requireIdentifier(shotID, field: "shot_id")
            try requireIdentifier(expressionID, field: "base_expression_id")
            try requireIdentifier(versionID, field: "base_version_id")
            guard ordinal > 0, commitments.count <= 256 else {
                throw TohsenoCompanionError.invalidEncoding("invalid evolution base or commitments")
            }
            try requireBoundedText(intention, field: "intention", maximum: 1024 * 1024)
            for commitment in commitments { _ = try Base64URL.decode(commitment, expectedBytes: 32) }
            try Self.validateReferences(references)
        case let .shotCreateRequest(suggestedName, intention, references):
            if let suggestedName {
                try requireBoundedText(suggestedName, field: "suggested_name", maximum: 256)
            }
            try requireBoundedText(intention, field: "intention", maximum: 1024 * 1024)
            try Self.validateReferences(references)
        }
    }

    private static func validateReferences(_ references: [CompanionReferenceDescriptor]) throws {
        guard references.count <= 8, Set(references.map(\.blobID)).count == references.count else {
            throw TohsenoCompanionError.invalidEncoding("references exceed bounds or repeat")
        }
        for reference in references { try reference.validate() }
    }

    func canonicalValue() -> CanonicalValue {
        switch self {
        case .workspaceSnapshotRequest:
            return .object(["command_kind": .string("workspace.snapshot.request")])
        case let .feedbackSubmit(shotID, expressionID, versionID, ordinal, body):
            return .object([
                "body": .string(body), "command_kind": .string("feedback.submit"),
                "expression_id": .string(expressionID), "shot_id": .string(shotID),
                "version_id": .string(versionID), "version_ordinal": .unsigned(ordinal),
            ])
        case let .marketingSubmit(noteID, shotID, body):
            return .object([
                "body": .string(body), "command_kind": .string("marketing.submit"),
                "note_id": .string(noteID), "shot_id": .string(shotID),
            ])
        case let .shotEvolveRequest(
            shotID, expressionID, versionID, ordinal, intention, commitments, references
        ):
            return .object([
                "base_expression_id": .string(expressionID),
                "base_version_id": .string(versionID), "base_version_ordinal": .unsigned(ordinal),
                "command_kind": .string("shot.evolve.request"), "intention": .string(intention),
                "references": .array(references.map { $0.canonicalValue() }),
                "selected_feedback_action_commitments": .array(commitments.map { .string($0) }),
                "shot_id": .string(shotID),
            ])
        case let .shotCreateRequest(suggestedName, intention, references):
            var value: [String: CanonicalValue] = [
                "command_kind": .string("shot.create.request"), "intention": .string(intention),
                "references": .array(references.map { $0.canonicalValue() }),
            ]
            if let suggestedName { value["suggested_name"] = .string(suggestedName) }
            return .object(value)
        }
    }
}

public struct CompanionCommand: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.companion-command/1"
    static let signatureDomain = "tohseno.companion.command-signature.v1"

    public let schema: String
    public let commandID: String
    public let workspaceID: String
    public let capabilityID: String
    public let authorDeviceID: String
    public let createdAt: String
    public let payload: CompanionCommandPayload
    public let signature: String

    enum CodingKeys: String, CodingKey {
        case schema
        case commandID = "command_id"
        case workspaceID = "workspace_id"
        case capabilityID = "capability_id"
        case authorDeviceID = "author_device_id"
        case createdAt = "created_at"
        case payload, signature
    }

    init(
        commandID: String,
        workspaceID: String,
        capabilityID: String,
        authorDeviceID: String,
        createdAt: String,
        payload: CompanionCommandPayload,
        signature: String
    ) {
        schema = Self.schemaV1
        self.commandID = commandID
        self.workspaceID = workspaceID
        self.capabilityID = capabilityID
        self.authorDeviceID = authorDeviceID
        self.createdAt = createdAt
        self.payload = payload
        self.signature = signature
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, [
            "schema", "command_id", "workspace_id", "capability_id", "author_device_id",
            "created_at", "payload", "signature",
        ])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        commandID = try container.decode(String.self, forKey: .commandID)
        workspaceID = try container.decode(String.self, forKey: .workspaceID)
        capabilityID = try container.decode(String.self, forKey: .capabilityID)
        authorDeviceID = try container.decode(String.self, forKey: .authorDeviceID)
        createdAt = try container.decode(String.self, forKey: .createdAt)
        payload = try container.decode(CompanionCommandPayload.self, forKey: .payload)
        signature = try container.decode(String.self, forKey: .signature)
    }

    static func sign(
        identity: CompanionIdentity,
        commandID: String,
        workspaceID: String,
        capabilityID: String,
        createdAt: String,
        payload: CompanionCommandPayload
    ) throws -> Self {
        let draft = CompanionCommand(
            commandID: commandID,
            workspaceID: workspaceID,
            capabilityID: capabilityID,
            authorDeviceID: identity.description.deviceID,
            createdAt: createdAt,
            payload: payload,
            signature: "pending"
        )
        try draft.validateShape()
        return CompanionCommand(
            commandID: draft.commandID,
            workspaceID: draft.workspaceID,
            capabilityID: draft.capabilityID,
            authorDeviceID: draft.authorDeviceID,
            createdAt: draft.createdAt,
            payload: draft.payload,
            signature: Base64URL.encode(try identity.sign(
                domain: Self.signatureDomain,
                message: draft.canonicalBody()
            ))
        )
    }

    public func verify(expectedSigningPublicKey: Data, expectedDeviceID: String, now: Date = Date()) throws {
        try validateShape()
        guard authorDeviceID == expectedDeviceID else {
            throw TohsenoCompanionError.invalidEncoding("command author differs")
        }
        let created = try CompanionTimestamp.parse(createdAt)
        guard now >= created.addingTimeInterval(-30), now <= created.addingTimeInterval(30 * 24 * 60 * 60) else {
            throw TohsenoCompanionError.invalidEncoding("command is outside its admission window")
        }
        guard try CompanionIdentity.verify(
            publicKey: expectedSigningPublicKey,
            domain: Self.signatureDomain,
            message: canonicalBody(),
            signature: Base64URL.decode(signature, expectedBytes: 64)
        ) else { throw TohsenoCompanionError.invalidEncoding("command signature failed") }
    }

    public func payloadDigest() throws -> Data { try canonicalBody().companionSHA256 }

    func validateShape() throws {
        guard schema == Self.schemaV1 else { throw TohsenoCompanionError.invalidEncoding("command schema") }
        try requireIdentifier(commandID, field: "command_id")
        try requireIdentifier(workspaceID, field: "workspace_id")
        try requireIdentifier(capabilityID, field: "capability_id")
        try requireIdentifier(authorDeviceID, field: "author_device_id")
        _ = try CompanionTimestamp.parse(createdAt)
        try payload.validate()
    }

    func canonicalBody() throws -> Data {
        try CanonicalValue.object([
            "author_device_id": .string(authorDeviceID), "capability_id": .string(capabilityID),
            "command_id": .string(commandID), "created_at": .string(createdAt),
            "payload": payload.canonicalValue(), "schema": .string(schema),
            "workspace_id": .string(workspaceID),
        ]).data()
    }
}

public enum CommandReceiptState: String, Codable, Sendable {
    case received, accepted, completed, rejected, failed
}

public struct CommandReceipt: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.companion-command-receipt/1"
    public let schema: String
    public let commandID: String
    public let state: CommandReceiptState
    public let shotID: String?
    public let executionID: String?
    public let resultID: String?
    public let rejectionCode: String?

    enum CodingKeys: String, CodingKey {
        case schema
        case commandID = "command_id"
        case state
        case shotID = "shot_id"
        case executionID = "execution_id"
        case resultID = "result_id"
        case rejectionCode = "rejection_code"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        var expected: Set<String> = ["schema", "command_id", "state"]
        for (key, name) in [
            (CodingKeys.shotID, "shot_id"), (CodingKeys.executionID, "execution_id"),
            (CodingKeys.resultID, "result_id"), (CodingKeys.rejectionCode, "rejection_code"),
        ] where container.contains(key) { expected.insert(name) }
        try requireExactKeys(decoder, expected)
        schema = try container.decode(String.self, forKey: .schema)
        commandID = try container.decode(String.self, forKey: .commandID)
        state = try container.decode(CommandReceiptState.self, forKey: .state)
        shotID = try container.decodeIfPresent(String.self, forKey: .shotID)
        executionID = try container.decodeIfPresent(String.self, forKey: .executionID)
        resultID = try container.decodeIfPresent(String.self, forKey: .resultID)
        rejectionCode = try container.decodeIfPresent(String.self, forKey: .rejectionCode)
    }

    public init(
        schema: String = schemaV1,
        commandID: String,
        state: CommandReceiptState,
        shotID: String? = nil,
        executionID: String? = nil,
        resultID: String? = nil,
        rejectionCode: String? = nil
    ) {
        self.schema = schema
        self.commandID = commandID
        self.state = state
        self.shotID = shotID
        self.executionID = executionID
        self.resultID = resultID
        self.rejectionCode = rejectionCode
    }

    public func validate() throws {
        guard schema == Self.schemaV1 else { throw TohsenoCompanionError.invalidEncoding("receipt schema") }
        try requireIdentifier(commandID, field: "command_id")
        for (label, value) in [
            ("shot_id", shotID), ("execution_id", executionID),
            ("result_id", resultID), ("rejection_code", rejectionCode),
        ] where value != nil { try requireIdentifier(value!, field: label) }
        guard state == .rejected || rejectionCode == nil else {
            throw TohsenoCompanionError.invalidEncoding("non-rejection receipt has a rejection code")
        }
    }
}
