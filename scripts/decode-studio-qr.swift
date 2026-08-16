#!/usr/bin/env swift

import Foundation
import Vision

guard CommandLine.arguments.count == 3 else {
    FileHandle.standardError.write(Data("usage: decode-studio-qr.swift INPUT_PNG PRIVATE_OUTPUT\n".utf8))
    exit(64)
}

let imageURL = URL(fileURLWithPath: CommandLine.arguments[1])
let outputURL = URL(fileURLWithPath: CommandLine.arguments[2])
let request = VNDetectBarcodesRequest()
request.symbologies = [.qr]
try VNImageRequestHandler(url: imageURL).perform([request])
let values = (request.results ?? []).compactMap(\.payloadStringValue)
guard values.count == 1, let value = values.first else {
    throw NSError(domain: "tohseno.qr-decoder", code: 1)
}
try Data(value.utf8).write(to: outputURL, options: .atomic)
try FileManager.default.setAttributes(
    [.posixPermissions: 0o600],
    ofItemAtPath: outputURL.path
)
let result: [String: Any] = [
    "schema": "tohseno.studio-qr-independent-decode/1",
    "decoder": "Apple Vision VNDetectBarcodesRequest",
    "qr_observations": values.count,
    "tohseno_pair_v1_prefix": value.hasPrefix("tohseno://pair/v1/"),
    "payload_within_bound": value.utf8.count <= 32 * 1024,
    "private_output_mode": "0600",
]
let encoded = try JSONSerialization.data(withJSONObject: result, options: [.sortedKeys])
FileHandle.standardOutput.write(encoded)
FileHandle.standardOutput.write(Data("\n".utf8))
