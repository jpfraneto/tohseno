#!/usr/bin/env swift

import Foundation
import Security

guard CommandLine.arguments.count == 5 else {
    FileHandle.standardError.write(Data("usage: verify-keychain-isolation.swift KEYCHAIN SERVICE ACCOUNT SCAN_ROOT\n".utf8))
    exit(64)
}
let keychainPath = CommandLine.arguments[1]
let service = CommandLine.arguments[2]
let account = CommandLine.arguments[3]
let scanRoot = URL(fileURLWithPath: CommandLine.arguments[4], isDirectory: true)

var keychain: SecKeychain?
guard SecKeychainOpen(keychainPath, &keychain) == errSecSuccess, let keychain else {
    throw NSError(domain: "tohseno.keychain-verification", code: 1)
}
guard SecKeychainUnlock(keychain, 0, nil, false) == errSecSuccess else {
    throw NSError(domain: "tohseno.keychain-verification", code: 2)
}
var item: CFTypeRef?
let status = SecItemCopyMatching([
    kSecClass: kSecClassGenericPassword,
    kSecAttrService: service,
    kSecAttrAccount: account,
    kSecMatchSearchList: [keychain],
    kSecMatchLimit: kSecMatchLimitOne,
    kSecReturnData: true,
] as CFDictionary, &item)
guard status == errSecSuccess, let secret = item as? Data, secret.count == 64 else {
    throw NSError(domain: "tohseno.keychain-verification", code: Int(status))
}

let keys: [URLResourceKey] = [.isRegularFileKey, .isSymbolicLinkKey, .fileSizeKey]
let enumerator = FileManager.default.enumerator(
    at: scanRoot,
    includingPropertiesForKeys: keys,
    options: [.skipsHiddenFiles]
)
var scannedFiles = 0
var leaked = false
while let file = enumerator?.nextObject() as? URL {
    let values = try file.resourceValues(forKeys: Set(keys))
    guard values.isSymbolicLink != true, values.isRegularFile == true else { continue }
    if file.path == keychainPath || file.lastPathComponent.hasSuffix(".keychain-db") { continue }
    guard let size = values.fileSize, size <= 64 * 1024 * 1024 else { continue }
    scannedFiles += 1
    if try Data(contentsOf: file, options: .mappedIfSafe).range(of: secret) != nil {
        leaked = true
        break
    }
}
let result: [String: Any] = [
    "schema": "tohseno.real-keychain-verification/1",
    "keychain_api": "SecKeychainOpen + SecItemCopyMatching",
    "keychain_item_present": true,
    "workspace_secret_byte_count": secret.count,
    "ordinary_files_scanned": scannedFiles,
    "secret_found_in_ordinary_files": leaked,
]
let encoded = try JSONSerialization.data(withJSONObject: result, options: [.sortedKeys])
FileHandle.standardOutput.write(encoded)
FileHandle.standardOutput.write(Data("\n".utf8))
if leaked { exit(1) }
