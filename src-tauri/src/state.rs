use kotoclip_core::document::DocumentSession;
use kotoclip_core::Engine;
use std::collections::HashMap;
use std::sync::{atomic::AtomicU64, Mutex};

/// 全局共享状态，在 Tauri 后端各个 Commands 之间持有并发安全的核心 Engine 实例
pub struct AppState {
    pub engine: Mutex<Engine>,
    pub sessions: Mutex<HashMap<String, DocumentSession>>,
    pub next_session_id: AtomicU64,
}
