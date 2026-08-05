import Darwin
import Foundation
import TohsenoAppleIdentity

private let schema = "tohseno.apple-identity/1"

private struct Success<Result: Encodable>: Encodable {
    let schema: String
    let ok: Bool
    let command: String
    let result: Result
}

private struct DeleteResult: Encodable {
    let tag: String
    let deleted: Bool
    let backend: AppleIdentityBackend
    let testOnly: Bool
}

private struct Failure: Encodable {
    let schema: String
    let ok: Bool
    let error: FailureDetail
}

private struct FailureDetail: Encodable {
    let code: String
    let message: String
}

private func encoded<T: Encodable>(_ value: T) throws -> Data {
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
    encoder.keyEncodingStrategy = .convertToSnakeCase
    return try encoder.encode(value) + Data([0x0a])
}

private func write<T: Encodable>(_ value: T, to handle: FileHandle) {
    do {
        try handle.write(contentsOf: encoded(value))
    } catch {
        let fallback = #"{"error":{"code":"json_encoding_failure","message":"could not encode response"},"ok":false,"schema":"tohseno.apple-identity/1"}"# + "\n"
        try? handle.write(contentsOf: Data(fallback.utf8))
    }
}

private func failure(for error: Error) -> Failure {
    if let error = error as? AppleIdentityError {
        return Failure(
            schema: schema,
            ok: false,
            error: FailureDetail(code: error.code, message: error.safeMessage)
        )
    }
    if let error = error as? AppleIdentityCommandError {
        return Failure(
            schema: schema,
            ok: false,
            error: FailureDetail(code: error.code, message: error.safeMessage)
        )
    }
    return Failure(
        schema: schema,
        ok: false,
        error: FailureDetail(
            code: "internal_failure",
            message: "Apple identity operation failed"
        )
    )
}

do {
    let arguments = Array(CommandLine.arguments.dropFirst())
    if arguments == ["--help"] || arguments == ["-h"] {
        print("""
        Usage:
          tohseno-apple-identity create --tag <tag> [--backend secure-enclave|software-test]
          tohseno-apple-identity public --tag <tag>
          tohseno-apple-identity sign --tag <tag> --digest <32-byte-hex>
          tohseno-apple-identity delete --tag <tag>
        """)
        exit(EXIT_SUCCESS)
    }
    if arguments == ["--version"] {
        print("tohseno-apple-identity 0.8.4")
        exit(EXIT_SUCCESS)
    }

    let command = try AppleIdentityCommand.parse(arguments)
    let store = AppleIdentityStore.shared
    switch command {
    case let .create(tag, backend):
        let result = try store.create(tag: tag, backend: backend)
        write(
            Success(schema: schema, ok: true, command: command.name, result: result),
            to: .standardOutput
        )
    case let .showPublic(tag):
        let result = try store.publicIdentity(tag: tag)
        write(
            Success(schema: schema, ok: true, command: command.name, result: result),
            to: .standardOutput
        )
    case let .sign(tag, digest):
        let result = try store.sign(tag: tag, digest: digest)
        write(
            Success(schema: schema, ok: true, command: command.name, result: result),
            to: .standardOutput
        )
    case let .delete(tag):
        let identity = try store.delete(tag: tag)
        let result = DeleteResult(
            tag: tag,
            deleted: true,
            backend: identity.backend,
            testOnly: identity.testOnly
        )
        write(
            Success(schema: schema, ok: true, command: command.name, result: result),
            to: .standardOutput
        )
    }
} catch {
    write(failure(for: error), to: .standardError)
    exit(EXIT_FAILURE)
}
