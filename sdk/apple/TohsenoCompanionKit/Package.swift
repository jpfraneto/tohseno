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
    dependencies: [
        .package(path: "../../../apple-identity"),
        .package(path: "../TohsenoWorkshopKit"),
    ],
    targets: [
        .target(
            name: "TohsenoCompanionKit",
            dependencies: [
                .product(name: "TohsenoAppleIdentity", package: "apple-identity"),
                .product(name: "TohsenoWorkshopKit", package: "TohsenoWorkshopKit"),
            ],
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
