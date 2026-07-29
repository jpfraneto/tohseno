// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "TohsenoAppleFascia",
    platforms: [
        .iOS(.v17),
        .macOS(.v14),
        .visionOS(.v1),
    ],
    products: [
        .library(
            name: "TohsenoAppleFascia",
            targets: ["TohsenoAppleFascia"]
        ),
    ],
    targets: [
        .target(
            name: "TohsenoAppleFascia",
            path: "swift"
        ),
        .testTarget(
            name: "TohsenoAppleFasciaTests",
            dependencies: ["TohsenoAppleFascia"],
            path: "tests",
            exclude: ["validate-fascia.sh"]
        ),
    ]
)
