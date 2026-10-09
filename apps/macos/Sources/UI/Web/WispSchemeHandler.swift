import Foundation
import UniformTypeIdentifiers
import WebKit
import WispUtilities

/// Serves Wisp's own content to web views:
/// * `wisp://app/…`   — the bundled React build
/// * `wisp://focus/<id>.png` — focus captures from the cache (PNG only)
///
/// Paths are resolved inside each host's root only; traversal and symlink escapes return 404.
public final class WispSchemeHandler: NSObject, WKURLSchemeHandler {
    public static let scheme = "wisp"
    private let roots: [String: URL]
    private let allowedExtensions: [String: Set<String>] = ["focus": ["png"]]

    public init(roots: [String: URL]) {
        self.roots = roots.mapValues { $0.standardizedFileURL.resolvingSymlinksInPath() }
    }

    public convenience init(root: URL) {
        self.init(roots: ["app": root])
    }

    public func webView(_ webView: WKWebView, start task: any WKURLSchemeTask) {
        guard let url = task.request.url, let host = url.host, roots[host] != nil else {
            return fail(task, status: 400)
        }
        var path = url.path
        if host == "app", path.isEmpty || path == "/" { path = "/index.html" }

        guard let file = resolve(path, host: host) else { return fail(task, status: 404, url: url) }
        do {
            let data = try Data(contentsOf: file)
            let mime = UTType(filenameExtension: file.pathExtension)?.preferredMIMEType ?? "application/octet-stream"
            let response = HTTPURLResponse(
                url: url, statusCode: 200, httpVersion: "HTTP/1.1",
                headerFields: [
                    "Content-Type": mime,
                    "Content-Length": String(data.count),
                    "Cache-Control": "no-cache",
                    "X-Content-Type-Options": "nosniff",
                ]
            )!
            task.didReceive(response)
            task.didReceive(data)
            task.didFinish()
        } catch {
            fail(task, status: 404, url: url)
        }
    }

    public func webView(_ webView: WKWebView, stop task: any WKURLSchemeTask) {}

    /// Resolve a URL path to a file strictly inside the host's root.
    func resolve(_ path: String, host: String = "app") -> URL? {
        guard let root = roots[host] else { return nil }
        let decoded = path.removingPercentEncoding ?? path
        guard !decoded.contains("\0") else { return nil }
        let candidate = root.appendingPathComponent(String(decoded.drop(while: { $0 == "/" })))
            .standardizedFileURL.resolvingSymlinksInPath()
        guard candidate.path.hasPrefix(root.path + "/") else { return nil }
        var isDir: ObjCBool = false
        guard FileManager.default.fileExists(atPath: candidate.path, isDirectory: &isDir), !isDir.boolValue else { return nil }
        if let allowed = allowedExtensions[host], !allowed.contains(candidate.pathExtension.lowercased()) { return nil }
        return candidate
    }

    private func fail(_ task: any WKURLSchemeTask, status: Int, url: URL? = nil) {
        let u = url ?? URL(string: "wisp://app/")!
        task.didReceive(HTTPURLResponse(url: u, statusCode: status, httpVersion: "HTTP/1.1", headerFields: nil)!)
        task.didFinish()
    }
}
