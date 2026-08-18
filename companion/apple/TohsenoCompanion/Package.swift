// swift-tools-version: 6.0

import PackageDescription

// The TOHSENO Companion product surface.
//
// All of the product lives in a library target so it can be built and tested
// without an iOS Simulator; `App/` is a thin @main shell that hosts it. The
// package depends on the released Companion SDK and implements no second
// protocol, backend, synchronization mechanism, or mobile coding harness.
let package = Package(
    name: "TohsenoCompanion",
    platforms: [
        .iOS(.v17),
        .macOS(.v14),
    ],
    products: [
        .library(
            name: "TohsenoCompanionApp",
            targets: ["TohsenoCompanionApp"]
        ),
    ],
    dependencies: [
        .package(path: "../../../sdk/apple/TohsenoCompanionKit"),
    ],
    targets: [
        .target(
            name: "TohsenoCompanionApp",
            dependencies: [
                .product(name: "TohsenoCompanionKit", package: "TohsenoCompanionKit"),
            ],
            resources: [
                .process("Resources"),
            ]
        ),
        .testTarget(
            name: "TohsenoCompanionAppTests",
            dependencies: ["TohsenoCompanionApp"]
        ),
    ]
)
