import Foundation
import UniformTypeIdentifiers
import WebKit
import WispUtilities

/// Serves the bundled React build at `wisp://app/…`.
///
/// Paths are resolved inside `root` only; traversal and symlink escapes return 404.
public final class WispSchemeHandler: NSObject, WKURLSchemeHandler {
    public static let scheme = "wisp"
    private let root: URL

    public init(root: URL) {
        self.root = root.standardizedFileURL.resolvingSymlinksInPath()
    }

    public func webView(_ webView: WKWebView, start task: any WKURLSchemeTask) {
        guard let url = task.request.url, url.host == "app" else {
            return fail(task, status: 400)
        }
        var path = url.path
        if path.isEmpty || path == "/" { path = "/index.html" }

        guard let file = resolve(path) else { return fail(task, status: 404, url: url) }
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

    /// Resolve a URL path to a file strictly inside `root`.
    func resolve(_ path: String) -> URL? {
        let decoded = path.removingPercentEncoding ?? path
        guard !decoded.contains("\0") else { return nil }
        let candidate = root.appendingPathComponent(String(decoded.drop(while: { $0 == "/" })))
            .standardizedFileURL.resolvingSymlinksInPath()
        guard candidate.path.hasPrefix(root.path + "/") else { return nil }
        var isDir: ObjCBool = false
        guard FileManager.default.fileExists(atPath: candidate.path, isDirectory: &isDir), !isDir.boolValue else { return nil }
        return candidate
    }

    private func fail(_ task: any WKURLSchemeTask, status: Int, url: URL? = nil) {
        let u = url ?? URL(string: "wisp://app/")!
        task.didReceive(HTTPURLResponse(url: u, statusCode: status, httpVersion: "HTTP/1.1", headerFields: nil)!)
        task.didFinish()
    }
}
