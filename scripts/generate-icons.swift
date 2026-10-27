#!/usr/bin/env swift
import AppKit
import Foundation

// Compose every platform icon from the source portrait, never a traced copy.
let output = URL(fileURLWithPath: CommandLine.arguments.dropFirst().first ?? "assets", isDirectory: true)
let portraitURL = URL(fileURLWithPath: "assets/siamese-portrait.png")
guard let rep = NSBitmapImageRep(data: try Data(contentsOf: portraitURL)), let pixels = rep.cgImage else {
    throw NSError(domain: "ForceIcons", code: 4, userInfo: [NSLocalizedDescriptionKey: "Cannot load assets/siamese-portrait.png. Run from the repository root."])
}
let portrait = NSImage(cgImage: pixels, size: NSSize(width: pixels.width, height: pixels.height))
let manager = FileManager.default
try manager.createDirectory(at: output, withIntermediateDirectories: true)
let iconset = output.appendingPathComponent("Force.iconset", isDirectory: true)
guard !manager.fileExists(atPath: iconset.path) else {
    throw NSError(domain: "ForceIcons", code: 5, userInfo: [NSLocalizedDescriptionKey: "\(iconset.path) already exists; refusing to remove an existing iconset."])
}
try manager.createDirectory(at: iconset, withIntermediateDirectories: false)
defer { try? manager.removeItem(at: iconset) }

func iconPNG(size: Int) throws -> Data {
    guard let bitmap = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true,
        isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
    ), let graphics = NSGraphicsContext(bitmapImageRep: bitmap) else {
        throw NSError(domain: "ForceIcons", code: 1)
    }
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = graphics
    defer { NSGraphicsContext.restoreGraphicsState() }
    let bounds = NSRect(x: 0, y: 0, width: size, height: size)
    graphics.cgContext.clear(bounds)
    graphics.imageInterpolation = .high
    let scale = CGFloat(size) / 1024
    graphics.cgContext.scaleBy(x: scale, y: scale)

    // Retain the existing 896-point white tile, its 64-point inset and radius.
    NSColor.white.setFill()
    NSBezierPath(roundedRect: NSRect(x: 64, y: 64, width: 896, height: 896),
                 xRadius: 192, yRadius: 192).fill()
    let portraitBounds = NSRect(x: 96, y: 96, width: 832, height: 832)
    let factor = min(portraitBounds.width / portrait.size.width, portraitBounds.height / portrait.size.height)
    let portraitSize = NSSize(width: portrait.size.width * factor, height: portrait.size.height * factor)
    let destination = NSRect(x: portraitBounds.midX - portraitSize.width / 2,
                             y: portraitBounds.midY - portraitSize.height / 2,
                             width: portraitSize.width, height: portraitSize.height)
    portrait.draw(in: destination, from: .zero, operation: .sourceOver, fraction: 1,
                  respectFlipped: false, hints: [.interpolation: NSImageInterpolation.high])
    guard let data = bitmap.representation(using: .png, properties: [:]) else {
        throw NSError(domain: "ForceIcons", code: 2)
    }
    return data
}

for points in [16, 32, 128, 256, 512] {
    for scale in [1, 2] {
        let suffix = scale == 2 ? "@2x" : ""
        try iconPNG(size: points * scale).write(to: iconset.appendingPathComponent("icon_\(points)x\(points)\(suffix).png"))
    }
}
try iconPNG(size: 1024).write(to: output.appendingPathComponent("icon.png"))

let iconutil = Process()
iconutil.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
iconutil.arguments = ["-c", "icns", "-o", output.appendingPathComponent("Force.icns").path, iconset.path]
try iconutil.run()
iconutil.waitUntilExit()
guard iconutil.terminationStatus == 0 else { throw NSError(domain: "ForceIcons", code: 3) }

// Keep native small sizes crisp in Explorer, the taskbar and window chrome.
let windowsImages = try [16, 32, 48, 256].map { (size: $0, png: try iconPNG(size: $0)) }
var ico = Data([0, 0, 1, 0])
func appendUInt16(_ value: UInt16) {
    let number = value.littleEndian
    withUnsafeBytes(of: number) { ico.append(contentsOf: $0) }
}
func appendUInt32(_ value: UInt32) {
    let number = value.littleEndian
    withUnsafeBytes(of: number) { ico.append(contentsOf: $0) }
}
appendUInt16(UInt16(windowsImages.count))
var offset = UInt32(6 + windowsImages.count * 16)
for image in windowsImages {
    let dimension = image.size == 256 ? UInt8(0) : UInt8(image.size)
    ico.append(contentsOf: [dimension, dimension, 0, 0])
    appendUInt16(1)
    appendUInt16(32)
    appendUInt32(UInt32(image.png.count))
    appendUInt32(offset)
    offset += UInt32(image.png.count)
}
for image in windowsImages { ico.append(image.png) }
try ico.write(to: output.appendingPathComponent("Force.ico"))
print("Generated portrait-based icon.png, Force.icns and Force.ico (16/32/48/256 px)")
