// Original vector artwork. Regenerate on macOS: swift desktop/scripts/generate-icon.swift
import AppKit
let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
let assets = root.appendingPathComponent("assets")
let iconset = root.appendingPathComponent("target/WhooshBridge.iconset")
try FileManager.default.createDirectory(at: assets, withIntermediateDirectories: true)
try FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)
// Matching editable SVG source, with the same paths as the Core Graphics renderer.
let svg = """
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" fill="none">
<title>Whoosh Bridge</title>
<defs><linearGradient id="blue" x1="180" y1="100" x2="850" y2="950" gradientUnits="userSpaceOnUse"><stop stop-color="#2585FF"/><stop offset="1" stop-color="#004DCE"/></linearGradient></defs>
<rect x="64" y="64" width="896" height="896" rx="204" fill="url(#blue)"/>
<path d="M270 674V518C270 384 378 294 512 294S754 384 754 518V674" stroke="white" stroke-width="76" stroke-linecap="round"/>
<path d="M270 674H754M646 566L754 674L646 782" stroke="#94E8FF" stroke-width="76" stroke-linecap="round" stroke-linejoin="round"/>
</svg>
"""
try svg.write(to: assets.appendingPathComponent("icon.svg"), atomically: true, encoding: .utf8)
func render(_ size: Int) -> Data {
    let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    let ctx = NSGraphicsContext(bitmapImageRep: bitmap)!.cgContext
    ctx.scaleBy(x: CGFloat(size)/1024, y: CGFloat(size)/1024)
    ctx.translateBy(x: 0, y: 1024); ctx.scaleBy(x: 1, y: -1)
    ctx.saveGState()
    ctx.addPath(CGPath(roundedRect: CGRect(x:64,y:64,width:896,height:896),cornerWidth:204,cornerHeight:204,transform:nil)); ctx.clip()
    let colors = [CGColor(red:37/255,green:133/255,blue:1,alpha:1),CGColor(red:0,green:77/255,blue:206/255,alpha:1)] as CFArray
    ctx.drawLinearGradient(CGGradient(colorsSpace:CGColorSpaceCreateDeviceRGB(),colors:colors,locations:[0,1])!,start:CGPoint(x:180,y:100),end:CGPoint(x:850,y:950),options:[.drawsBeforeStartLocation,.drawsAfterEndLocation]); ctx.restoreGState()
    ctx.setLineWidth(76);ctx.setLineCap(.round);ctx.setLineJoin(.round)
    ctx.setStrokeColor(CGColor(red:1,green:1,blue:1,alpha:1))
    ctx.move(to:CGPoint(x:270,y:674));ctx.addLine(to:CGPoint(x:270,y:518))
    ctx.addCurve(to:CGPoint(x:512,y:294),control1:CGPoint(x:270,y:384),control2:CGPoint(x:378,y:294))
    ctx.addCurve(to:CGPoint(x:754,y:518),control1:CGPoint(x:646,y:294),control2:CGPoint(x:754,y:384))
    ctx.addLine(to:CGPoint(x:754,y:674));ctx.strokePath()
    ctx.setStrokeColor(CGColor(red:148/255,green:232/255,blue:1,alpha:1))
    ctx.move(to:CGPoint(x:270,y:674));ctx.addLine(to:CGPoint(x:754,y:674));ctx.strokePath()
    ctx.move(to:CGPoint(x:646,y:566));ctx.addLine(to:CGPoint(x:754,y:674));ctx.addLine(to:CGPoint(x:646,y:782));ctx.strokePath()
    return bitmap.representation(using:.png,properties:[:])!
}
try render(1024).write(to:assets.appendingPathComponent("icon.png"))
for size in [16,32,128,256,512] {
    try render(size).write(to:iconset.appendingPathComponent("icon_\(size)x\(size).png"))
    try render(size*2).write(to:iconset.appendingPathComponent("icon_\(size)x\(size)@2x.png"))
}
let process = Process();process.executableURL=URL(fileURLWithPath:"/usr/bin/iconutil")
process.arguments=["-c","icns",iconset.path,"-o",assets.appendingPathComponent("WhooshBridge.icns").path]
try process.run();process.waitUntilExit()
if process.terminationStatus != 0 { exit(process.terminationStatus) }
print("Generated SVG, PNG and macOS ICNS assets.")
