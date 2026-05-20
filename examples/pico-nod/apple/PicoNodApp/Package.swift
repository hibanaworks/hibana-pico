// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "PicoNodApp",
    platforms: [
        .iOS(.v17),
        .macOS(.v14),
    ],
    products: [
        .library(name: "PicoNodAppCore", targets: ["PicoNodAppCore"]),
        .executable(name: "PicoNodApp", targets: ["PicoNodApp"]),
    ],
    targets: [
        .target(name: "PicoNodAppCore"),
        .executableTarget(
            name: "PicoNodApp",
            dependencies: ["PicoNodAppCore"],
            resources: [
                .copy("Assets.xcassets"),
                .copy("PrivacyInfo.xcprivacy"),
                .copy("PicoNod.entitlements"),
                .copy("LaunchScreen.storyboard"),
            ]
        ),
        .testTarget(
            name: "PicoNodAppCoreTests",
            dependencies: ["PicoNodAppCore"]
        ),
    ]
)
