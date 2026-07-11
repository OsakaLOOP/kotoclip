use kotoclip_core::Engine;
use std::sync::Mutex;

/// 全局共享状态，在 Tauri 后端各个 Commands 之间持有并发安全的核心 Engine 实例
pub struct AppState {
    pub engine: Mutex<Engine>,
}
