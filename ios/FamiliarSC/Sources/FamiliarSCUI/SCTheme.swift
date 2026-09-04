import SwiftUI
import FamiliarSC

/// The bridge's instrument palette — the Familiar console's own dark ink and blues, kept
/// here so the package draws the same as the app without depending on it. Restyled from
/// the design canvas when it lands.
public enum SC {
    public static let bg = Color(red: 0.020, green: 0.027, blue: 0.051)
    public static let panel = Color(red: 0.055, green: 0.075, blue: 0.125)
    public static let ink = Color(red: 0.933, green: 0.949, blue: 0.984)
    public static let dim = Color(red: 0.549, green: 0.647, blue: 0.863)
    public static let blue = Color(red: 0.184, green: 0.420, blue: 1.0)
    public static let ice = Color(red: 0.812, green: 0.878, blue: 1.0)
    public static let green = Color(red: 0.239, green: 0.863, blue: 0.592)
    public static let amber = Color(red: 1.0, green: 0.694, blue: 0.353)
    public static let red = Color(red: 1.0, green: 0.420, blue: 0.420)

    public static func color(for mood: BridgeReport.Mood) -> Color {
        switch mood {
        case .steady: return dim
        case .pleased: return green
        case .watchful: return amber
        case .concerned: return red
        }
    }

    public static func glyph(for mood: BridgeReport.Mood) -> String {
        switch mood {
        case .steady: return "circle"
        case .pleased: return "sun.max"
        case .watchful: return "eye"
        case .concerned: return "exclamationmark.triangle"
        }
    }

    public static func money(_ v: Int64?) -> String { v.map { "ℳ\($0)" } ?? "ℳ—" }
}

struct Panel<Content: View>: View {
    let content: Content
    init(@ViewBuilder _ content: () -> Content) { self.content = content() }
    var body: some View {
        content
            .padding(14)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(SC.panel, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
    }
}

struct Chip: View {
    let text: String
    var tint: Color = SC.dim
    var body: some View {
        Text(text)
            .font(.caption2.weight(.semibold))
            .padding(.horizontal, 8).padding(.vertical, 3)
            .background(tint.opacity(0.18), in: Capsule())
            .foregroundStyle(tint)
    }
}
