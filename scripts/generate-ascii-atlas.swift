// Draws the atlas the ASCII pane style samples, src-tauri/crates/sikemux-gpui/assets/ascii-atlas.png.
// Its top is a strip of glyphs in the app's mono font, one per cell at twice the cell's size in
// points: twelve levels of light, two glyphs each, then the two the falling specks use. Below
// it is a moon jellyfish at rest: red is its light, green its bright white core.
//   swift scripts/generate-ascii-atlas.swift

import CoreGraphics
import CoreImage
import CoreText
import Foundation
import ImageIO
import UniformTypeIdentifiers

let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
let assets = root.appendingPathComponent("src-tauri/crates/sikemux-gpui/assets")
let fontURL = assets.appendingPathComponent("fonts/JetBrainsMonoNerdFontMono-Regular-Base.ttf")
let output = assets.appendingPathComponent("ascii-atlas.png")

let glyphs = Array(".,'`:';:o;tot%S%XS8X@8#@-·")
let cell = (width: 18, height: 28)
let fontSize: CGFloat = 20
// The jellyfish: its bell's rim centred at `rim`, `radius` texels to each side.
let sprite = (width: 256, height: 320)
let rim = CGPoint(x: 128, y: 62)
let radius: CGFloat = 64
let width = max(glyphs.count * cell.width, sprite.width)
let height = cell.height + sprite.height

func grayContext(_ w: Int, _ h: Int) -> CGContext {
    let context = CGContext(
        data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w,
        space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue)!
    context.translateBy(x: 0, y: CGFloat(h))
    context.scaleBy(x: 1, y: -1)
    context.setFillColor(gray: 0, alpha: 1)
    context.fill(CGRect(x: 0, y: 0, width: w, height: h))
    return context
}

func bytes(_ context: CGContext) -> [UInt8] {
    let pointer = context.data!.bindMemory(to: UInt8.self, capacity: context.width * context.height)
    return Array(UnsafeBufferPointer(start: pointer, count: context.width * context.height))
}

// MARK: Glyphs

let fontData = try! Data(contentsOf: fontURL)
let descriptor = (CTFontManagerCreateFontDescriptorsFromData(fontData as CFData) as! [CTFontDescriptor])[0]
let font = CTFontCreateWithFontDescriptor(descriptor, fontSize, nil)
let strip = grayContext(width, cell.height)
for (index, character) in glyphs.enumerated() {
    let text = NSAttributedString(
        string: String(character),
        attributes: [
            NSAttributedString.Key(kCTFontAttributeName as String): font,
            NSAttributedString.Key(kCTForegroundColorAttributeName as String): CGColor(gray: 1, alpha: 1),
            // A touch heavier than Regular, as a medium weight would draw.
            NSAttributedString.Key(kCTStrokeWidthAttributeName as String): -2.5,
            NSAttributedString.Key(kCTStrokeColorAttributeName as String): CGColor(gray: 1, alpha: 1),
        ])
    let line = CTLineCreateWithAttributedString(text)
    var ascent: CGFloat = 0
    var descent: CGFloat = 0
    let advance = CTLineGetTypographicBounds(line, &ascent, &descent, nil)
    strip.saveGState()
    strip.textMatrix = CGAffineTransform(scaleX: 1, y: -1)
    let middle = CGFloat(cell.height) / 2 + 1
    strip.textPosition = CGPoint(
        x: CGFloat(index * cell.width) + (CGFloat(cell.width) - advance) / 2,
        y: middle + (ascent - descent) / 2 - 1.5)
    CTLineDraw(line, strip)
    strip.restoreGState()
}
let stripBytes = bytes(strip)

// MARK: Jellyfish

var seed: UInt64 = 0x5eed_1e11
func random() -> CGFloat {
    seed = seed &* 6364136223846793005 &+ 1442695040888963407
    return CGFloat(seed >> 33) / CGFloat(1 << 31)
}

let light = grayContext(sprite.width, sprite.height)
let core = grayContext(sprite.width, sprite.height)
for context in [light, core] {
    context.setLineCap(.round)
    context.setLineJoin(.round)
    context.translateBy(x: rim.x, y: rim.y)
}
let rx = radius
let ry = radius * 0.8

func stroke(_ points: [CGPoint], from w0: CGFloat, to w1: CGFloat, light l: CGFloat, core c: CGFloat) {
    for s in stride(from: 0, to: points.count - 1, by: 1) {
        let f = CGFloat(s) / CGFloat(points.count - 1)
        let width = (w0 + (w1 - w0) * f) * radius
        for (context, value) in [(light, l * (1 - f * 0.82)), (core, c)] where value > 0 {
            context.setLineWidth(width)
            context.setStrokeColor(gray: value, alpha: 1)
            context.move(to: points[s])
            context.addLine(to: points[s + 1])
            context.strokePath()
        }
    }
}

// Tentacles from the rim and four oral arms, laid over one another rather than added, so a
// stroke's overlapping joints do not bead from the middle, hanging and gently curling.
for i in 0..<9 {
    let u = -0.92 + 1.84 * CGFloat(i) / 8
    let phase = random() * 6.28
    let sway = 0.6 + random() * 0.8
    let start = CGPoint(x: u * rx * 0.95, y: ry * 0.03 * (1 - abs(u)))
    let points = (0..<15).map { k -> CGPoint in
        let f = CGFloat(k) / 15
        let x = start.x + u * radius * 0.35 * f + sin(CGFloat(k) * 0.42 + phase) * sway * (0.25 + f) * 4
        return CGPoint(x: x, y: start.y + CGFloat(k) * 0.25 * radius)
    }
    stroke(points, from: 0.09, to: 0.03, light: 0.62, core: 0)
}
for u in [-0.3, -0.1, 0.1, 0.3] as [CGFloat] {
    let phase = random() * 6.28
    let points = (0..<10).map { k -> CGPoint in
        let f = CGFloat(k) / 10
        return CGPoint(x: u * rx + sin(CGFloat(k) * 0.6 + phase) * 6 * f, y: -ry * 0.05 + CGFloat(k) * 0.2 * radius)
    }
    stroke(points, from: 0.17, to: 0.06, light: 0.62, core: 0.12)
}

for context in [light, core] {
    context.setBlendMode(.plusLighter)
}

// The bell: a dome over a shallow concave underside, brighter towards its edge.
let bell = CGMutablePath()
bell.move(to: CGPoint(x: -rx, y: 0))
bell.addCurve(to: CGPoint(x: 0, y: -ry), control1: CGPoint(x: -rx, y: -ry * 0.58), control2: CGPoint(x: -rx * 0.56, y: -ry))
bell.addCurve(to: CGPoint(x: rx, y: 0), control1: CGPoint(x: rx * 0.56, y: -ry), control2: CGPoint(x: rx, y: -ry * 0.58))
bell.addQuadCurve(to: CGPoint(x: -rx, y: 0), control: CGPoint(x: 0, y: -ry * 0.2))
bell.closeSubpath()
let gray = CGColorSpaceCreateDeviceGray()
for (context, stops) in [(light, [0.34, 0.42, 0.66]), (core, [0.1, 0.03, 0.08])] {
    context.saveGState()
    context.addPath(bell)
    context.clip()
    let gradient = CGGradient(
        colorsSpace: gray, colors: stops.map { CGColor(gray: $0, alpha: 1) } as CFArray, locations: [0, 0.7, 1])!
    context.drawRadialGradient(
        gradient, startCenter: CGPoint(x: 0, y: -ry * 0.38), startRadius: rx * 0.1,
        endCenter: CGPoint(x: 0, y: -ry * 0.3), endRadius: rx * 1.05, options: [.drawsBeforeStartLocation, .drawsAfterEndLocation])
    context.restoreGState()
}
for (context, value) in [(light, 0.55), (core, 0.2)] {
    context.setLineWidth(max(5, radius * 0.085))
    context.setStrokeColor(gray: value, alpha: 1)
    context.addPath(bell)
    context.strokePath()
}
// The four gonads, the moon jellyfish's clover, and the bulbs along the rim.
for i in 0..<4 {
    let angle = CGFloat.pi / 4 + CGFloat(i) * CGFloat.pi / 2
    for (context, value) in [(light, 0.14), (core, 0.52)] {
        context.saveGState()
        context.translateBy(x: cos(angle) * rx * 0.26, y: -ry * 0.44 + sin(angle) * ry * 0.2)
        context.rotate(by: angle)
        context.setLineWidth(max(3, radius * 0.05))
        context.setStrokeColor(gray: value, alpha: 1)
        context.strokeEllipse(in: CGRect(x: -rx * 0.14, y: -ry * 0.12, width: rx * 0.28, height: ry * 0.24))
        context.restoreGState()
    }
}
for i in 0...8 {
    let u = -0.95 + 1.9 * CGFloat(i) / 8
    let r = max(2.5, radius * 0.034)
    let centre = CGPoint(x: u * rx, y: -ry * 0.03 * (1 - abs(u)))
    for (context, value) in [(light, 0.4), (core, 0.36)] {
        context.setFillColor(gray: value, alpha: 1)
        context.fillEllipse(in: CGRect(x: centre.x - r, y: centre.y - r, width: 2 * r, height: 2 * r))
    }
}

// Softened by about half a cell, as the light a cell samples is its average over the cell.
let images = CIContext(options: [.workingColorSpace: NSNull()])
func softened(_ context: CGContext) -> [UInt8] {
    let image = context.makeImage()!
    let extent = CGRect(x: 0, y: 0, width: image.width, height: image.height)
    let soft = CIImage(cgImage: image).clampedToExtent().applyingGaussianBlur(sigma: 1.6).cropped(to: extent)
    let out = grayContext(image.width, image.height)
    out.scaleBy(x: 1, y: -1)
    out.translateBy(x: 0, y: -CGFloat(image.height))
    out.draw(images.createCGImage(soft, from: extent)!, in: extent)
    return bytes(out)
}
let lightBytes = softened(light)
let coreBytes = softened(core)

// MARK: Atlas

var pixels = [UInt8](repeating: 0, count: width * height * 4)
for y in 0..<height {
    for x in 0..<width {
        let at = (y * width + x) * 4
        pixels[at + 3] = 255
        if y < cell.height {
            pixels[at] = stripBytes[y * width + x]
        } else if x < sprite.width {
            let from = (y - cell.height) * sprite.width + x
            pixels[at] = lightBytes[from]
            pixels[at + 1] = coreBytes[from]
        }
    }
}
let atlas = CGImage(
    width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: width * 4,
    space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.noneSkipLast.rawValue),
    provider: CGDataProvider(data: Data(pixels) as CFData)!, decode: nil, shouldInterpolate: false,
    intent: .defaultIntent)!
let destination = CGImageDestinationCreateWithURL(output as CFURL, UTType.png.identifier as CFString, 1, nil)!
CGImageDestinationAddImage(destination, atlas, nil)
CGImageDestinationFinalize(destination)
print("wrote \(output.path) \(width)x\(height)")
