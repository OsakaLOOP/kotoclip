//! 本机模型进程管理。请求串行复用模型，取消和超时释放对应进程。
use kotoclip_nlp::external::SourceArtifact;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{atomic::{AtomicU64, Ordering}, mpsc::{self, Receiver}, Arc, Mutex},
    time::{Duration, Instant},
};

const PROTOCOL: &str = "kotoclip.provider-process.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub python: PathBuf,
    pub model: String,
    pub enabled: bool,
    pub timeout_seconds: u64,
    #[serde(default)]
    pub dictionary: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSettings {
    pub ginza: ProviderConfig,
    #[serde(default = "default_analysis_timing_enabled")]
    pub analysis_timing_enabled: bool,
}

fn default_analysis_timing_enabled() -> bool { true }

impl ProviderSettings {
    pub fn development(root: &Path) -> Self {
        Self {
            ginza: ProviderConfig { python: root.join("experiments/ginza311/Scripts/python.exe"), model: "ja_ginza".into(), enabled: true, timeout_seconds: 120, dictionary: PathBuf::new() },
            analysis_timing_enabled: true,
        }
    }

    /// 便携包默认值：随程序分发的 Python 运行时位于 `python` 目录。
    pub fn portable(root: &Path) -> Self {
        let mut settings = Self::development(root);
        for bundled in [root.join("python/python.exe"), root.join("python/Scripts/python.exe")] {
            if bundled.is_file() {
                settings.ginza.python = bundled;
                break;
            }
        }
        settings
    }
}

struct Worker {
    child: Child,
    input: ChildStdin,
    output: Receiver<Result<Value, String>>,
    manifest: Value,
    identity: String,
}

fn worker_identity(id: &str, config: &ProviderConfig, script: &Path) -> String {
    serde_json::to_string(&(PROTOCOL, id, &config.python, &config.model, &config.dictionary, script))
        .expect("provider identity must be serializable")
}

impl Worker {
    fn start(id: &str, config: &ProviderConfig, script: &Path, log: &Path, cancel: &AtomicU64, generation: u64) -> Result<Self, String> {
        let identity = worker_identity(id, config, script);
        let mut command = Command::new(&config.python);
        command.args(["-X", "utf8", "-u"]).arg(script)
            .args(["--provider", id, "--model", &config.model])
            .stdin(Stdio::piped()).stdout(Stdio::piped())
            .stderr(Stdio::from(File::create(log).map_err(|e| format!("无法创建来源日志：{e}"))?));
        if !config.dictionary.as_os_str().is_empty() { command.arg("--dictionary").arg(&config.dictionary); }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().map_err(|e| format!("无法启动 {}：{e}", config.python.display()))?;
        let input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let value = line.map_err(|e| e.to_string()).and_then(|line| serde_json::from_str(&line).map_err(|e| format!("来源消息解析失败：{e}")));
                if sender.send(value).is_err() { break; }
            }
        });
        let mut worker = Self { child, input, output: receiver, manifest: Value::Null, identity };
        let ready = worker.receive(config.timeout_seconds, cancel, generation)?;
        if ready["event"] != "ready" { return Err(ready["error"].as_str().unwrap_or("来源初始化失败").into()); }
        worker.manifest = ready["manifest"].clone();
        Ok(worker)
    }

    fn receive(&self, timeout: u64, cancel: &AtomicU64, generation: u64) -> Result<Value, String> {
        let deadline = Instant::now() + Duration::from_secs(timeout);
        loop {
            if cancel.load(Ordering::Relaxed) != generation { return Err("来源分析已取消".into()); }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() { return Err(format!("来源分析超过 {timeout} 秒")); }
            match self.output.recv_timeout(remaining.min(Duration::from_millis(100))) {
                Ok(value) => {
                    let value = value?;
                    if value["protocol"] != PROTOCOL { return Err("来源进程协议版本不匹配".into()); }
                    return Ok(value);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {},
                Err(mpsc::RecvTimeoutError::Disconnected) => return Err("来源进程已退出，请重试并检查日志".into()),
            }
        }
    }

    fn analyze(&mut self, text: &str, request_id: &str, timeout: u64, cancel: &AtomicU64, generation: u64) -> Result<SourceArtifact, String> {
        let message = json!({"protocol": PROTOCOL, "command": "analyze", "request_id": request_id, "text": text});
        writeln!(self.input, "{message}").and_then(|_| self.input.flush()).map_err(|e| format!("来源请求发送失败：{e}"))?;
        let value = self.receive(timeout, cancel, generation)?;
        if value["request_id"] != request_id { return Err("来源响应请求身份不匹配".into()); }
        if value["event"] != "result" { return Err(value["error"].as_str().unwrap_or("来源分析失败").into()); }
        let artifact: SourceArtifact = serde_json::from_value(value["result"].clone()).map_err(|e| format!("来源结果协议不匹配：{e}"))?;
        artifact.validate(text)?;
        Ok(artifact)
    }

    fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct ProviderManager {
    config_path: PathBuf,
    script: PathBuf,
    defaults: ProviderSettings,
    workers: HashMap<String, Arc<Mutex<Worker>>>,
    pub cancellation: Arc<AtomicU64>,
    sequence: u64,
    cache: Mutex<crate::analysis_cache::AnalysisCache<SourceArtifact>>,
}

impl ProviderManager {
    pub fn new(config_path: PathBuf, script: PathBuf, defaults: ProviderSettings) -> Self {
        Self { config_path, script, defaults, workers: HashMap::new(), cancellation: Arc::new(AtomicU64::new(0)), sequence: 0, cache: Mutex::new(crate::analysis_cache::AnalysisCache::new(32 * 1024 * 1024)) }
    }

    pub fn settings(&self) -> Result<ProviderSettings, String> {
        match fs::read_to_string(&self.config_path) {
            Ok(contents) => serde_json::from_str(&contents).map_err(|e| format!("来源配置无效：{e}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(self.defaults.clone()),
            Err(e) => Err(format!("无法读取来源配置：{e}")),
        }
    }

    pub fn analysis_timing_enabled(&self) -> Result<bool, String> {
        Ok(self.settings()?.analysis_timing_enabled)
    }

    pub fn cache_identity(&self) -> Result<String, String> {
        let mut value = serde_json::to_value(self.settings()?).map_err(|error| error.to_string())?;
        value.as_object_mut().map(|object| object.remove("analysis_timing_enabled"));
        serde_json::to_string(&value).map_err(|error| error.to_string())
    }

    pub fn configure(&mut self, settings: ProviderSettings) -> Result<Value, String> {
        for (id, config) in [("ginza", &settings.ginza)] {
            if config.timeout_seconds == 0 || config.model.trim().is_empty() { return Err(format!("{id} 的模型和超时设置无效")); }
        }
        let previous = self.settings().ok();
        fs::create_dir_all(self.config_path.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::write(&self.config_path, serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let new_identity = worker_identity("ginza", &settings.ginza, &self.script);
        let keep_worker = settings.ginza.enabled && self.workers.get("ginza").map(|worker| {
            let mut worker = worker.lock().unwrap();
            worker.identity == new_identity && worker.is_alive()
        }).unwrap_or(false);
        if !keep_worker { self.workers.remove("ginza"); }
        let previous_config = previous.as_ref().and_then(|value| serde_json::to_string(&value.ginza).ok());
        let new_config = serde_json::to_string(&settings.ginza).map_err(|error| error.to_string())?;
        if previous_config.as_deref() != Some(new_config.as_str()) { self.cache.lock().unwrap().clear(); }
        self.status()
    }

    pub fn set_analysis_timing(&self, enabled: bool) -> Result<Value, String> {
        let mut settings = self.settings()?;
        settings.analysis_timing_enabled = enabled;
        fs::create_dir_all(self.config_path.parent().unwrap()).map_err(|error| error.to_string())?;
        fs::write(&self.config_path, serde_json::to_vec_pretty(&settings).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
        Ok(json!({"analysis_timing_enabled": enabled}))
    }

    pub fn status(&self) -> Result<Value, String> {
        let settings = self.settings()?;
        let providers: Vec<Value> = [("ginza", &settings.ginza)].into_iter().map(|(id, config)| {
            let identity = worker_identity(id, config, &self.script);
            let state = self.workers.get(id).map(|worker| {
                let mut worker = worker.lock().unwrap();
                if worker.identity == identity && worker.is_alive() {
                    (Some(worker.child.id()), Some(worker.manifest.clone()))
                } else {
                    (None, None)
                }
            }).unwrap_or((None, None));
            let loaded = state.0.is_some();
            json!({"id": id, "configured": config.python.is_file() && self.script.is_file(), "available": loaded, "enabled": config.enabled,
                "loaded": loaded, "pid": state.0, "manifest": state.1})
        }).collect();
        Ok(json!({"settings": settings, "providers": providers, "config_path": self.config_path, "script": self.script}))
    }

    fn ensure_worker(&mut self, id: &str, config: &ProviderConfig, generation: u64) -> Result<Arc<Mutex<Worker>>, String> {
        if self.cancellation.load(Ordering::Relaxed) != generation { return Err("来源初始化已取消".into()); }
        let identity = worker_identity(id, config, &self.script);
        if let Some(existing) = self.workers.get(id).cloned() {
            let reusable = {
                let mut worker = existing.lock().unwrap();
                worker.identity == identity && worker.is_alive()
            };
            if reusable { return Ok(existing); }
            self.workers.remove(id);
        }
        let log_dir = self.config_path.parent().unwrap().join("provider-logs");
        fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;
        let worker = Worker::start(id, config, &self.script, &log_dir.join(format!("{id}.log")), &self.cancellation, generation)
            .map_err(|error| format!("{id} 初始化失败：{error}"))?;
        let worker = Arc::new(Mutex::new(worker));
        self.workers.insert(id.into(), worker.clone());
        Ok(worker)
    }

    pub fn check(&mut self, generation: u64) -> Result<Value, String> {
        let settings = self.settings()?;
        let mut diagnostics = Vec::new();
        for (id, config) in [("ginza", &settings.ginza)] {
            if !config.enabled { self.workers.remove(id); diagnostics.push(json!({"id": id, "status": "disabled"})); continue; }
            let result = self.ensure_worker(id, config, generation);
            diagnostics.push(match result {
                Ok(worker) => { let worker = worker.lock().unwrap(); json!({"id": id, "status": "ready", "manifest": worker.manifest, "pid": worker.child.id()}) },
                Err(error) => json!({"id": id, "status": if self.cancellation.load(Ordering::Relaxed) != generation { "cancelled" } else { "failed" }, "error": error}),
            });
        }
        Ok(json!({"providers": diagnostics}))
    }

    pub fn analyze(&mut self, text: &str, generation: u64) -> (Vec<SourceArtifact>, Vec<Value>) {
        let settings = match self.settings() { Ok(s) => s, Err(e) => return (vec![], vec![json!({"id": "configuration", "status": "failed", "error": e})]) };
        let mut artifacts = Vec::new();
        let mut diagnostics = Vec::new();
        for (id, config) in [("ginza", settings.ginza.clone())] {
            if !config.enabled { diagnostics.push(json!({"id": id, "status": "disabled"})); continue; }
            if self.cancellation.load(Ordering::Relaxed) != generation { diagnostics.push(json!({"id": id, "status": "cancelled"})); continue; }
            self.sequence += 1;
            let request_id = format!("{id}-{}", self.sequence);
            let worker = match self.ensure_worker(id, &config, generation) {
                Ok(worker) => worker,
                Err(error) => {
                    diagnostics.push(json!({"id": id, "status": if self.cancellation.load(Ordering::Relaxed) != generation { "cancelled" } else { "failed" }, "error": error, "request_id": request_id}));
                    continue;
                }
            };
            let started = Instant::now();
            let mut cache_hit = false;
            let result = (|| {
                let (resource, identity) = {
                    let worker = worker.lock().unwrap();
                    (worker.manifest["resource_digest"].as_str().unwrap_or("unknown").to_string(), worker.identity.clone())
                };
                let digest = kotoclip_nlp::external::text_digest(text);
                let key = format!("{id}:{identity}:{resource}:{digest}");
                if let Some(artifact) = self.cache.lock().unwrap().get(&key) { cache_hit = true; return Ok((*artifact).clone()); }
                let artifact = worker.lock().unwrap().analyze(text, &request_id, config.timeout_seconds, &self.cancellation, generation)?;
                self.cache.lock().unwrap().insert(key, Arc::new(artifact.clone()));
                Ok::<_, String>(artifact)
            })();
            match result {
                Ok(artifact) => { diagnostics.push(json!({"id": id, "status": "ready", "elapsed_ms": started.elapsed().as_millis(), "request_id": request_id, "cache_hit": cache_hit})); artifacts.push(artifact); }
                Err(error) => { self.workers.remove(id); diagnostics.push(json!({"id": id, "status": if self.cancellation.load(Ordering::Relaxed) != generation { "cancelled" } else { "failed" }, "error": error, "request_id": request_id})); }
            }
        }
        artifacts.sort_by(|a, b| a.provider.id.cmp(&b.provider.id));
        diagnostics.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        (artifacts, diagnostics)
    }
}
