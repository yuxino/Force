#!/usr/bin/env swift
import AppKit
import Foundation

// Size the source raster artwork without tracing, recoloring or redrawing it.
// The three siamese-*.png files are authoritative sources and are never outputs.
let assets = URL(fileURLWithPath: "assets", isDirectory: true)
let output = URL(fileURLWithPath: CommandLine.arguments.dropFirst().first ?? "assets", isDirectory: true)
try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)

func failure(_ message: String) -> NSError {
    NSError(domain: "ForceBrandAssets", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
}

func raster(_ name: String, crop: CGRect? = nil) throws -> NSImage {
    let url = assets.appendingPathComponent(name)
    guard let rep = NSBitmapImageRep(data: try Data(contentsOf: url)), let original = rep.cgImage else {
        throw failure("Cannot load \(url.path). Run from the repository root.")
    }
    let image: CGImage
    if let crop {
        let bounds = CGRect(x: 0, y: 0, width: original.width, height: original.height)
        guard bounds.contains(crop), let cropped = original.cropping(to: crop) else {
            throw failure("Clock crop is outside \(name): \(original.width) × \(original.height).")
        }
        image = cropped
    } else {
        image = original
    }
    return NSImage(cgImage: image, size: NSSize(width: image.width, height: image.height))
}

func contain(_ image: NSImage, in bounds: NSRect) {
    let scale = min(bounds.width / image.size.width, bounds.height / image.size.height)
    let size = NSSize(width: image.size.width * scale, height: image.size.height * scale)
    let destination = NSRect(x: bounds.midX - size.width / 2, y: bounds.midY - size.height / 2,
                             width: size.width, height: size.height)
    image.draw(in: destination, from: .zero, operation: .sourceOver, fraction: 1,
               respectFlipped: false, hints: [.interpolation: NSImageInterpolation.high])
}

func png(_ name: String, size: NSSize, scale: Int, draw: (NSRect) -> Void) throws {
    guard let bitmap = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: Int(size.width) * scale, pixelsHigh: Int(size.height) * scale,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true,
        isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
    ), let graphics = NSGraphicsContext(bitmapImageRep: bitmap) else {
        throw failure("Cannot create bitmap for \(name).")
    }
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = graphics
    defer { NSGraphicsContext.restoreGraphicsState() }
    graphics.imageInterpolation = .high
    graphics.cgContext.clear(CGRect(x: 0, y: 0, width: bitmap.pixelsWide, height: bitmap.pixelsHigh))
    graphics.cgContext.scaleBy(x: CGFloat(scale), y: CGFloat(scale))
    draw(NSRect(origin: .zero, size: size))
    guard let data = bitmap.representation(using: .png, properties: [:]) else {
        throw failure("Cannot encode \(name).")
    }
    try data.write(to: output.appendingPathComponent(name), options: .atomic)
}

// CGImage crops use source pixels measured from the top-left. Keep the open and
// closed-eye sources in the identical frame so blinking cannot move the cat.
let clockCrop = CGRect(x: 220, y: 86, width: 888, height: 1244)
let open = try raster("siamese-cat.png", crop: clockCrop)
let closed = try raster("siamese-blink.png", crop: clockCrop)
let portrait = try raster("siamese-portrait.png")
let clockSize = NSSize(width: 76, height: 112)
try png("cat-clock.png", size: clockSize, scale: 4) { contain(open, in: $0) }
try png("cat-blink.png", size: clockSize, scale: 4) { contain(closed, in: $0) }
try png("logo.png", size: NSSize(width: 64, height: 64), scale: 4) { contain(portrait, in: $0) }

// Preserve the existing geometric FORCE lettering; only its ink varies by theme.
let letteringSource = try String(contentsOf: assets.appendingPathComponent("wordmark-lettering.svg"), encoding: .utf8)
for (name, ink) in [("wordmark.png", "#141414"), ("wordmark-dark.png", "#FFFFFF")] {
    let source = letteringSource.replacingOccurrences(of: "<svg ", with: "<svg fill=\"\(ink)\" ")
    guard let lettering = NSImage(data: Data(source.utf8)) else {
        throw failure("Cannot render assets/wordmark-lettering.svg.")
    }
    try png(name, size: NSSize(width: 304, height: 64), scale: 3) { bounds in
        contain(portrait, in: NSRect(x: 0, y: 0, width: 64, height: 64))
        lettering.draw(in: bounds, from: .zero, operation: .sourceOver, fraction: 1,
                       respectFlipped: false, hints: [.interpolation: NSImageInterpolation.high])
    }
}
print("Generated cat-clock.png / cat-blink.png (304×448), logo.png (256×256), and wordmarks (912×192)")
