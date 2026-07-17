use kotoclip_core::cache::AnalysisCache;
use kotoclip_core::document::DocumentSession;
use kotoclip_core::{DictionaryService, Engine};
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{atomic::AtomicU64, Arc, Condvar, Mutex, MutexGuard};

enum ResourceState<T> {
    Pending,
    Ready(T),
    Failed(String),
}

struct ResourceInner<T> {
    state: Mutex<ResourceState<T>>,
    ready: Condvar,
}

/// 延迟初始化的共享资源。启动线程负责填充资源，IPC 命令在真正需要时等待结果。
pub struct LazyResource<T> {
    inner: Arc<ResourceInner<T>>,
}

pub struct ResourceGuard<'a, T> {
    guard: MutexGuard<'a, ResourceState<T>>,
}

impl<T> Clone for LazyResource<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> LazyResource<T> {
    pub fn pending() -> Self {
        Self {
            inner: Arc::new(ResourceInner {
                state: Mutex::new(ResourceState::Pending),
                ready: Condvar::new(),
            }),
        }
    }

    pub fn initialize(&self, result: Result<T, String>) {
        let Ok(mut guard) = self.inner.state.lock() else {
            return;
        };
        if matches!(*guard, ResourceState::Pending) {
            *guard = match result {
                Ok(value) => ResourceState::Ready(value),
                Err(error) => ResourceState::Failed(error),
            };
            self.inner.ready.notify_all();
        }
    }

    pub fn lock(&self) -> Result<ResourceGuard<'_, T>, String> {
        let mut guard = self.inner.state.lock().map_err(|error| error.to_string())?;
        loop {
            match &*guard {
                ResourceState::Pending => {
                    guard = self
                        .inner
                        .ready
                        .wait(guard)
                        .map_err(|error| error.to_string())?;
                }
                ResourceState::Ready(_) => return Ok(ResourceGuard { guard }),
                ResourceState::Failed(error) => return Err(error.clone()),
            }
        }
    }

    pub fn status(&self) -> Result<bool, String> {
        let guard = self.inner.state.lock().map_err(|error| error.to_string())?;
        match &*guard {
            ResourceState::Pending => Ok(false),
            ResourceState::Ready(_) => Ok(true),
            ResourceState::Failed(error) => Err(error.clone()),
        }
    }
}

impl<T> Deref for ResourceGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match &*self.guard {
            ResourceState::Ready(value) => value,
            ResourceState::Pending | ResourceState::Failed(_) => {
                unreachable!("资源锁只会在初始化完成后返回")
            }
        }
    }
}

/// 全局共享状态，在 Tauri 后端各个 Commands 之间持有并发安全的核心 Engine 实例
pub struct AppState {
    pub engine: LazyResource<Engine>,
    pub dictionary: LazyResource<DictionaryService>,
    pub sessions: Mutex<HashMap<String, DocumentSession>>,
    pub next_session_id: AtomicU64,
    pub analysis_cache: LazyResource<AnalysisCache>,
}
