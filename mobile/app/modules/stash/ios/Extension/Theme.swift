import CoreText
import SwiftUI

/// The app's default colours and type, as the phone app draws them.
enum Theme {
  static let ground = Color(hex: "#1e1e22")
  static let raised = Color(hex: "#18181c")
  static let sunken = Color(hex: "#09090b")
  static let border = Color(hex: "#27272b")
  static let borderStrong = Color(hex: "#3f3f43")
  static let ink = Color(hex: "#e7e5ef")
  static let secondary = Color(hex: "#b3b2ba")
  static let tertiary = Color(hex: "#86858c")
  static let inkDim = Color(hex: "#8b8898")
  static let accent = Color(hex: "#a277ff")
  static let live = Color(hex: "#61ffca")
  static let danger = Color(hex: "#ff6767")
  static let rule = Color(hex: "#e7e5ef").opacity(0.09)

  private static let fontFiles = [
    "Figtree_400Regular", "Figtree_500Medium", "Figtree_600SemiBold", "JetBrainsMono_400Regular",
  ]

  /// The app's fonts, read from the app itself so the extension carries no copy of them.
  static func registerFonts() {
    let app = Bundle.main.bundleURL.deletingLastPathComponent().deletingLastPathComponent()
    for name in fontFiles {
      CTFontManagerRegisterFontsForURL(app.appendingPathComponent("\(name).ttf") as CFURL, .process, nil)
    }
  }

  static func ui(_ size: CGFloat) -> Font { .custom("Figtree-Regular", size: size) }
  static func medium(_ size: CGFloat) -> Font { .custom("Figtree-Medium", size: size) }
  static func semibold(_ size: CGFloat) -> Font { .custom("Figtree-SemiBold", size: size) }
  static func mono(_ size: CGFloat) -> Font { .custom("JetBrainsMono-Regular", size: size) }
}

extension Color {
  init(hex: String) {
    var text = hex.trimmingCharacters(in: .whitespaces)
    if text.hasPrefix("#") { text.removeFirst() }
    let value = UInt64(text, radix: 16) ?? 0
    self.init(
      .sRGB,
      red: Double((value >> 16) & 0xff) / 255,
      green: Double((value >> 8) & 0xff) / 255,
      blue: Double(value & 0xff) / 255,
      opacity: 1
    )
  }
}
