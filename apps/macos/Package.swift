// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "CloseMyLid",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .executable(name: "CloseMyLid", targets: ["CloseMyLidApp"]),
        .executable(name: "CloseMyLidCoreTests", targets: ["CloseMyLidCoreTests"]),
        .library(name: "CloseMyLidCore", targets: ["CloseMyLidCore"])
    ],
    targets: [
        .executableTarget(
            name: "CloseMyLidApp",
            dependencies: ["CloseMyLidCore"]
        ),
        .target(name: "CloseMyLidCore"),
        .executableTarget(
            name: "CloseMyLidCoreTests",
            dependencies: ["CloseMyLidCore"],
            path: "Tests/CloseMyLidCoreTests"
        )
    ]
)
