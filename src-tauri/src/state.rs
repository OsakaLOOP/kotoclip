use kotoclip_core::cache::AnalysisCache;
use kotoclip_core::document::DocumentSession;
use kotoclip_core::library::ReaderLibrary;
use kotoclip_core::{DictionaryService, Engine};
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Condvar, Mutex, MutexGuard,
};

/// 标准互斥锁在持锁任务 panic 后会保留 poison 标志；这里恢复 guard 并清除标志，
/// 让 RAII 释放语义不因一次任务失败演变为进程生命周期内的永久不可用。
pub struct RecoveringMutex<T> {
    inner: Mutex<T>,
    label: &'static str,
}

impl<T> RecoveringMutex<T> {
    pub fn new(value: T, label: &'static str) -> Self {
        Self {
            inner: Mutex::new(value),
            label,
        }
    }

    pub fn lock(&self) -> Result<MutexGuard<'_, T>, String> {
        match self.inner.lock() {
            Ok(guard) => Ok(guard),
            Err(poisoned) => {
                eprintln!(
                    "[后端恢复] {} 的持锁任务异常结束，正在恢复互斥状态",
                    self.label
                );
                let guard = poisoned.into_inner();
                self.inner.clear_poison();
                Ok(guard)
            }
        }
    }

    fn clear_poison(&self) {
        self.inner.clear_poison();
    }

    fn label(&self) -> &'static str {
        self.label
    }
}

pub struct AnalysisCancellationRegistry {
    requests: RecoveringMutex<HashMap<String, Arc<AtomicBool>>>,
}

impl AnalysisCancellationRegistry {
    pub fn new() -> Self {
        Self {
            requests: RecoveringMutex::new(HashMap::new(), "analysis cancellations"),
        }
    }

    pub fn begin(&self, request_id: String) -> AnalysisCancellationGuard<'_> {
        let mut requests = self.requests.lock().expect("取消注册表应始终可恢复");
        let cancelled = Arc::clone(
            requests
                .entry(request_id.clone())
                .or_insert_with(|| Arc::new(AtomicBool::new(false))),
        );
        AnalysisCancellationGuard {
            registry: self,
            request_id,
            cancelled,
        }
    }

    pub fn cancel(&self, request_id: &str) -> bool {
        let mut requests = self.requests.lock().expect("取消注册表应始终可恢复");
        if requests.len() >= 128 {
            requests.retain(|_, flag| Arc::strong_count(flag) > 1);
        }
        let was_running = requests.contains_key(request_id);
        requests
            .entry(request_id.to_string())
            .or_insert_with(|| Arc::new(AtomicBool::new(true)))
            .store(true, Ordering::Release);
        was_running
    }

    fn finish(&self, request_id: &str, cancelled: &Arc<AtomicBool>) {
        let mut requests = self.requests.lock().expect("取消注册表应始终可恢复");
        if requests
            .get(request_id)
            .is_some_and(|current| Arc::ptr_eq(current, cancelled))
        {
            requests.remove(request_id);
        }
    }
}

pub struct AnalysisCancellationGuard<'a> {
    registry: &'a AnalysisCancellationRegistry,
    request_id: String,
    cancelled: Arc<AtomicBool>,
}

impl AnalysisCancellationGuard<'_> {
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

impl Drop for AnalysisCancellationGuard<'_> {
    fn drop(&mut self) {
        self.registry.finish(&self.request_id, &self.cancelled);
    }
}

enum ResourceState<T> {
    Pending,
    Ready(T),
    Failed(String),
}

struct ResourceInner<T> {
    state: RecoveringMutex<ResourceState<T>>,
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
                state: RecoveringMutex::new(ResourceState::Pending, std::any::type_name::<T>()),
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
        let mut guard = self.inner.state.lock()?;
        loop {
            match &*guard {
                ResourceState::Pending => {
                    guard = match self.inner.ready.wait(guard) {
                        Ok(guard) => guard,
                        Err(poisoned) => {
                            eprintln!(
                                "[后端恢复] {} 在等待初始化时发生持锁任务异常，正在恢复互斥状态",
                                self.inner.state.label(),
                            );
                            let guard = poisoned.into_inner();
                            self.inner.state.clear_poison();
                            guard
                        }
                    };
                }
                ResourceState::Ready(_) => return Ok(ResourceGuard { guard }),
                ResourceState::Failed(error) => return Err(error.clone()),
            }
        }
    }

    pub fn status(&self) -> Result<bool, String> {
        let guard = self.inner.state.lock()?;
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
    /// 后台整词预取使用独立 SQLite 连接，不能阻塞前台悬浮词条。
    pub dictionary_background: LazyResource<DictionaryService>,
    pub sessions: RecoveringMutex<HashMap<String, DocumentSession>>,
    pub analysis_cancellations: AnalysisCancellationRegistry,
    pub next_session_id: AtomicU64,
    pub analysis_cache: LazyResource<AnalysisCache>,
    pub library: ReaderLibrary,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn recovering_mutex_releases_guard_and_clears_poison_after_panic() {
        let mutex = Arc::new(RecoveringMutex::new(7_u8, "test mutex"));
        let panicking_mutex = Arc::clone(&mutex);
        let result = std::thread::spawn(move || {
            let _guard = panicking_mutex.lock().expect("首次获取锁应成功");
            panic!("模拟持锁任务失败");
        })
        .join();

        assert!(result.is_err());
        let recovered_guard = mutex.lock().expect("poison 后应恢复 guard");
        assert_eq!(*recovered_guard, 7);
        drop(recovered_guard);

        let (sender, receiver) = mpsc::channel();
        let next_mutex = Arc::clone(&mutex);
        let next_task = std::thread::spawn(move || {
            let guard = next_mutex.lock().expect("poison 标志应已清除");
            sender.send(*guard).expect("应返回后续加锁结果");
        });
        assert_eq!(
            receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("恢复 guard 释放后，其他线程应能取得锁"),
            7
        );
        next_task.join().expect("后续加锁任务应正常结束");
    }

    #[test]
    fn analysis_cancellation_supports_running_and_early_requests() {
        let registry = AnalysisCancellationRegistry::new();
        let running = registry.begin("running".to_string());
        assert!(!running.is_cancelled());
        assert!(registry.cancel("running"));
        assert!(running.is_cancelled());
        drop(running);

        assert!(!registry.cancel("early"));
        let early = registry.begin("early".to_string());
        assert!(early.is_cancelled());
        drop(early);

        assert!(registry
            .requests
            .lock()
            .expect("取消注册表应可读取")
            .is_empty());
    }
}
