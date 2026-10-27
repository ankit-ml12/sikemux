import AppKit
import WebKit

// Draws a page as page_preview does, in an offscreen WebKit view: runs its script, measures it and writes a PNG.
// Arguments: page (the themed copy), folder the page may read from, width, picture path ("" for none), script.
let arguments = CommandLine.arguments
let page = URL(fileURLWithPath: arguments[1])
let readable = URL(fileURLWithPath: arguments[2], isDirectory: true)
let width = CGFloat(Double(arguments[3]) ?? 756)
let picture = arguments[4]
let script = arguments.count > 5 ? arguments[5] : ""

let console = """
window.__messages = [];
for (const level of ['log', 'info', 'warn', 'error', 'debug']) {
  const original = console[level];
  console[level] = (...parts) => { window.__messages.push({ level, text: parts.map(String).join(' ') }); original.apply(console, parts); };
}
window.addEventListener('error', (event) => window.__messages.push({ level: 'error', text: String(event.message) }));
"""

final class Preview: NSObject, WKNavigationDelegate {
  let view: WKWebView

  override init() {
    let configuration = WKWebViewConfiguration()
    configuration.websiteDataStore = .nonPersistent()
    configuration.userContentController.addUserScript(WKUserScript(source: console, injectionTime: .atDocumentStart, forMainFrameOnly: true))
    view = WKWebView(frame: NSRect(x: 0, y: 0, width: width, height: 900), configuration: configuration)
    super.init()
    view.navigationDelegate = self
    view.loadFileURL(page, allowingReadAccessTo: readable)
  }

  func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { self.run() }
  }

  func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
    finish(["error": error.localizedDescription])
  }

  func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
    finish(["error": error.localizedDescription])
  }

  func run() {
    guard !script.isEmpty else { return measure(nil) }
    let failed = { (error: Error) -> String in
      String(describing: (error as NSError).userInfo["WKJavaScriptExceptionMessage"] ?? error.localizedDescription)
    }
    view.callAsyncJavaScript("return { value: await (\(script)\n) };", arguments: [:], in: nil, in: .page) { first in
      switch first {
      case .success(let wrapped): self.measure((wrapped as? [String: Any])?["value"] ?? NSNull())
      case .failure(let error) where !failed(error).contains("SyntaxError"): self.measure(["error": failed(error)])
      case .failure:
        self.view.evaluateJavaScript("window.__messages = window.__messages.filter((message) => message.text !== 'Script error.')")
        self.view.callAsyncJavaScript(script, arguments: [:], in: nil, in: .page) { second in
          switch second {
          case .success(let value): self.measure(value)
          case .failure(let error): self.measure(["error": failed(error)])
          }
        }
      }
    }
  }

  func measure(_ result: Any?) {
    view.frame = NSRect(x: 0, y: 0, width: width, height: 1)
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { self.report(result) }
  }

  func report(_ result: Any?) {
    view.evaluateJavaScript("JSON.stringify({ height: Math.ceil(document.documentElement.scrollHeight), messages: window.__messages })") { answer, _ in
      let measured = (try? JSONSerialization.jsonObject(with: Data(((answer as? String) ?? "{}").utf8))) as? [String: Any] ?? [:]
      let height = max(1, (measured["height"] as? NSNumber)?.doubleValue ?? 900)
      var report: [String: Any] = ["contentHeight": height, "messages": measured["messages"] ?? []]
      if let result { report["script"] = result }
      guard !picture.isEmpty else { return self.finish(report) }
      self.view.frame = NSRect(x: 0, y: 0, width: width, height: min(height, 14_400))
      DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
        let shot = WKSnapshotConfiguration()
        shot.rect = self.view.bounds
        shot.snapshotWidth = NSNumber(value: Double(width))
        self.view.takeSnapshot(with: shot) { image, _ in
          if let image, let tiff = image.tiffRepresentation, let bitmap = NSBitmapImageRep(data: tiff), let png = bitmap.representation(using: .png, properties: [:]) {
            try? png.write(to: URL(fileURLWithPath: picture))
          }
          self.finish(report)
        }
      }
    }
  }

  func finish(_ report: [String: Any]) {
    let safe = JSONSerialization.isValidJSONObject(report) ? report : ["contentHeight": report["contentHeight"] ?? 900, "messages": [], "script": String(describing: report["script"] ?? "")]
    FileHandle.standardOutput.write((try? JSONSerialization.data(withJSONObject: safe)) ?? Data("{}".utf8))
    exit(0)
  }
}

let app = NSApplication.shared
app.setActivationPolicy(.prohibited)
let preview = Preview()
DispatchQueue.main.asyncAfter(deadline: .now() + 20) { preview.finish(["error": "the page took too long to answer"]) }
app.run()
