// Render the supplied vector geometry as native app artwork.
import AppKit
let size = 1024
let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size,
    bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
    colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
NSColor(srgbRed: 246/255, green: 243/255, blue: 234/255, alpha: 1).setFill()
NSRect(x: 0, y: 0, width: size, height: size).fill()
let transform = AffineTransform(translationByX: 128, byY: 128)
let path = NSBezierPath()
path.windingRule = .evenOdd
// SVG uses a downward y axis. Mirror its coordinates into AppKit's canvas.
let scale: CGFloat = 16
path.appendRect(NSRect(x: 4*scale, y: 16*scale, width: 28*scale, height: 28*scale))
path.appendRect(NSRect(x: 10*scale, y: 22*scale, width: 16*scale, height: 16*scale))
path.transform(using: transform)
NSColor(srgbRed: 36/255, green: 40/255, blue: 32/255, alpha: 1).setFill()
path.fill()
NSRect(x: 128+20*scale, y: 128+4*scale, width: 24*scale, height: 24*scale).fill()
NSGraphicsContext.restoreGraphicsState()
try bitmap.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: CommandLine.arguments[1]))
