import Foundation
import WebKit

/// Serves the bundled sphere console over a custom URL scheme instead of `file://`.
///
/// **Why this exists.** The console is an ES module (`<script type="module">` + an import map for
/// three.js). A `file://` document has an *opaque* origin, so WebKit treats every module import as
/// cross-origin and refuses it — and it reports the failure as a bare `"Script error." at :0:0`
/// with no file and no line, because cross-origin script errors are deliberately opaque. The
/// symptom is the console sitting on its loading spinner forever: the module never runs, so
/// `window.sphereUpdate` is never defined and nothing ever clears `S.loading`.
///
/// This bit only when the assets were vendored (2026-07-30). Before that, three.js came from
/// `https://unpkg.com` — a real origin with CORS headers, which WebKit happily loads even from a
/// `file://` page. Vendoring turned a cross-origin *https* import into a same-directory *file*
/// import and quietly crossed WebKit's line. It survived a first round of testing because
/// `file://` module loading is inconsistent across WebKit versions; a Safari 27 update is the most
/// likely reason it started failing on a build that had worked.
///
/// A custom scheme gives the document a real, stable origin, so modules, the import map and
/// relative asset paths all behave exactly as they do over http — without shipping a web server or
/// depending on the network. This is the supported WKWebView answer to precisely this problem.
enum SphereScheme {
    /// Must be a scheme WebKit does not already handle — registering `file`, `http(s)`, `about`
    /// and friends throws. Kept distinctive so it can't collide with anything else in-process.
    static let name = "familiar-sphere"
    static let indexURL = URL(string: "\(name)://console/index.html")!

    /// Attach the handler to a configuration before the web view is created. Doing it afterwards
    /// has no effect — WKWebView copies its configuration at init.
    static func register(on config: WKWebViewConfiguration) {
        config.setURLSchemeHandler(Handler(), forURLScheme: name)
    }

    /// Resolves `familiar-sphere://console/<path>` to a file inside the bundled `sphere/` folder.
    final class Handler: NSObject, WKURLSchemeHandler {
        /// The bundled folder reference (`sphere/`), resolved once.
        private static let root: URL? = Bundle.main
            .url(forResource: "index", withExtension: "html", subdirectory: "sphere")?
            .deletingLastPathComponent()

        func webView(_ webView: WKWebView, start task: WKURLSchemeTask) {
            guard let root = Self.root, let url = task.request.url else {
                task.didFailWithError(URLError(.fileDoesNotExist))
                return
            }
            // Strip the leading "/" and normalise. `standardized` collapses any "..", and the
            // prefix check below is what actually enforces the jail — a bundled page should never
            // be able to read outside its own folder even though it is our own content.
            var rel = url.path
            if rel.hasPrefix("/") { rel.removeFirst() }
            if rel.isEmpty { rel = "index.html" }
            let target = root.appendingPathComponent(rel).standardized
            guard target.path.hasPrefix(root.standardized.path),
                  let data = try? Data(contentsOf: target)
            else {
                task.didFailWithError(URLError(.fileDoesNotExist))
                return
            }
            let response = HTTPURLResponse(
                url: url,
                statusCode: 200,
                httpVersion: "HTTP/1.1",
                // A module is rejected outright unless it is served as JavaScript, so the MIME
                // type is load-bearing here rather than cosmetic.
                headerFields: ["Content-Type": Self.mime(for: target.pathExtension),
                               "Access-Control-Allow-Origin": "*"]
            )!
            task.didReceive(response)
            task.didReceive(data)
            task.didFinish()
        }

        func webView(_ webView: WKWebView, stop task: WKURLSchemeTask) {}

        private static func mime(for ext: String) -> String {
            switch ext.lowercased() {
            case "html": return "text/html; charset=utf-8"
            case "js", "mjs": return "text/javascript; charset=utf-8"
            case "css": return "text/css; charset=utf-8"
            case "json": return "application/json"
            case "png": return "image/png"
            case "jpg", "jpeg": return "image/jpeg"
            case "svg": return "image/svg+xml"
            case "woff2": return "font/woff2"
            case "woff": return "font/woff"
            default: return "application/octet-stream"
            }
        }
    }
}
