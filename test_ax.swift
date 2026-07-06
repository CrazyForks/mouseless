import Cocoa
import ApplicationServices

let system = AXUIElementCreateSystemWide()
var element: AXUIElement?
// Get the element at the current mouse position
let loc = NSEvent.mouseLocation
let quartzY = NSScreen.screens.first!.frame.height - loc.y
AXUIElementCopyElementAtPosition(system, Float(loc.x), Float(quartzY), &element)

if let element = element {
    var role: CFTypeRef?
    AXUIElementCopyAttributeValue(element, "AXRole" as CFString, &role)
    print("Role: \(role ?? "nil" as CFTypeRef)")
    var title: CFTypeRef?
    AXUIElementCopyAttributeValue(element, "AXTitle" as CFString, &title)
    print("Title: \(title ?? "nil" as CFTypeRef)")
}
