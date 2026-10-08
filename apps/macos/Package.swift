// swift-tools-version: 6.0
import PackageDescription

// Wisp's macOS host. Built with SwiftPM (no Xcode project); `scripts/build/build-macos.sh`
// assembles the .app bundle around the `Wisp` executable. See DECISIONS.md D-005.
let package = Package(
    name: "Wisp",
    platforms: [.macOS(.v15)],
    products: [
        .executable(name: "Wisp", targets: ["Wisp"]),
    ],
    targets: [
        .target(name: "WispUtilities", path: "Sources/Utilities"),
        .target(name: "WispIPC", dependencies: ["WispUtilities"], path: "Sources/IPC"),
        .target(name: "WispPlatform", dependencies: ["WispIPC", "WispUtilities"], path: "Sources/Platform"),
        .target(name: "WispUI", dependencies: ["WispIPC", "WispPlatform", "WispUtilities"], path: "Sources/UI"),
        .executableTarget(
            name: "Wisp",
            dependencies: ["WispIPC", "WispPlatform", "WispUI", "WispUtilities"],
            path: "App"
        ),
        .testTarget(
            name: "WispTests",
            dependencies: ["WispIPC", "WispPlatform", "WispUI", "WispUtilities"],
            path: "Tests/WispTests"
        ),
    ],
    swiftLanguageModes: [.v6]
)
