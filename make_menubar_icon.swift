#!/usr/bin/env swift
//
// Generates src/menubar-icon.png from icon.png.
//
// The app icon is a white rounded square containing the red hinomaru circle.
// That white square reads as a bright blob in the menu bar, so this strips it:
// find the red circle, crop to it, and clip to an ellipse so everything outside
// becomes transparent. The white イチ glyph inside the circle is preserved.
//
// Run after changing icon.png:  swift make_menubar_icon.swift

import AppKit
import CoreGraphics
import Foundation

let root = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
let sourceURL = root.appendingPathComponent("icon.png")
let outputURL = root.appendingPathComponent("src/menubar-icon.png")

// Menu bar icons are 18x18 points; emit @2x so Retina stays crisp.
let outputSize = 36

guard let source = CGImageSourceCreateWithURL(sourceURL as CFURL, nil),
      let image = CGImageSourceCreateImageAtIndex(source, 0, nil)
else {
    FileHandle.standardError.write("error: cannot read icon.png\n".data(using: .utf8)!)
    exit(1)
}

let width = image.width
let height = image.height

// Read the source into a known RGBA8 buffer so pixel probing is unambiguous.
var pixels = [UInt8](repeating: 0, count: width * height * 4)
guard let readContext = CGContext(
    data: &pixels,
    width: width,
    height: height,
    bitsPerComponent: 8,
    bytesPerRow: width * 4,
    space: CGColorSpaceCreateDeviceRGB(),
    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
) else {
    FileHandle.standardError.write("error: cannot create read context\n".data(using: .utf8)!)
    exit(1)
}
readContext.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))

// Locate the red circle: red-dominant, clearly not the white background.
var minX = width, minY = height, maxX = -1, maxY = -1
for y in 0..<height {
    for x in 0..<width {
        let i = (y * width + x) * 4
        let r = Int(pixels[i]), g = Int(pixels[i + 1]), b = Int(pixels[i + 2])
        let isRed = r > 100 && r > g * 2 && r > b * 2
        if isRed {
            if x < minX { minX = x }
            if x > maxX { maxX = x }
            if y < minY { minY = y }
            if y > maxY { maxY = y }
        }
    }
}

guard maxX > minX, maxY > minY else {
    FileHandle.standardError.write("error: no red circle found in icon.png\n".data(using: .utf8)!)
    exit(1)
}

// Square up the detected bounds so the ellipse clip stays a true circle.
let diameter = max(maxX - minX, maxY - minY) + 1
let centerX = (minX + maxX) / 2
let centerY = (minY + maxY) / 2
let cropOriginX = centerX - diameter / 2
let cropOriginY = centerY - diameter / 2

print("detected circle: \(diameter)px at (\(cropOriginX), \(cropOriginY))")

guard let outContext = CGContext(
    data: nil,
    width: outputSize,
    height: outputSize,
    bitsPerComponent: 8,
    bytesPerRow: 0,
    space: CGColorSpaceCreateDeviceRGB(),
    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
) else {
    FileHandle.standardError.write("error: cannot create output context\n".data(using: .utf8)!)
    exit(1)
}

outContext.interpolationQuality = .high
outContext.clear(CGRect(x: 0, y: 0, width: outputSize, height: outputSize))

// Inset by half a pixel so the clip edge antialiases instead of being cut flat.
let clip = CGRect(x: 0, y: 0, width: outputSize, height: outputSize).insetBy(dx: 0.5, dy: 0.5)
outContext.addEllipse(in: clip)
outContext.clip()

// Draw the source scaled so the detected circle exactly fills the output.
let scale = CGFloat(outputSize) / CGFloat(diameter)
outContext.draw(
    image,
    in: CGRect(
        x: -CGFloat(cropOriginX) * scale,
        y: -CGFloat(height - cropOriginY - diameter) * scale,
        width: CGFloat(width) * scale,
        height: CGFloat(height) * scale
    )
)

guard let output = outContext.makeImage(),
      let destination = CGImageDestinationCreateWithURL(
          outputURL as CFURL, "public.png" as CFString, 1, nil
      )
else {
    FileHandle.standardError.write("error: cannot encode output\n".data(using: .utf8)!)
    exit(1)
}

CGImageDestinationAddImage(destination, output, nil)
guard CGImageDestinationFinalize(destination) else {
    FileHandle.standardError.write("error: cannot write \(outputURL.path)\n".data(using: .utf8)!)
    exit(1)
}

print("wrote src/menubar-icon.png (\(outputSize)x\(outputSize), transparent outside the circle)")
