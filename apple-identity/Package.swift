// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "tohseno-apple-identity",
    platforms: [
        .iOS(.v17),
        .macOS(.v13),
    ],
    products: [
        .library(
            name: "TohsenoAppleIdentity",
            targets: ["TohsenoAppleIdentity"]
        ),
        .executable(
            name: "tohseno-apple-identity",
            targets: ["TohsenoAppleIdentityCLI"]
        ),
    ],
    targets: [
        .target(
            name: "CTohsenoVerificationKeychain",
            path: "Sources/CTohsenoVerificationKeychain",
            publicHeadersPath: "include"
        ),
        .target(
            name: "TohsenoAppleIdentity",
            dependencies: ["CTohsenoVerificationKeychain"],
            path: "Sources/TohsenoAppleIdentity"
        ),
        .executableTarget(
            name: "TohsenoAppleIdentityCLI",
            dependencies: ["TohsenoAppleIdentity"],
            path: "Sources/tohseno-apple-identity"
        ),
        .testTarget(
            name: "TohsenoAppleIdentityTests",
            dependencies: ["TohsenoAppleIdentity"],
            path: "Tests/TohsenoAppleIdentityTests"
        ),
    ]
)
