import Darwin
import Foundation

public struct LocalStoragePolicy: Codable, Equatable, Sendable {
    public let localFirst: Bool
    public let cloudKitEnabled: Bool
    public let telemetryEnabled: Bool

    public init(
        localFirst: Bool = true,
        cloudKitEnabled: Bool = false,
        telemetryEnabled: Bool = false
    ) {
        self.localFirst = localFirst
        self.cloudKitEnabled = cloudKitEnabled
        self.telemetryEnabled = telemetryEnabled
    }

    public static let fasciaDefault = LocalStoragePolicy()
}

public enum LocalPersistenceError: Error, Equatable, Sendable {
    case missingApplicationIdentifier
    case unsafeRelativePath
    case nonLocalRoot
}

/// Traversal-safe storage for app-local documents and exported state.
public actor LocalPersistence {
    public let root: URL
    private let fileManager: FileManager

    public init(
        applicationIdentifier: String,
        fileManager: FileManager = .default
    ) throws {
        guard !applicationIdentifier.isEmpty else {
            throw LocalPersistenceError.missingApplicationIdentifier
        }
        guard Self.isSafeApplicationIdentifier(applicationIdentifier) else {
            throw LocalPersistenceError.unsafeRelativePath
        }
        let support = try fileManager.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )
        root = support
            .appendingPathComponent(applicationIdentifier, isDirectory: true)
            .appendingPathComponent("LocalData", isDirectory: true)
            .standardizedFileURL
        self.fileManager = fileManager
        try fileManager.createDirectory(
            at: root,
            withIntermediateDirectories: true
        )
    }

    init(root: URL, fileManager: FileManager = .default) throws {
        guard root.isFileURL else {
            throw LocalPersistenceError.nonLocalRoot
        }
        self.root = root.standardizedFileURL
        self.fileManager = fileManager
        try fileManager.createDirectory(
            at: self.root,
            withIntermediateDirectories: true
        )
    }

    public func write(_ data: Data, to relativePath: String) throws {
        let destination = try resolved(relativePath)
        try fileManager.createDirectory(
            at: destination.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try data.write(
            to: destination,
            options: [.atomic, .completeFileProtection]
        )
    }

    public func read(from relativePath: String) throws -> Data? {
        let source = try resolved(relativePath)
        guard fileManager.fileExists(atPath: source.path) else {
            return nil
        }
        return try Data(contentsOf: source, options: [.mappedIfSafe])
    }

    public func remove(_ relativePath: String) throws {
        let target = try resolved(relativePath)
        if fileManager.fileExists(atPath: target.path) {
            try fileManager.removeItem(at: target)
        }
    }

    private func resolved(_ relativePath: String) throws -> URL {
        guard !relativePath.isEmpty,
              !relativePath.hasPrefix("/"),
              !relativePath.contains("\\"),
              !relativePath.unicodeScalars.contains(where: { $0.value == 0 }),
              !relativePath.split(separator: "/", omittingEmptySubsequences: false)
                .contains(where: { $0.isEmpty || $0 == "." || $0 == ".." })
        else {
            throw LocalPersistenceError.unsafeRelativePath
        }
        let candidate = root
            .appendingPathComponent(relativePath, isDirectory: false)
            .standardizedFileURL
        let rootPrefix = root.path.hasSuffix("/") ? root.path : root.path + "/"
        guard candidate.path.hasPrefix(rootPrefix) else {
            throw LocalPersistenceError.unsafeRelativePath
        }
        var componentURL = root
        for component in relativePath.split(separator: "/") {
            componentURL.appendPathComponent(String(component))
            if Self.isSymbolicLink(componentURL) {
                throw LocalPersistenceError.unsafeRelativePath
            }
        }
        return candidate
    }

    private static func isSafeApplicationIdentifier(_ value: String) -> Bool {
        guard (1 ... 255).contains(value.utf8.count),
              value != ".",
              value != "..",
              !value.hasPrefix("."),
              !value.hasSuffix("."),
              !value.contains("..")
        else {
            return false
        }
        return value.unicodeScalars.allSatisfy {
            switch $0.value {
            case 45, 46, 48 ... 57, 65 ... 90, 97 ... 122:
                true
            default:
                false
            }
        }
    }

    private static func isSymbolicLink(_ url: URL) -> Bool {
        var information = stat()
        return url.withUnsafeFileSystemRepresentation { path in
            guard let path, lstat(path, &information) == 0 else {
                return false
            }
            return information.st_mode & S_IFMT == S_IFLNK
        }
    }
}

/// Small non-secret preferences only. Domain data belongs in SwiftData or
/// `LocalPersistence`; secrets belong in Keychain.
public struct LocalPreferences: @unchecked Sendable {
    private let defaults: UserDefaults

    public init(suiteName: String? = nil) {
        defaults = suiteName.flatMap(UserDefaults.init(suiteName:)) ?? .standard
    }

    public func bool(forKey key: String) -> Bool {
        defaults.bool(forKey: key)
    }

    public func string(forKey key: String) -> String? {
        defaults.string(forKey: key)
    }

    public func set(_ value: Bool, forKey key: String) {
        defaults.set(value, forKey: key)
    }

    public func set(_ value: String?, forKey key: String) {
        defaults.set(value, forKey: key)
    }
}
