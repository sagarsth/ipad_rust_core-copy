// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "iPadRustCore",
    platforms: [
        .iOS(.v13),
        .macOS(.v14)
    ],
    products: [
        .library(
            name: "iPadRustCore",
            targets: ["iPadRustCore"]
        ),
        .executable(
            name: "RunMyCodeExample",
            targets: ["RunMyCodeExample"]
        ),
    ],
    targets: [
        .target(
            name: "iPadRustCore",
            dependencies: ["iPadRustCoreC"],
            path: "Sources/iPadRustCore"
        ),
        .target(
            name: "iPadRustCoreC",
            path: "Sources/iPadRustCoreC",
            publicHeadersPath: "include",
            cSettings: [
                .headerSearchPath("include"),
            ],
            linkerSettings: [
                .unsafeFlags(["-L/Users/sagarshrestha/ipad_rust_core copy/Sources/iPadRustCoreC", "-lipad_rust_core"]),
                .linkedFramework("SystemConfiguration")
            ]
        ),
        .executableTarget(
            name: "RunMyCodeExample",
            dependencies: ["iPadRustCore"],
            path: "Sources/RunMyCode"
        ),
        /* // Temporarily commented out due to missing path
        .testTarget(
            name: "iPadRustCoreTests",
            dependencies: ["iPadRustCore"],
            path: "Tests/iPadRustCoreTests"
        ),
        */
    ]
) 