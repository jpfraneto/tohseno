// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "tohseno-apple-identity",
    platforms: [
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
            name: "TohsenoAppleIdentity",
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
