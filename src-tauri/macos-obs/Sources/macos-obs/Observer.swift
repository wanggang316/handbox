import AppKit
import Foundation
import SwiftRs

class MouseObserverManager {
    static let shared = MouseObserverManager()
    var monitor: Any?
    
    func start(callback: @escaping @convention(c) (Double, Double, UnsafePointer<Int8>, UnsafePointer<Int8>, Int32) -> Void) {
        if self.monitor != nil { return }

        self.monitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseUp]) { _ in
            let mouseLocation = NSEvent.mouseLocation
            let frontApp = NSWorkspace.shared.frontmostApplication
            
            let appName = frontApp?.localizedName ?? "Unknown"
            let bundleId = frontApp?.bundleIdentifier ?? "unknown.app"
            let pid = frontApp?.processIdentifier ?? 0
            
            if let primaryScreen = NSScreen.screens.first {
                let screenHeight = primaryScreen.frame.height
                let x = Double(mouseLocation.x)
                let y = Double(screenHeight - mouseLocation.y)
                
                // These C string pointers are only valid inside withCString; the callback must copy.
                appName.withCString { namePtr in
                    bundleId.withCString { bidPtr in
                        callback(x, y, namePtr, bidPtr, Int32(pid))
                    }
                }
            }
        }
    }
}

@_cdecl("start_mouse_observer")
public func start_mouse_observer(callbackPtr: UnsafeRawPointer) {
    // Must match MouseObserverManager.start's callback signature exactly — unsafeBitCast checks nothing.
    typealias CallbackType = @convention(c) (Double, Double, UnsafePointer<Int8>, UnsafePointer<Int8>, Int32) -> Void
    let callback = unsafeBitCast(callbackPtr, to: CallbackType.self)
    
    DispatchQueue.main.async {
        MouseObserverManager.shared.start(callback: callback)
    }
}