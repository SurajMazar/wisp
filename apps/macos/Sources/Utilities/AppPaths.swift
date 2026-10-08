import Foundation

/// Locations inside the app bundle and the user's data directory.
public enum AppPaths {
    /// `~/Library/Application Support/Wisp` (or `WISP_DATA_DIR`).
    public static var dataDirectory: URL {
        if let override = ProcessInfo.processInfo.environment["WISP_DATA_DIR"], !override.isEmpty {
            return URL(fileURLWithPath: override, isDirectory: true)
        }
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        return base.appendingPathComponent("Wisp", isDirectory: true)
    }

    /// The `wisp-core` executable: `WISP_CORE_BIN`, else next to the host executable.
    public static var coreExecutable: URL {
        if let override = ProcessInfo.processInfo.environment["WISP_CORE_BIN"], !override.isEmpty {
            return URL(fileURLWithPath: override)
        }
        return Bundle.main.bundleURL.appendingPathComponent("Contents/MacOS/wisp-core")
    }

    /// Built React UI inside the bundle.
    public static var webRoot: URL? {
        Bundle.main.resourceURL?.appendingPathComponent("web", isDirectory: true)
    }

    /// When set, the UI loads from the Vite dev server instead of bundled assets.
    public static var devServerURL: URL? {
        guard let s = ProcessInfo.processInfo.environment["WISP_DEV_URL"], let url = URL(string: s),
              let host = url.host, ["127.0.0.1", "localhost"].contains(host) else { return nil }
        return url
    }
}
