// swift-tools-version:5.5
import PackageDescription

let package = Package(
    name: "macos-obs",
    platforms: [.macOS(.v10_15)],
    products: [
        .library(name: "macos-obs", type: .static, targets: ["macos-obs"]),
    ],
    dependencies: [
        .package(url: "https://github.com/Brendonovich/swift-rs", from: "1.0.0")
    ],
    targets: [
        .target(
            name: "macos-obs",
            dependencies: [.product(name: "SwiftRs", package: "swift-rs")]
        ),
    ]
)