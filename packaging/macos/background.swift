import AppKit
import Foundation

let arguments = CommandLine.arguments
guard arguments.count == 2 else {
    fputs("usage: background.swift <background.tiff>\n", stderr)
    exit(2)
}

let output_url = URL(fileURLWithPath: arguments[1])
let canvas_size = NSSize(width: 640, height: 380)

guard let bitmap = NSBitmapImageRep(
    bitmapDataPlanes: nil,
    pixelsWide: Int(canvas_size.width),
    pixelsHigh: Int(canvas_size.height),
    bitsPerSample: 8,
    samplesPerPixel: 4,
    hasAlpha: true,
    isPlanar: false,
    colorSpaceName: .deviceRGB,
    bytesPerRow: 0,
    bitsPerPixel: 0
), let context = NSGraphicsContext(bitmapImageRep: bitmap) else {
    fputs("failed to create DMG background\n", stderr)
    exit(1)
}

NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = context

NSColor(calibratedWhite: 0.96, alpha: 1).setFill()
NSBezierPath(rect: NSRect(origin: .zero, size: canvas_size)).fill()

let arrow = NSBezierPath()
arrow.lineWidth = 8
arrow.lineCapStyle = .round
arrow.lineJoinStyle = .round
arrow.move(to: NSPoint(x: 260, y: 155))
arrow.line(to: NSPoint(x: 380, y: 155))
arrow.move(to: NSPoint(x: 359, y: 176))
arrow.line(to: NSPoint(x: 380, y: 155))
arrow.line(to: NSPoint(x: 359, y: 134))
NSColor(calibratedWhite: 0.52, alpha: 1).setStroke()
arrow.stroke()

NSGraphicsContext.restoreGraphicsState()

guard let tiff_data = bitmap.tiffRepresentation else {
    fputs("failed to encode DMG background\n", stderr)
    exit(1)
}

do {
    try tiff_data.write(to: output_url, options: .atomic)
} catch {
    fputs("failed to write DMG background: \(error)\n", stderr)
    exit(1)
}
