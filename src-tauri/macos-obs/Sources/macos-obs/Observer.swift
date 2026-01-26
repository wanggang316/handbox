import AppKit
import Foundation
import SwiftRs

class MouseObserverManager {
    static let shared = MouseObserverManager()
    var monitor: Any?
    
    // 修改回调定义：使用 UnsafePointer<Int8> 代替 SRString
    func start(callback: @escaping @convention(c) (Double, Double, UnsafePointer<Int8>, UnsafePointer<Int8>, Int32) -> Void) {
        if self.monitor != nil { return }

        self.monitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseUp]) { _ in
            let mouseLocation = NSEvent.mouseLocation
            let frontApp = NSWorkspace.shared.frontmostApplication
            
            // 准备数据
            let appName = frontApp?.localizedName ?? "Unknown"
            let bundleId = frontApp?.bundleIdentifier ?? "unknown.app"
            let pid = frontApp?.processIdentifier ?? 0
            
            if let primaryScreen = NSScreen.screens.first {
                let screenHeight = primaryScreen.frame.height
                let x = Double(mouseLocation.x)
                let y = Double(screenHeight - mouseLocation.y)
                
                // 将 Swift String 转换为临时 C 字符串指针
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
    // 这里的签名必须与上面完全一致
    typealias CallbackType = @convention(c) (Double, Double, UnsafePointer<Int8>, UnsafePointer<Int8>, Int32) -> Void
    let callback = unsafeBitCast(callbackPtr, to: CallbackType.self)
    
    DispatchQueue.main.async {
        MouseObserverManager.shared.start(callback: callback)
    }
}