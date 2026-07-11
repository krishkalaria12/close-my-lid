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
    dependencies: [
        .package(url: "https://github.com/sparkle-project/Sparkle", exact: "2.9.4")
    ],
    targets: [
        .executableTarget(
            name: "CloseMyLidApp",
            dependencies: [
                "CloseMyLidCore",
                .product(name: "Sparkle", package: "Sparkle")
            ],
            linkerSettings: [
                .unsafeFlags([
                    "-Xlinker", "-rpath",
                    "-Xlinker", "@executable_path/../Frameworks"
                ])
            ]
        ),
        .target(name: "CloseMyLidCore"),
        .executableTarget(
            name: "CloseMyLidCoreTests",
            dependencies: ["CloseMyLidCore"],
            path: "Tests/CloseMyLidCoreTests"
        )
    ]
)
