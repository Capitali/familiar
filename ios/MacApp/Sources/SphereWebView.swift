import SwiftUI
import WebKit

// The Metal Sphere console (imported from Claude Design "Familiar Metal Sphere.dc.html"):
// a WKWebView renders Resources/sphere/index.html — the satellite globe, orbiting glyph
// screens, and hologram panel — while this host does all the daemon I/O natively:
// it polls the loopback /local/worldview and injects the JSON via window.sphereUpdate(),
// and relays the page's human acts (answers, gate flips) to /local/answer | /local/gate.
// The web layer never talks to the network itself except for three.js/fonts/textures (CDN).
struct SphereWebView: NSViewRepresentable {
    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeNSView(context: Context) -> WKWebView {
        let cfg = WKWebViewConfiguration()
        cfg.userContentController.add(context.coordinator, name: "sphere")
        let web = WKWebView(frame: .zero, configuration: cfg)
        web.setValue(false, forKey: "drawsBackground")   // let the page's own space-black show
        context.coordinator.attach(web)
        if let url = Bundle.main.url(forResource: "index", withExtension: "html", subdirectory: "sphere") {
            web.loadFileURL(url, allowingReadAccessTo: url.deletingLastPathComponent())
        }
        return web
    }

    func updateNSView(_ view: WKWebView, context: Context) {}

    static func dismantleNSView(_ view: WKWebView, coordinator: Coordinator) {
        coordinator.detach()
    }

    final class Coordinator: NSObject, WKScriptMessageHandler {
        private weak var web: WKWebView?
        private var timer: Timer?
        private let base = URL(string: "http://127.0.0.1:47100")!

        func attach(_ web: WKWebView) {
            self.web = web
            timer = Timer.scheduledTimer(withTimeInterval: 3, repeats: true) { [weak self] _ in
                Task { await self?.poll() }
            }
            Task { await poll() }
        }

        func detach() {
            timer?.invalidate()
            timer = nil
            web?.configuration.userContentController.removeScriptMessageHandler(forName: "sphere")
        }

        private func poll() async {
            do {
                let (data, resp) = try await URLSession.shared.data(from: base.appendingPathComponent("local/worldview"))
                guard (resp as? HTTPURLResponse)?.statusCode == 200,
                      let json = String(data: data, encoding: .utf8) else { throw URLError(.badServerResponse) }
                await run("window.sphereUpdate(\(json))")
            } catch {
                await run("window.sphereLinkDown && window.sphereLinkDown()")
            }
        }

        @MainActor private func run(_ js: String) {
            web?.evaluateJavaScript(js, completionHandler: nil)
        }

        func userContentController(_ ucc: WKUserContentController, didReceive message: WKScriptMessage) {
            guard let body = message.body as? [String: Any], let kind = body["kind"] as? String else { return }
            switch kind {
            case "answer":
                if let text = body["text"] as? String, !text.isEmpty {
                    post("local/answer", ["text": text])
                }
            case "gate":
                if let gate = body["gate"] as? String {
                    post("local/gate", ["gate": gate, "open": body["open"] as? Bool ?? false])
                }
            default:
                break
            }
        }

        private func post(_ path: String, _ payload: [String: Any]) {
            guard let data = try? JSONSerialization.data(withJSONObject: payload) else { return }
            var req = URLRequest(url: base.appendingPathComponent(path))
            req.httpMethod = "POST"
            req.setValue("application/json", forHTTPHeaderField: "Content-Type")
            req.httpBody = data
            let task = URLSession.shared.dataTask(with: req) { [weak self] _, _, _ in
                Task { await self?.poll() }   // reflect the act right away
            }
            task.resume()
        }
    }
}
