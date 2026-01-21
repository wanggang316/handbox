import AppKit
import Foundation
import SwiftRs

class MouseObserverManager {
    static let shared = MouseObserverManager()
    var monitor: Any?
    
    func start(callback: @escaping @convention(c) (Double, Double) -> Void) {
        self.monitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseUp]) { _ in
            // 1. 获取全局鼠标位置（左下角原点）
            let mouseLocation = NSEvent.mouseLocation
            
            // 2. 永远获取“主显示器”（索引为 0 的屏幕）来作为 Y 轴翻转基准
            // 在多屏系统中，screens[0] 始终是原点所在的屏幕
            if let primaryScreen = NSScreen.screens.first {
                let primaryScreenHeight = primaryScreen.frame.height
                
                // 3. 计算全局坐标
                // X轴无需特殊处理，macOS 全局坐标即为虚拟桌面坐标
                let x = Double(mouseLocation.x)
                
                // Y轴翻转：主屏幕高度 - 全局Y
                // 这样得到的坐标，(0,0) 就是主屏幕的左上角，完美匹配 Tauri
                let y = Double(primaryScreenHeight - mouseLocation.y)
                
                callback(x, y)
            }
        }
    }
}

@_cdecl("start_mouse_observer")
public func start_mouse_observer(callbackPtr: UnsafeRawPointer) {
    typealias CallbackType = @convention(c) (Double, Double) -> Void
    let callback = unsafeBitCast(callbackPtr, to: CallbackType.self)
    MouseObserverManager.shared.start(callback: callback)
}