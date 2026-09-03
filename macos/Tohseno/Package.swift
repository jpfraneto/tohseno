// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "TohsenoMac",
    // Observation and SwiftUI's modern Settings/window restoration APIs are
    // native on macOS 14. The Rust service itself remains compatible with the
    // older CLI-supported systems.
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "TohsenoMacCore", targets: ["TohsenoMacCore"]),
        .executable(name: "TohsenoMacApp", targets: ["TohsenoMacApp"]),
    ],
    dependencies: [
        .package(path: "../../sdk/apple/TohsenoWorkshopKit"),
    ],
    targets: [
        .target(
            name: "TohsenoMacCore",
            dependencies: [
                .product(name: "TohsenoWorkshopKit", package: "TohsenoWorkshopKit"),
            ]
        ),
        .executableTarget(
            name: "TohsenoMacApp",
            dependencies: ["TohsenoMacCore"],
            path: "App"
        ),
        .testTarget(
            name: "TohsenoMacCoreTests",
            dependencies: ["TohsenoMacCore"]
        ),
    ]
)
