// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "TohsenoCompanionKit",
    platforms: [
        .iOS(.v17),
        .macOS(.v14),
    ],
    products: [
        .library(
            name: "TohsenoCompanionKit",
            targets: ["TohsenoCompanionKit"]
        ),
        .executable(
            name: "TohsenoCompanionInteropProbe",
            targets: ["TohsenoCompanionInteropProbe"]
        ),
    ],
    targets: [
        .target(
            name: "TohsenoCompanionKit",
            resources: [
                .process("Resources"),
            ]
        ),
        .executableTarget(
            name: "TohsenoCompanionInteropProbe",
            dependencies: ["TohsenoCompanionKit"],
            path: "Tools/TohsenoCompanionInteropProbe"
        ),
        .testTarget(
            name: "TohsenoCompanionKitTests",
            dependencies: ["TohsenoCompanionKit"],
            resources: [
                .copy("TestVectors"),
            ]
        ),
    ]
)
