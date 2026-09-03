// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "TohsenoWorkshopKit",
    platforms: [
        .iOS(.v17),
        .macOS(.v14),
    ],
    products: [
        .library(name: "TohsenoWorkshopKit", targets: ["TohsenoWorkshopKit"]),
    ],
    targets: [
        .target(name: "TohsenoWorkshopKit"),
        .testTarget(
            name: "TohsenoWorkshopKitTests",
            dependencies: ["TohsenoWorkshopKit"]
        ),
    ]
)
