import Foundation

public enum FactoryClientError: Error, LocalizedError, Equatable, Sendable {
    case invalidConfiguration(String)
    case invalidResponse(String)
    case rejected(code: String, message: String)
    case transport(String)

    public var errorDescription: String? {
        switch self {
        case let .invalidConfiguration(message), let .invalidResponse(message), let .transport(message):
            message
        case let .rejected(_, message):
            message
        }
    }
}

public protocol FactoryServing: Sendable {
    func workspace() async throws -> WorkspaceSnapshot
    func factoryDefaults() async throws -> FactoryDefaults
    func readiness() async throws -> ReadinessView
    func managedStatus() async throws -> ManagedStatus
    func managedBalance() async throws -> ManagedBalance
    func managedCatalog() async throws -> ManagedCatalog
    func managedEstimate(model: String, privacy: String, intentionBytes: UInt64, referenceBytes: UInt64, appID: String?) async throws -> ManagedEstimate
    func managedCheckout(packID: String) async throws -> ManagedCheckout
    func registrySnapshot(appNames: [String]) async throws -> RegistrySnapshot
    func performReadinessAction(_ action: String) async throws -> ReadinessView
    func adoptProject(path: String, scheme: String?) async throws -> ProjectAdoptionResult
    func pairedCompanionDevices() async throws -> [PairedCompanionDevice]
    func createCompanionPairingSession() async throws -> CompanionPairingSession
    func companionPairingSession(id: String) async throws -> CompanionPairingSession
    func renameCompanionDevice(id: String, displayName: String) async throws -> PairedCompanionDevice
    func revokeCompanionDevice(id: String) async throws -> PairedCompanionDevice
    func create(_ draft: CreationDraft, commandID: String) async throws -> CommandReceipt
    func evolve(_ app: AppSummary, draft: EvolutionDraft, commandID: String) async throws -> CommandReceipt
    func receipt(for appID: String) async throws -> ExecutionReceipt?
    func activity(for appID: String) async throws -> ExecutionActivity?
    func icon(for appID: String) async throws -> Data?
    func preview(for appID: String) async throws -> Data?
    func openOnPhone(for appID: String) async throws
    func openSource(for appID: String) async throws
    func retire(appID: String) async throws
    func restore(appID: String) async throws
    func restartService() async throws
    func openLegacyStudio() async throws
    func configureCustomHarness(_ draft: CustomHarnessDraft) async throws
    func configureLocalEndpoint(_ draft: LocalEndpointDraft) async throws
    func events() async -> AsyncThrowingStream<Void, Error>
}

public actor LoopbackFactoryClient: FactoryServing {
    private let urlSession: URLSession
    private let helperOverride: URL?
    private var nativeSession: NativeSessionCredential?
    private var nativeSessionTask: Task<NativeSessionCredential, Error>?
    private var nativeSessionGeneration: UInt64 = 0

    public init(helperOverride: URL? = nil, urlSession: URLSession? = nil) {
        self.helperOverride = helperOverride
        if let urlSession {
            self.urlSession = urlSession
        } else {
            let configuration = URLSessionConfiguration.ephemeral
            configuration.httpCookieAcceptPolicy = .never
            configuration.httpShouldSetCookies = false
            configuration.requestCachePolicy = .reloadIgnoringLocalAndRemoteCacheData
            configuration.timeoutIntervalForRequest = 15
            configuration.timeoutIntervalForResource = 60
            self.urlSession = URLSession(configuration: configuration)
        }
    }

    public func workspace() async throws -> WorkspaceSnapshot {
        try await request("/api/v1/workspace")
    }

    public func factoryDefaults() async throws -> FactoryDefaults {
        try await request("/api/v1/factory-defaults")
    }

    public func readiness() async throws -> ReadinessView {
        let genesis: CableGenesisResponse = try await request("/api/v1/genesis")
        guard genesis.schema == "tohseno.cable-genesis-view/1" else {
            throw FactoryClientError.invalidResponse("The local Companion setup response is invalid.")
        }
        return ReadinessView(genesis: genesis)
    }

    public func managedStatus() async throws -> ManagedStatus {
        try await request("/api/v1/managed/status")
    }

    public func managedBalance() async throws -> ManagedBalance {
        try await request("/api/v1/managed/balance")
    }

    public func managedCatalog() async throws -> ManagedCatalog {
        try await request("/api/v1/managed/catalog")
    }

    public func managedEstimate(
        model: String,
        privacy: String,
        intentionBytes: UInt64,
        referenceBytes: UInt64,
        appID: String?
    ) async throws -> ManagedEstimate {
        try await request(
            "/api/v1/managed/estimate",
            method: "POST",
            body: ManagedEstimateBody(
                model: model,
                privacy: privacy,
                intentionBytes: intentionBytes,
                referenceBytes: referenceBytes,
                sourceContextBytes: 0,
                shotID: appID
            )
        )
    }

    public func managedCheckout(packID: String) async throws -> ManagedCheckout {
        try validateToken(packID, label: "balance pack")
        return try await request(
            "/api/v1/managed/checkout",
            method: "POST",
            body: ManagedCheckoutBody(packID: packID)
        )
    }

    public func registrySnapshot(appNames: [String]) async throws -> RegistrySnapshot {
        guard appNames.count <= 1_000 else {
            throw FactoryClientError.invalidConfiguration("The local Registry app list is too large.")
        }
        let builder: BuilderIdentityView = try await helperJSON([
            "--json", "advanced", "identity", "show",
        ])
        let network: RegistryNetworkStatus = try await helperJSON([
            "--json", "advanced", "network", "status",
        ])
        var records: [LocalRegistryRecord] = []
        records.reserveCapacity(appNames.count)
        for name in appNames.sorted() {
            try validateToken(name, label: "app name")
            let record: LocalRegistryRecord = try await helperJSON([
                "--json", "advanced", "registry", "show", "--", name,
            ])
            records.append(record)
        }
        return RegistrySnapshot(builder: builder, network: network, records: records)
    }

    public func performReadinessAction(_ action: String) async throws -> ReadinessView {
        try validateToken(action, label: "readiness action")
        let genesis: CableGenesisResponse = try await request(
            "/api/v1/genesis/actions/\(action)", method: "POST", body: EmptyBody()
        )
        guard genesis.schema == "tohseno.cable-genesis-view/1" else {
            throw FactoryClientError.invalidResponse("The local Companion setup response is invalid.")
        }
        return ReadinessView(genesis: genesis)
    }

    public func adoptProject(
        path: String,
        scheme: String? = nil
    ) async throws -> ProjectAdoptionResult {
        guard path.hasPrefix("/"), !path.contains("\0") else {
            throw FactoryClientError.invalidConfiguration(
                "Choose a local Xcode project or workspace."
            )
        }
        return try await request(
            "/api/v1/projects/adopt",
            method: "POST",
            body: AdoptProjectBody(path: path, scheme: scheme, harness: nil, model: nil)
        )
    }

    public func pairedCompanionDevices() async throws -> [PairedCompanionDevice] {
        let response: CompanionDeviceListResponse = try await request("/api/v1/companion/devices")
        return response.devices
    }

    public func createCompanionPairingSession() async throws -> CompanionPairingSession {
        try await request(
            "/api/v1/companion/pairing-sessions", method: "POST", body: EmptyBody()
        )
    }

    public func companionPairingSession(id: String) async throws -> CompanionPairingSession {
        try await request("/api/v1/companion/pairing-sessions/\(try pathToken(id))")
    }

    public func renameCompanionDevice(
        id: String,
        displayName: String
    ) async throws -> PairedCompanionDevice {
        try await request(
            "/api/v1/companion/devices/\(try pathToken(id))",
            method: "POST",
            body: RenameCompanionDeviceBody(displayName: displayName)
        )
    }

    public func revokeCompanionDevice(id: String) async throws -> PairedCompanionDevice {
        try await request(
            "/api/v1/companion/devices/\(try pathToken(id))",
            method: "DELETE",
            body: EmptyBody()
        )
    }

    public func create(_ draft: CreationDraft, commandID: String) async throws -> CommandReceipt {
        let managed = try managedBody(
            harness: draft.harness,
            model: draft.model,
            privacy: draft.managedPrivacy,
            maximum: draft.managedMaximumMicrousd,
            consent: draft.managedConsent
        )
        let body = CreateBody(
            commandID: commandID,
            name: draft.name.trimmingCharacters(in: .whitespacesAndNewlines).nilIfEmpty,
            harness: managed == nil ? draft.harness : nil,
            model: managed == nil ? draft.model : nil,
            managed: managed,
            intention: draft.intention,
            references: draft.references.map(APIReference.init)
        )
        return try await request("/api/v1/shots", method: "POST", body: body)
    }

    public func evolve(
        _ app: AppSummary,
        draft: EvolutionDraft,
        commandID: String
    ) async throws -> CommandReceipt {
        if let sourceState = app.sourceState {
            let body = ProjectEvolveBody(
                commandID: commandID,
                baseSourceState: sourceState,
                intention: draft.intention,
                references: draft.references.map(APIReference.init)
            )
            return try await request(
                "/api/v1/projects/\(try pathToken(app.shotID))/evolutions",
                method: "POST",
                body: body
            )
        }
        guard let expressionID = app.expressionID,
              let versionID = app.latestVersionID,
              let ordinal = app.latestVersionOrdinal else {
            throw FactoryClientError.invalidConfiguration("This app has no accepted base to evolve.")
        }
        let managed = try managedBody(
            harness: draft.harness,
            model: draft.model,
            privacy: draft.managedPrivacy,
            maximum: draft.managedMaximumMicrousd,
            consent: draft.managedConsent
        )
        let body = EvolveBody(
            commandID: commandID,
            baseExpressionID: expressionID,
            baseVersionID: versionID,
            baseVersionOrdinal: ordinal,
            intention: draft.intention,
            harness: managed == nil ? draft.harness : nil,
            model: managed == nil ? draft.model : nil,
            managed: managed,
            references: draft.references.map(APIReference.init)
        )
        return try await request(
            "/api/v1/shots/\(try pathToken(app.shotID))/evolutions",
            method: "POST",
            body: body
        )
    }

    public func receipt(for appID: String) async throws -> ExecutionReceipt? {
        do {
            return try await request("/api/v1/shots/\(try pathToken(appID))/receipt")
        } catch FactoryClientError.rejected(code: "not_found", message: _) {
            return nil
        }
    }

    public func activity(for appID: String) async throws -> ExecutionActivity? {
        do {
            let resource = appID.hasPrefix("project_") ? "projects" : "shots"
            return try await request("/api/v1/\(resource)/\(try pathToken(appID))/activity")
        } catch FactoryClientError.rejected(code: "not_found", message: _) {
            return nil
        }
    }

    public func icon(for appID: String) async throws -> Data? {
        try await asset("/api/v1/shots/\(try pathToken(appID))/icon", maximum: 16 * 1024 * 1024)
    }

    public func preview(for appID: String) async throws -> Data? {
        try await asset("/api/v1/shots/\(try pathToken(appID))/preview", maximum: 32 * 1024 * 1024)
    }

    public func openOnPhone(for appID: String) async throws {
        let resource = appID.hasPrefix("project_") ? "projects" : "shots"
        let _: EmptyResponse = try await request(
            "/api/v1/\(resource)/\(try pathToken(appID))/open-on-iphone",
            method: "POST",
            body: EmptyBody()
        )
    }

    public func openSource(for appID: String) async throws {
        let resource = appID.hasPrefix("project_") ? "projects" : "shots"
        let _: EmptyResponse = try await request(
            "/api/v1/\(resource)/\(try pathToken(appID))/open-source",
            method: "POST",
            body: EmptyBody()
        )
    }

    public func retire(appID: String) async throws {
        let _: RetireResponse = try await request(
            "/api/v1/shots/\(try pathToken(appID))",
            method: "DELETE",
            body: EmptyBody()
        )
    }

    public func restore(appID: String) async throws {
        let _: RestoreResponse = try await request(
            "/api/v1/shots/\(try pathToken(appID))/restore",
            method: "POST",
            body: EmptyBody()
        )
    }

    public func restartService() async throws {
        invalidateNativeSession()
        try await runHelper(["service", "restart"])
    }

    public func openLegacyStudio() async throws {
        try await runHelper(["studio"])
    }

    public func configureCustomHarness(_ draft: CustomHarnessDraft) async throws {
        let arguments = try parseArguments(draft.arguments)
        let models = try parseModels(draft.models)
        let body = CustomHarnessBody(
            id: try configuredID(draft.id),
            label: try configuredLabel(draft.label),
            executable: draft.executable,
            arguments: arguments,
            models: models,
            preferred: draft.preferred
        )
        let _: ConfigurationReceipt = try await request(
            "/api/v1/intelligence/custom-harnesses", method: "POST", body: body
        )
        try await restartService()
    }

    public func configureLocalEndpoint(_ draft: LocalEndpointDraft) async throws {
        let id = try configuredID(draft.id)
        let reference = draft.credential.isEmpty ? nil : "local-model-\(id)"
        if let reference {
            guard let secret = draft.credential.data(using: .utf8), secret.count <= 16 * 1024 else {
                throw FactoryClientError.invalidConfiguration("The local endpoint credential is invalid.")
            }
            try await runHelper(
                ["local-model-credential", "--reference", reference],
                standardInput: secret
            )
        }
        let body = LocalEndpointBody(
            id: id,
            label: try configuredLabel(draft.label),
            baseURL: draft.baseURL,
            models: try parseModels(draft.models),
            credentialReference: reference,
            consentToSendSource: draft.consentToSendSource,
            privacyMode: draft.privacyMode,
            preferred: draft.preferred
        )
        let _: ConfigurationReceipt = try await request(
            "/api/v1/intelligence/local-endpoints", method: "POST", body: body
        )
        try await restartService()
    }

    public func events() async -> AsyncThrowingStream<Void, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let credential = try await self.credential()
                    guard let url = URL(string: "/api/v1/events", relativeTo: URL(string: credential.origin)) else {
                        throw FactoryClientError.invalidConfiguration("The local event URL is invalid.")
                    }
                    var request = URLRequest(url: url)
                    request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
                    request.setValue(
                        "\(credential.tokenType) \(credential.token)",
                        forHTTPHeaderField: "Authorization"
                    )
                    let (bytes, response) = try await self.urlSession.bytes(for: request)
                    guard let http = response as? HTTPURLResponse else {
                        throw FactoryClientError.invalidResponse("The local event stream was refused.")
                    }
                    if http.statusCode == 403 {
                        self.invalidateNativeSession(rejected: credential)
                    }
                    guard http.statusCode == 200 else {
                        throw FactoryClientError.invalidResponse("The local event stream was refused.")
                    }
                    for try await line in bytes.lines where line == "event: workspace.changed" || line == "event: workspace.reconcile" {
                        continuation.yield(())
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }

    private func request<Response: Decodable, Body: Encodable & Sendable>(
        _ path: String,
        method: String,
        body: Body,
        retrySession: Bool = true
    ) async throws -> Response {
        let credential = try await credential()
        guard let base = URL(string: credential.origin),
              let url = URL(string: path, relativeTo: base),
              url.host == "127.0.0.1",
              url.scheme == "http" else {
            throw FactoryClientError.invalidConfiguration("The local factory origin is invalid.")
        }
        var urlRequest = URLRequest(url: url)
        urlRequest.httpMethod = method
        urlRequest.cachePolicy = .reloadIgnoringLocalAndRemoteCacheData
        urlRequest.setValue("application/json", forHTTPHeaderField: "Accept")
        urlRequest.setValue(
            "\(credential.tokenType) \(credential.token)",
            forHTTPHeaderField: "Authorization"
        )
        urlRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")
        urlRequest.httpBody = try JSONEncoder.tohseno.encode(body)
        do {
            let (data, response) = try await urlSession.data(for: urlRequest)
            guard let http = response as? HTTPURLResponse else {
                throw FactoryClientError.invalidResponse("The local factory returned no HTTP response.")
            }
            guard data.count <= 4 * 1024 * 1024 else {
                throw FactoryClientError.invalidResponse("The local factory response is oversized.")
            }
            if (200..<300).contains(http.statusCode) {
                if Response.self == EmptyResponse.self && data.isEmpty {
                    return EmptyResponse() as! Response
                }
                do { return try JSONDecoder.tohseno.decode(Response.self, from: data) }
                catch {
                    throw FactoryClientError.invalidResponse("The local factory returned an invalid response.")
                }
            }
            let api = try? JSONDecoder.tohseno.decode(APIErrorBody.self, from: data)
            if http.statusCode == 403, api?.code == "native_session_rejected", retrySession {
                invalidateNativeSession(rejected: credential)
                return try await request(path, method: method, body: body, retrySession: false)
            }
            throw FactoryClientError.rejected(
                code: api?.code ?? "http_\(http.statusCode)",
                message: api?.message ?? "The local factory refused this request."
            )
        } catch let error as FactoryClientError {
            throw error
        } catch {
            throw FactoryClientError.transport("The local factory could not be reached: \(error.localizedDescription)")
        }
    }

    private func request<Response: Decodable>(_ path: String) async throws -> Response {
        try await get(path, retrySession: true)
    }

    private func get<Response: Decodable>(_ path: String, retrySession: Bool) async throws -> Response {
        let credential = try await credential()
        guard let base = URL(string: credential.origin),
              let url = URL(string: path, relativeTo: base),
              url.host == "127.0.0.1",
              url.scheme == "http" else {
            throw FactoryClientError.invalidConfiguration("The local factory origin is invalid.")
        }
        var urlRequest = URLRequest(url: url)
        urlRequest.cachePolicy = .reloadIgnoringLocalAndRemoteCacheData
        urlRequest.setValue("application/json", forHTTPHeaderField: "Accept")
        urlRequest.setValue(
            "\(credential.tokenType) \(credential.token)",
            forHTTPHeaderField: "Authorization"
        )
        do {
            let (data, response) = try await urlSession.data(for: urlRequest)
            guard let http = response as? HTTPURLResponse else {
                throw FactoryClientError.invalidResponse("The local factory returned no HTTP response.")
            }
            guard data.count <= 4 * 1024 * 1024 else {
                throw FactoryClientError.invalidResponse("The local factory response is oversized.")
            }
            if (200..<300).contains(http.statusCode) {
                do { return try JSONDecoder.tohseno.decode(Response.self, from: data) }
                catch {
                    throw FactoryClientError.invalidResponse("The local factory returned an invalid response.")
                }
            }
            let api = try? JSONDecoder.tohseno.decode(APIErrorBody.self, from: data)
            if http.statusCode == 403, api?.code == "native_session_rejected", retrySession {
                invalidateNativeSession(rejected: credential)
                return try await get(path, retrySession: false)
            }
            throw FactoryClientError.rejected(
                code: api?.code ?? "http_\(http.statusCode)",
                message: api?.message ?? "The local factory refused this request."
            )
        } catch let error as FactoryClientError {
            throw error
        } catch {
            throw FactoryClientError.transport("The local factory could not be reached: \(error.localizedDescription)")
        }
    }

    private func asset(_ path: String, maximum: Int, retrySession: Bool = true) async throws -> Data? {
        let credential = try await credential()
        guard let base = URL(string: credential.origin),
              let url = URL(string: path, relativeTo: base),
              url.host == "127.0.0.1", url.scheme == "http" else {
            throw FactoryClientError.invalidConfiguration("The local factory asset URL is invalid.")
        }
        var request = URLRequest(url: url)
        request.cachePolicy = .reloadIgnoringLocalAndRemoteCacheData
        request.setValue("image/png,image/jpeg", forHTTPHeaderField: "Accept")
        request.setValue("\(credential.tokenType) \(credential.token)", forHTTPHeaderField: "Authorization")
        let (data, response) = try await urlSession.data(for: request)
        guard let http = response as? HTTPURLResponse else {
            throw FactoryClientError.invalidResponse("The local factory returned no asset response.")
        }
        if http.statusCode == 404 { return nil }
        if http.statusCode == 403, retrySession {
            invalidateNativeSession(rejected: credential)
            return try await asset(path, maximum: maximum, retrySession: false)
        }
        guard http.statusCode == 200, !data.isEmpty, data.count <= maximum else {
            throw FactoryClientError.invalidResponse("The local factory returned an invalid app image.")
        }
        return data
    }

    private func credential() async throws -> NativeSessionCredential {
        if let nativeSession,
           let expiry = ISO8601DateFormatter().date(from: nativeSession.expiresAt),
           expiry.timeIntervalSinceNow > 10 {
            return nativeSession
        }

        let generation = nativeSessionGeneration
        let task: Task<NativeSessionCredential, Error>
        if let nativeSessionTask {
            task = nativeSessionTask
        } else {
            let pending = Task { try await Self.launchHelper(helperOverride) }
            nativeSessionTask = pending
            task = pending
        }

        do {
            let credential = try await task.value
            guard generation == nativeSessionGeneration else {
                return try await self.credential()
            }
            guard credential.schema == "tohseno.native-session/1",
                  credential.clientID == "com.tohseno.mac",
                  credential.tokenType == "TohsenoNative",
                  credential.token.count == 43,
                  credential.scopes.contains("factory.read"),
                  credential.scopes.contains("factory.mutate") else {
                throw FactoryClientError.invalidResponse("The native session helper returned an invalid credential.")
            }
            nativeSession = credential
            nativeSessionTask = nil
            return credential
        } catch {
            if generation == nativeSessionGeneration {
                nativeSessionTask = nil
            }
            throw error
        }
    }

    private func invalidateNativeSession(rejected credential: NativeSessionCredential? = nil) {
        if let credential, nativeSession?.token != credential.token {
            return
        }
        nativeSession = nil
        nativeSessionGeneration &+= 1
        nativeSessionTask?.cancel()
        nativeSessionTask = nil
    }

    private static func launchHelper(_ override: URL?) async throws -> NativeSessionCredential {
        try await Task.detached(priority: .userInitiated) {
            let helper = try helperURL(override)
            let process = Process()
            process.executableURL = helper
            process.arguments = ["native-session"]
            process.standardInput = FileHandle.nullDevice
            let output = Pipe()
            let errors = Pipe()
            process.standardOutput = output
            process.standardError = errors
            do { try process.run() }
            catch {
                throw FactoryClientError.invalidConfiguration("The bundled local factory helper could not start.")
            }
            process.waitUntilExit()
            let data = output.fileHandleForReading.readDataToEndOfFile()
            let errorData = errors.fileHandleForReading.readDataToEndOfFile()
            guard process.terminationStatus == 0 else {
                let detail = String(data: errorData.prefix(2_048), encoding: .utf8)?
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                throw FactoryClientError.transport(detail?.nilIfEmpty ?? "The native client could not authenticate.")
            }
            guard !data.isEmpty, data.count <= 64 * 1024 else {
                throw FactoryClientError.invalidResponse("The native session helper response is invalid.")
            }
            do { return try JSONDecoder.tohseno.decode(NativeSessionCredential.self, from: data) }
            catch {
                throw FactoryClientError.invalidResponse("The native session helper response is invalid.")
            }
        }.value
    }

    private func runHelper(_ arguments: [String], standardInput: Data? = nil) async throws {
        let helperOverride = self.helperOverride
        try await Task.detached(priority: .userInitiated) {
            let process = Process()
            process.executableURL = try Self.helperURL(helperOverride)
            process.arguments = arguments
            let input = standardInput.map { _ in Pipe() }
            process.standardInput = input ?? FileHandle.nullDevice
            process.standardOutput = FileHandle.nullDevice
            let errors = Pipe()
            process.standardError = errors
            try process.run()
            if let input, let standardInput {
                input.fileHandleForWriting.write(standardInput)
                try input.fileHandleForWriting.close()
            }
            process.waitUntilExit()
            guard process.terminationStatus == 0 else {
                let detail = String(data: errors.fileHandleForReading.readDataToEndOfFile().prefix(2_048), encoding: .utf8)?
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                throw FactoryClientError.transport(detail?.nilIfEmpty ?? "The local factory action did not complete.")
            }
        }.value
    }

    private func helperJSON<Response: Decodable & Sendable>(
        _ arguments: [String]
    ) async throws -> Response {
        let helperOverride = self.helperOverride
        return try await Task.detached(priority: .userInitiated) {
            let process = Process()
            process.executableURL = try Self.registryHelperURL(helperOverride)
            process.arguments = arguments
            process.standardInput = FileHandle.nullDevice
            let output = Pipe()
            let errors = Pipe()
            process.standardOutput = output
            process.standardError = errors
            do { try process.run() }
            catch {
                throw FactoryClientError.invalidConfiguration(
                    "The bundled local Registry helper could not start."
                )
            }
            async let outputData = output.fileHandleForReading.readDataToEndOfFile()
            async let errorOutputData = errors.fileHandleForReading.readDataToEndOfFile()
            process.waitUntilExit()
            let (data, errorData) = await (outputData, errorOutputData)
            guard process.terminationStatus == 0 else {
                let detail = String(
                    data: errorData.prefix(2_048), encoding: .utf8
                )?.trimmingCharacters(in: .whitespacesAndNewlines)
                throw FactoryClientError.transport(
                    detail?.nilIfEmpty ?? "The local Registry inspection did not complete."
                )
            }
            guard !data.isEmpty, data.count <= 4 * 1024 * 1024 else {
                throw FactoryClientError.invalidResponse(
                    "The local Registry inspection returned invalid data."
                )
            }
            do { return try JSONDecoder.tohseno.decode(Response.self, from: data) }
            catch {
                throw FactoryClientError.invalidResponse(
                    "The local Registry inspection returned an invalid response."
                )
            }
        }.value
    }

    private static func helperURL(_ override: URL?) throws -> URL {
        if let override { return try validatedHelper(override) }
        if let configured = ProcessInfo.processInfo.environment["TOHSENO_NATIVE_HELPER"] {
            return try validatedHelper(URL(fileURLWithPath: configured))
        }
        return try validatedHelper(
            Bundle.main.bundleURL
                .appendingPathComponent("Contents", isDirectory: true)
                .appendingPathComponent("Helpers", isDirectory: true)
                .appendingPathComponent("tohseno", isDirectory: false)
        )
    }

    private static func registryHelperURL(_ override: URL?) throws -> URL {
        if override != nil || ProcessInfo.processInfo.environment["TOHSENO_NATIVE_HELPER"] != nil {
            return try helperURL(override)
        }
        return try validatedHelper(
            Bundle.main.bundleURL
                .appendingPathComponent("Contents", isDirectory: true)
                .appendingPathComponent("Resources", isDirectory: true)
                .appendingPathComponent("FactoryRelease", isDirectory: true)
                .appendingPathComponent("bin", isDirectory: true)
                .appendingPathComponent("tohseno", isDirectory: false)
        )
    }

    private static func validatedHelper(_ url: URL) throws -> URL {
        guard url.isFileURL, url.path.hasPrefix("/") else {
            throw FactoryClientError.invalidConfiguration("The native helper path must be absolute.")
        }
        let values = try url.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey])
        guard values.isRegularFile == true, values.isSymbolicLink != true,
              FileManager.default.isExecutableFile(atPath: url.path) else {
            throw FactoryClientError.invalidConfiguration("The native helper is missing or unsafe.")
        }
        return url
    }
}

private struct EmptyBody: Encodable, Sendable {}
private struct AdoptProjectBody: Encodable, Sendable {
    let path: String
    let scheme: String?
    let harness: String?
    let model: String?
}
private struct CompanionDeviceListResponse: Decodable, Sendable {
    let devices: [PairedCompanionDevice]
}
private struct RenameCompanionDeviceBody: Encodable, Sendable {
    let displayName: String
}
private struct EmptyResponse: Codable, Sendable { init() {} }

private struct CableGenesisResponse: Decodable, Sendable {
    let schema: String
    let step: String
    let instruction: String
    let detail: String?
    let primaryAction: String?
    let automaticallyObserved: Bool
    let companionInstallState: String
    let deviceName: String?
    let deviceProductType: String?
}

private extension ReadinessView {
    init(genesis: CableGenesisResponse) {
        let projectedStep: String
        switch genesis.step {
        case "install_companion":
            if genesis.companionInstallState == "building" {
                projectedStep = "building_companion"
            } else if genesis.companionInstallState == "installing" {
                projectedStep = "installing_companion"
            } else if genesis.companionInstallState == "launching" {
                projectedStep = "launching_companion"
            } else {
                projectedStep = "install_companion"
            }
        case "pairing": projectedStep = "pairing_companion"
        case "pick_up_iphone": projectedStep = "welcome"
        default: projectedStep = genesis.step
        }

        let ready = genesis.step == "first_shot"
        let explanation = switch genesis.step {
        case "pick_up_iphone":
            "Tohseno keeps the iPhone apps you use connected to the Mac, source project, and coding harness that evolve them."
        case "first_shot":
            "Tohseno Companion completed an authenticated exchange with this Mac. Adopt an existing project to start the living connection."
        default:
            genesis.detail ?? "Tohseno checks this step locally and advances only when it can observe success."
        }
        let label: String? = switch genesis.primaryAction {
        case "begin": "Set Up Tohseno"
        case "continue": "Continue"
        case "check": "Check Again"
        case "open_app_store": "Open Xcode in the App Store"
        case "open_xcode_accounts": "Open Xcode"
        case "install_companion": "Install Tohseno Companion"
        case "retry_companion": "Reconnect Tohseno Companion"
        default: nil
        }
        let progress: Double? = switch projectedStep {
        case "building_companion": 0.62
        case "installing_companion": 0.78
        case "launching_companion": 0.88
        case "pairing_companion": 0.95
        default: nil
        }
        self.init(
            schema: "tohseno.native-onboarding-view/1",
            ready: ready,
            step: projectedStep,
            headline: ready ? "Your iPhone is connected" : genesis.instruction,
            detail: explanation,
            primaryAction: ready ? nil : genesis.primaryAction,
            primaryLabel: label,
            automaticallyObserved: genesis.automaticallyObserved,
            progress: progress,
            deviceName: genesis.deviceName,
            deviceProductType: genesis.deviceProductType,
            companionConnected: ready
        )
    }
}

private struct RetireResponse: Codable, Sendable {
    let schema: String
}

private struct RestoreResponse: Codable, Sendable {
    let schema: String
}

private struct ConfigurationReceipt: Decodable, Sendable {
    let schema: String
    let harnessID: String
    let restartRequired: Bool
}

private struct CustomHarnessBody: Encodable, Sendable {
    let id: String
    let label: String
    let executable: String
    let arguments: [String]
    let models: [String]
    let preferred: Bool
}

private struct LocalEndpointBody: Encodable, Sendable {
    let id: String
    let label: String
    let baseURL: String
    let models: [String]
    let credentialReference: String?
    let consentToSendSource: Bool
    let privacyMode: String
    let preferred: Bool
}

private struct ManagedExecutionBody: Encodable, Sendable, Equatable {
    let model: String
    let privacy: String
    let maximumMicrousd: UInt64
    let explicitConsent: Bool
}

private struct ManagedEstimateBody: Encodable, Sendable {
    let model: String
    let privacy: String
    let intentionBytes: UInt64
    let referenceBytes: UInt64
    let sourceContextBytes: UInt64
    let shotID: String?
}

private struct ManagedCheckoutBody: Encodable, Sendable {
    let packID: String
}

private struct APIErrorBody: Codable, Sendable {
    let code: String
    let message: String
}

private struct APIReference: Encodable, Sendable {
    let filename: String
    let mediaType: String
    let origin: String
    let bytesBase64url: String

    init(_ draft: ReferenceDraft) {
        filename = draft.filename
        mediaType = draft.mediaType
        origin = draft.origin
        bytesBase64url = draft.data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }
}

private struct CreateBody: Encodable, Sendable {
    let commandID: String
    let origin = "native"
    let name: String?
    let harness: String?
    let model: String?
    let managed: ManagedExecutionBody?
    let intention: String
    let references: [APIReference]
}

private struct EvolveBody: Encodable, Sendable {
    let commandID: String
    let origin = "native"
    let baseExpressionID: String
    let baseVersionID: String
    let baseVersionOrdinal: UInt64
    let intention: String
    let harness: String?
    let model: String?
    let managed: ManagedExecutionBody?
    let selectedFeedbackActions: [String] = []
    let references: [APIReference]
}

private struct ProjectEvolveBody: Encodable, Sendable {
    let commandID: String
    let origin = "native"
    let baseSourceState: String
    let intention: String
    let references: [APIReference]
    let followUpTo: String? = nil
}

private func validateToken(_ value: String, label: String) throws {
    guard !value.isEmpty, value.count <= 128,
          value.utf8.allSatisfy({
              (65...90).contains($0) || (97...122).contains($0) ||
              (48...57).contains($0) || $0 == 45 || $0 == 95
          }) else {
        throw FactoryClientError.invalidConfiguration("The \(label) is invalid.")
    }
}

private func pathToken(_ value: String) throws -> String {
    try validateToken(value, label: "app identifier")
    return value
}

private func configuredID(_ value: String) throws -> String {
    let value = value.trimmingCharacters(in: .whitespacesAndNewlines)
    try validateToken(value, label: "configured harness identifier")
    return value
}

private func configuredLabel(_ value: String) throws -> String {
    let value = value.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !value.isEmpty, value.utf8.count <= 160, !containsControl(value) else {
        throw FactoryClientError.invalidConfiguration("The configured harness label is invalid.")
    }
    return value
}

private func parseModels(_ value: String) throws -> [String] {
    let models = value.split(whereSeparator: { $0 == "," || $0.isWhitespace }).map(String.init)
    guard !models.isEmpty, models.count <= 32, Set(models).count == models.count else {
        throw FactoryClientError.invalidConfiguration("Provide between one and 32 unique model identifiers.")
    }
    for model in models { try validateToken(model, label: "model identifier") }
    return models
}

private func parseArguments(_ value: String) throws -> [String] {
    let arguments = value.split(whereSeparator: \.isNewline).map {
        $0.trimmingCharacters(in: .whitespaces)
    }.filter { !$0.isEmpty }
    guard arguments.count <= 32,
          arguments.allSatisfy({ $0.utf8.count <= 512 && !containsControl($0) }) else {
        throw FactoryClientError.invalidConfiguration("Custom arguments must be one bounded argument per line.")
    }
    return arguments
}

private func managedBody(
    harness: String?,
    model: String?,
    privacy: String,
    maximum: UInt64?,
    consent: Bool
) throws -> ManagedExecutionBody? {
    guard harness == "tohseno-managed" else { return nil }
    guard let model, let maximum, maximum > 0, consent,
          ["standard", "zdr", "private"].contains(privacy) else {
        throw FactoryClientError.invalidConfiguration(
            "Choose a managed model and privacy mode, review its estimate, and approve the displayed maximum."
        )
    }
    try validateToken(model, label: "managed model")
    return ManagedExecutionBody(
        model: model,
        privacy: privacy,
        maximumMicrousd: maximum,
        explicitConsent: true
    )
}

private func containsControl(_ value: String) -> Bool {
    value.unicodeScalars.contains { CharacterSet.controlCharacters.contains($0) }
}

private extension String {
    var nilIfEmpty: String? { isEmpty ? nil : self }
}
