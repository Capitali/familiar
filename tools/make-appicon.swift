// make-appicon.swift — render the app icon: the glassy blue marble on an opaque navy field.
// App Store icons must be fully opaque (no alpha), and iOS applies the rounded-rect mask itself,
// so this fills the whole square. Mirrors the Rust marble_icon() / packaging/make-icon.swift palette.
//
// Usage: swift tools/make-appicon.swift <output.png> [size]

import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

func renderIcon(_ size: Int) -> CGImage? {
    let s = CGFloat(size)
    let space = CGColorSpaceCreateDeviceRGB()
    guard let ctx = CGContext(
        data: nil, width: size, height: size, bitsPerComponent: 8, bytesPerRow: 0,
        space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
    ) else { return nil }

    // Opaque background (no alpha in app icons): deep navy.
    ctx.setFillColor(CGColor(red: 8 / 255, green: 22 / 255, blue: 55 / 255, alpha: 1))
    ctx.fill(CGRect(x: 0, y: 0, width: s, height: s))

    let center = CGPoint(x: s / 2, y: s / 2)
    let radius = s / 2 * 0.78
    let circle = CGRect(x: center.x - radius, y: center.y - radius, width: radius * 2, height: radius * 2)

    ctx.saveGState()
    ctx.addEllipse(in: circle)
    ctx.clip()
    // Base glass: bright core (120,185,255) → deep rim (18,64,150).
    let base = CGGradient(colorsSpace: space, colors: [
        CGColor(red: 120 / 255, green: 185 / 255, blue: 255 / 255, alpha: 1),
        CGColor(red: 18 / 255, green: 64 / 255, blue: 150 / 255, alpha: 1),
    ] as CFArray, locations: [0, 1])!
    ctx.drawRadialGradient(base, startCenter: center, startRadius: 0, endCenter: center, endRadius: radius, options: [])
    // Specular highlight, up-left (CG y points up).
    let hc = CGPoint(x: center.x - radius * 0.35, y: center.y + radius * 0.35)
    let spec = CGGradient(colorsSpace: space, colors: [
        CGColor(red: 1, green: 1, blue: 1, alpha: 0.9),
        CGColor(red: 1, green: 1, blue: 1, alpha: 0),
    ] as CFArray, locations: [0, 1])!
    ctx.drawRadialGradient(spec, startCenter: hc, startRadius: 0, endCenter: hc, endRadius: radius * 0.6, options: [])
    ctx.restoreGState()

    // Soft darker rim for definition.
    ctx.setStrokeColor(CGColor(red: 10 / 255, green: 30 / 255, blue: 80 / 255, alpha: 0.5))
    ctx.setLineWidth(max(1, s * 0.012))
    ctx.addEllipse(in: circle.insetBy(dx: s * 0.01, dy: s * 0.01))
    ctx.strokePath()

    return ctx.makeImage()
}

func writePNG(_ image: CGImage, to path: String) -> Bool {
    let url = URL(fileURLWithPath: path) as CFURL
    guard let dest = CGImageDestinationCreateWithURL(url, UTType.png.identifier as CFString, 1, nil) else { return false }
    CGImageDestinationAddImage(dest, image, nil)
    return CGImageDestinationFinalize(dest)
}

let args = CommandLine.arguments
guard args.count >= 2 else {
    FileHandle.standardError.write(Data("usage: make-appicon <output.png> [size]\n".utf8)); exit(2)
}
let size = args.count >= 3 ? Int(args[2]) ?? 1024 : 1024
guard let img = renderIcon(size), writePNG(img, to: args[1]) else {
    FileHandle.standardError.write(Data("make-appicon: render/write failed\n".utf8)); exit(3)
}
print("wrote \(args[1]) (\(size)px)")
