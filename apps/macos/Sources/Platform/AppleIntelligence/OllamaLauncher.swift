import AppKit
import WispIPC

/// Starts a local Ollama installation the user already has (spec §12 "Detect … Installed/Running").
public enum OllamaLauncher {
    @MainActor
    public static func start() throws {
        if let app = NSWorkspace.shared.urlForApplication(withBundleIdentifier: "com.electron.ollama")
            ?? (FileManager.default.fileExists(atPath: "/Applications/Ollama.app") ? URL(fileURLWithPath: "/Applications/Ollama.app") : nil) {
            NSWorkspace.shared.openApplication(at: app, configuration: .init())
            return
        }
        for brew in ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"] where FileManager.default.isExecutableFile(atPath: brew) {
            let p = Process()
            p.executableURL = URL(fileURLWithPath: brew)
            p.arguments = ["services", "start", "ollama"]
            try p.run()
            return
        }
        throw IpcError(code: .notFound, details: nil, message: "Ollama isn't installed. Get it from ollama.com.")
    }
}
