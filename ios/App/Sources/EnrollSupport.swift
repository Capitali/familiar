import SwiftUI

// The enroll flow's design pieces — the palette, the breathing sphere, and the glass panel —
// kept from the retired GlassConsole (the sphere took over everything post-enrollment).

enum Fam {
    static let bg = Color(hex: 0x05070d)
    static let ink = Color(hex: 0xeef2fb)
    static let blue = Color(hex: 0x2f6bff)
    static let blueBright = Color(hex: 0x6c9bff)
    static let blueLink = Color(hex: 0x7aa2ff)
    static let blueSoft = Color(hex: 0x9cc0ff)
    static let iceStat = Color(hex: 0xcfe0ff)
    static let green = Color(hex: 0x3ddc97)
    static let greenSoft = Color(hex: 0x7ce0b4)
    static let amber = Color(hex: 0xffb15a)
    static let red = Color(hex: 0xff6b6b)
    static let monoDim = Color(hex: 0x8ca5dc)
    static let labelBlue = Color(hex: 0x96b4ff)

    static func surface(_ o: Double = 0.04) -> Color { Color.white.opacity(o) }
    static func hairline(_ o: Double = 0.07) -> Color { Color.white.opacity(o) }
    static func inkDim(_ o: Double) -> Color { ink.opacity(o) }

    static let sans = Font.Design.default
    static func mono(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .system(size: size, weight: weight, design: .monospaced)
    }
}

extension Color {
    init(hex: UInt32) {
        self.init(
            .sRGB,
            red: Double((hex >> 16) & 0xff) / 255,
            green: Double((hex >> 8) & 0xff) / 255,
            blue: Double(hex & 0xff) / 255,
            opacity: 1
        )
    }
}

/// The breathing sphere — the familiar's face on the join screen.
struct BreathingSphere: View {
    var size: CGFloat
    @State private var breathe = false
    var body: some View {
        ZStack {
            Circle()
                .fill(RadialGradient(colors: [Fam.blue.opacity(0.55), .clear],
                                     center: .center, startRadius: 0, endRadius: size * 0.9))
                .frame(width: size * 1.7, height: size * 1.7)
                .blur(radius: 5)
                .opacity(breathe ? 0.82 : 0.45)
            Circle()
                .fill(RadialGradient(
                    stops: [
                        .init(color: Color(hex: 0xe2edff), location: 0.0),
                        .init(color: Color(hex: 0x7ba3ff), location: 0.24),
                        .init(color: Color(hex: 0x3568e8), location: 0.50),
                        .init(color: Color(hex: 0x123a9e), location: 0.74),
                        .init(color: Color(hex: 0x05132f), location: 1.0),
                    ],
                    center: UnitPoint(x: 0.34, y: 0.28), startRadius: 0, endRadius: size * 0.62))
                .overlay(Circle().stroke(Fam.blueSoft.opacity(0.25), lineWidth: 1))
                .shadow(color: Color(hex: 0x020a28).opacity(0.82), radius: 12, x: -6, y: -7)
            Circle()
                .fill(RadialGradient(colors: [Color.white.opacity(0.9), .clear],
                                     center: .center, startRadius: 0, endRadius: size * 0.2))
                .frame(width: size * 0.34, height: size * 0.28)
                .blur(radius: 2)
                .offset(x: -size * 0.16, y: -size * 0.2)
        }
        .frame(width: size, height: size)
        .scaleEffect(breathe ? 1.045 : 1.0)
        .onAppear {
            withAnimation(.easeInOut(duration: 3).repeatForever(autoreverses: true)) { breathe = true }
        }
    }
}

struct Panel<Content: View>: View {
    var radius: CGFloat = 24
    var fill: Double = 0.03
    @ViewBuilder var content: () -> Content
    var body: some View {
        content()
            .padding(24)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: radius).fill(Color.white.opacity(fill))
                .overlay(RoundedRectangle(cornerRadius: radius).stroke(Fam.hairline(0.07), lineWidth: 1)))
    }
}

struct MonoLabel: View {
    let text: String
    var body: some View {
        Text(text).font(Fam.mono(10.5)).tracking(1.9).foregroundStyle(Fam.labelBlue.opacity(0.6))
    }
}
