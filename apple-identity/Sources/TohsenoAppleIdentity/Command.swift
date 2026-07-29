import Foundation

public enum AppleIdentityCommand: Equatable, Sendable {
    case create(tag: String, backend: AppleIdentityBackend)
    case showPublic(tag: String)
    case sign(tag: String, digest: Data)
    case delete(tag: String)

    public var name: String {
        switch self {
        case .create: "create"
        case .showPublic: "public"
        case .sign: "sign"
        case .delete: "delete"
        }
    }

    public static func parse(_ arguments: [String]) throws -> AppleIdentityCommand {
        guard let command = arguments.first else {
            throw AppleIdentityCommandError.usage
        }
        let flags = try parseFlags(Array(arguments.dropFirst()))
        guard let tag = flags["--tag"] else {
            throw AppleIdentityCommandError.missing("--tag")
        }
        try AppleIdentityStore.validate(tag: tag)

        switch command {
        case "create":
            let backend: AppleIdentityBackend
            switch flags["--backend"] ?? "secure-enclave" {
            case "secure-enclave":
                backend = .secureEnclave
            case "software-test":
                backend = .softwareTest
            default:
                throw AppleIdentityCommandError.invalidBackend
            }
            try rejectUnexpected(flags, allowed: ["--tag", "--backend"])
            return .create(tag: tag, backend: backend)
        case "public":
            try rejectUnexpected(flags, allowed: ["--tag"])
            return .showPublic(tag: tag)
        case "sign":
            try rejectUnexpected(flags, allowed: ["--tag", "--digest"])
            guard let encoded = flags["--digest"],
                  let digest = Data(strictHexadecimal: encoded, expectedBytes: 32)
            else {
                throw AppleIdentityError.invalidDigest
            }
            return .sign(tag: tag, digest: digest)
        case "delete":
            try rejectUnexpected(flags, allowed: ["--tag"])
            return .delete(tag: tag)
        default:
            throw AppleIdentityCommandError.unknownCommand(command)
        }
    }

    private static func parseFlags(_ arguments: [String]) throws -> [String: String] {
        guard arguments.count % 2 == 0 else {
            throw AppleIdentityCommandError.usage
        }
        var result: [String: String] = [:]
        for index in stride(from: 0, to: arguments.count, by: 2) {
            let flag = arguments[index]
            guard flag.hasPrefix("--"), result[flag] == nil else {
                throw AppleIdentityCommandError.duplicateOrInvalidFlag(flag)
            }
            result[flag] = arguments[index + 1]
        }
        return result
    }

    private static func rejectUnexpected(
        _ flags: [String: String],
        allowed: Set<String>
    ) throws {
        if let unexpected = flags.keys.first(where: { !allowed.contains($0) }) {
            throw AppleIdentityCommandError.unexpected(unexpected)
        }
    }
}

public enum AppleIdentityCommandError: Error, Equatable, Sendable {
    case usage
    case missing(String)
    case unknownCommand(String)
    case invalidBackend
    case duplicateOrInvalidFlag(String)
    case unexpected(String)

    public var code: String {
        switch self {
        case .usage: "usage"
        case .missing: "missing_argument"
        case .unknownCommand: "unknown_command"
        case .invalidBackend: "invalid_backend"
        case .duplicateOrInvalidFlag: "invalid_argument"
        case .unexpected: "unexpected_argument"
        }
    }

    public var safeMessage: String {
        switch self {
        case .usage:
            "usage: tohseno-apple-identity <create|public|sign|delete> --tag <tag>"
        case let .missing(flag):
            "missing required argument \(flag)"
        case let .unknownCommand(command):
            "unknown command \(command)"
        case .invalidBackend:
            "backend must be secure-enclave or software-test"
        case let .duplicateOrInvalidFlag(flag):
            "invalid or duplicate argument \(flag)"
        case let .unexpected(flag):
            "unexpected argument \(flag)"
        }
    }
}
