import AppKit
import WebKit
import WispIPC
import WispUtilities

/// A WKWebView hosting a React route, bridged to the IPC router.
///
/// Inbound messages are accepted only from the main frame of the expected origin
/// (`wisp://app` or the configured dev server). Outbound envelopes are passed as
/// structured arguments to `window.__wispReceive` — never string-interpolated into script.
@MainActor
public final class WebSurface: NSObject, EnvelopeSink {
    public let webView: WKWebView
    private let router: IPCRouter
    private let route: String

    public init(router: IPCRouter, route: String, transparent: Bool = false) {
        self.router = router
        self.route = route

        let config = WKWebViewConfiguration()
        config.suppressesIncrementalRendering = true
        var roots = ["focus": AppPaths.focusCacheDirectory]
        if let root = AppPaths.webRoot { roots["app"] = root }
        config.setURLSchemeHandler(WispSchemeHandler(roots: roots), forURLScheme: WispSchemeHandler.scheme)
        config.preferences.isElementFullscreenEnabled = false
        config.defaultWebpagePreferences.allowsContentJavaScript = true

        webView = WKWebView(frame: .zero, configuration: config)
        webView.allowsBackForwardNavigationGestures = false
        webView.allowsMagnification = false
        if transparent {
            // No public API for a transparent WKWebView background on macOS; this KVC key is the
            // long-standing supported path (used by Safari extensions and many apps).
            webView.setValue(false, forKey: "drawsBackground")
        }
        #if DEBUG
        webView.isInspectable = true
        #endif
        super.init()

        config.userContentController.add(WeakMessageHandler(self), name: "wisp")
        webView.navigationDelegate = self
        router.attach(self)
    }

    public func load() {
        let url: URL
        if let dev = AppPaths.devServerURL {
            url = URL(string: "#\(route)", relativeTo: dev)!.absoluteURL
        } else {
            url = URL(string: "wisp://app/index.html#\(route)")!
        }
        webView.load(URLRequest(url: url))
    }

    public func navigate(to route: String) {
        let arg = ["route": route]
        webView.callAsyncJavaScript("window.location.hash = route", arguments: arg, in: nil, in: .page) { _ in }
    }

    public func deliver(_ envelope: Envelope) {
        guard let value = try? JSONValue.from(envelope) else { return }
        webView.callAsyncJavaScript(
            "if (window.__wispReceive) { window.__wispReceive(envelope) }",
            arguments: ["envelope": value.foundationObject],
            in: nil,
            in: .page
        ) { result in
            if case .failure(let error) = result {
                Log.ui.debug("deliver failed: \(error.localizedDescription, privacy: .public)")
            }
        }
    }

    fileprivate func received(_ message: WKScriptMessage) {
        guard message.frameInfo.isMainFrame, isTrusted(message.frameInfo.securityOrigin) else {
            Log.ipc.error("rejected message from untrusted frame")
            return
        }
        guard JSONSerialization.isValidJSONObject(message.body),
              let data = try? JSONSerialization.data(withJSONObject: message.body) else {
            Log.ipc.error("rejected non-JSON message body")
            return
        }
        router.receive(json: data, from: self)
    }

    private func isTrusted(_ origin: WKSecurityOrigin) -> Bool {
        if origin.protocol == WispSchemeHandler.scheme && origin.host == "app" { return true }
        if let dev = AppPaths.devServerURL, origin.protocol == dev.scheme, origin.host == dev.host,
           origin.port == (dev.port ?? 0) { return true }
        return false
    }
}

extension WebSurface: WKNavigationDelegate {
    /// Only the app's own pages may load in the surface; anything else opens in the default browser
    /// (http/https only) or is blocked.
    public func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction) async -> WKNavigationActionPolicy {
        guard let url = action.request.url else { return .cancel }
        if url.scheme == WispSchemeHandler.scheme { return .allow }
        if let dev = AppPaths.devServerURL, url.scheme == dev.scheme, url.host == dev.host, url.port == dev.port {
            return .allow
        }
        if action.navigationType == .linkActivated, ["http", "https"].contains(url.scheme) {
            NSWorkspace.shared.open(url)
        }
        return .cancel
    }

    public func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        Log.ui.error("web content process terminated; reloading")
        load()
    }
}

/// Breaks the WKUserContentController → handler retain cycle.
private final class WeakMessageHandler: NSObject, WKScriptMessageHandler {
    weak var target: WebSurface?
    init(_ target: WebSurface) { self.target = target }

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        MainActor.assumeIsolated { target?.received(message) }
    }
}
