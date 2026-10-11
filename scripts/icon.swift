import AppKit
let output = CommandLine.arguments[1]
let image = NSImage(size: NSSize(width: 1024, height: 1024))
image.lockFocus()
NSColor(calibratedWhite: 0.06, alpha: 1).setFill()
NSBezierPath(roundedRect: NSRect(x: 40, y: 40, width: 944, height: 944), xRadius: 208, yRadius: 208).fill()
let attributes: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: 600, weight: .bold), .foregroundColor: NSColor(calibratedWhite: 0.94, alpha: 1)]
let letter = "F" as NSString
let size = letter.size(withAttributes: attributes)
letter.draw(at: NSPoint(x: (1024-size.width)/2 - 12, y: (1024-size.height)/2 + 18), withAttributes: attributes)
NSColor(calibratedWhite: 0.06, alpha: 1).setFill()
NSBezierPath(roundedRect: NSRect(x: 356, y: 840, width: 312, height: 180), xRadius: 30, yRadius: 30).fill()
image.unlockFocus()
let bitmap = NSBitmapImageRep(data: image.tiffRepresentation!)!
try bitmap.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: output))
