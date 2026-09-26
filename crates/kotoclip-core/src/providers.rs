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
}

struct Worker {
    child: Child,
    input: ChildStdin,
    output: Receiver<Result<Value, String>>,
    manifest: Value,
}

impl Worker {
    fn start(id: &str, config: &ProviderConfig, script: &Path, log: &Path, cancel: &AtomicU64, generation: u64) -> Result<Self, String> {
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
        let mut worker = Self { child, input, output: receiver, manifest: Value::Null };
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
        fs::create_dir_all(self.config_path.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::write(&self.config_path, serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        self.workers.clear();
        self.cache.lock().unwrap().clear();
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
            let worker = self.workers.get(id);
            let (pid, manifest) = worker.map(|w| { let w = w.lock().unwrap(); (Some(w.child.id()), Some(w.manifest.clone())) }).unwrap_or((None, None));
            json!({"id": id, "configured": config.python.is_file() && self.script.is_file(), "available": worker.is_some(), "enabled": config.enabled,
                "loaded": worker.is_some(), "pid": pid, "manifest": manifest})
        }).collect();
        Ok(json!({"settings": settings, "providers": providers, "config_path": self.config_path, "script": self.script}))
    }

    fn initialize(&mut self, id: &str, config: &ProviderConfig, generation: u64) -> Result<(), String> {
        if self.cancellation.load(Ordering::Relaxed) != generation { return Err("来源初始化已取消".into()); }
        if !self.workers.contains_key(id) {
            let log_dir = self.config_path.parent().unwrap().join("provider-logs");
            fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;
            let worker = Worker::start(id, config, &self.script, &log_dir.join(format!("{id}.log")), &self.cancellation, generation)
                .map_err(|error| format!("{id} 初始化失败：{error}"))?;
            self.workers.insert(id.into(), Arc::new(Mutex::new(worker)));
        }
        Ok(())
    }

    pub fn check(&mut self, generation: u64) -> Result<Value, String> {
        let settings = self.settings()?;
        self.workers.clear();
        self.cache.lock().unwrap().clear();
        let mut diagnostics = Vec::new();
        for (id, config) in [("ginza", &settings.ginza)] {
            if !config.enabled { diagnostics.push(json!({"id": id, "status": "disabled"})); continue; }
            let result = self.initialize(id, config, generation);
            diagnostics.push(match result {
                Ok(()) => { let worker = self.workers[id].lock().unwrap(); json!({"id": id, "status": "ready", "manifest": worker.manifest, "pid": worker.child.id()}) },
                Err(error) => json!({"id": id, "status": if self.cancellation.load(Ordering::Relaxed) != generation { "cancelled" } else { "failed" }, "error": error}),
            });
        }
        Ok(json!({"providers": diagnostics}))
    }

    pub fn analyze(&mut self, text: &str, generation: u64) -> (Vec<SourceArtifact>, Vec<Value>) {
        let settings = match self.settings() { Ok(s) => s, Err(e) => return (vec![], vec![json!({"id": "configuration", "status": "failed", "error": e})]) };
        let mut artifacts = Vec::new();
        let mut diagnostics = Vec::new();
        let mut jobs = Vec::new();
        let configs = [("ginza", settings.ginza.clone())];
        let mut pending = Vec::new();
        for (id, config) in configs {
            if !config.enabled { diagnostics.push(json!({"id": id, "status": "disabled"})); continue; }
            if self.cancellation.load(Ordering::Relaxed) != generation { diagnostics.push(json!({"id": id, "status": "cancelled"})); continue; }
            self.sequence += 1;
            let request_id = format!("{id}-{}", self.sequence);
            if let Some(worker) = self.workers.get(id).cloned() {
                jobs.push((id.to_string(), config, request_id, worker));
            } else {
                pending.push((id.to_string(), config, request_id));
            }
        }
        let script = self.script.clone();
        let config_path = self.config_path.clone();
        let cancellation = self.cancellation.clone();
        std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for (id, config, request_id) in pending {
                let script = script.clone();
                let config_path = config_path.clone();
                let cancellation = cancellation.clone();
                handles.push(scope.spawn(move || {
                    let log_dir = config_path.parent().unwrap().join("provider-logs");
                    let result = fs::create_dir_all(&log_dir).map_err(|e| e.to_string())
                        .and_then(|_| Worker::start(&id, &config, &script, &log_dir.join(format!("{id}.log")), &cancellation, generation));
                    result.map(|worker| (id.clone(), config, request_id, worker)).map_err(|error| (id, error))
                }));
            }
            for handle in handles {
                match handle.join().unwrap() {
                    Ok((id, config, request_id, worker)) => {
                        let worker = Arc::new(Mutex::new(worker));
                        self.workers.insert(id.clone(), worker.clone());
                        jobs.push((id, config, request_id, worker));
                    }
                    Err((id, error)) => diagnostics.push(json!({"id": id, "status": if cancellation.load(Ordering::Relaxed) != generation { "cancelled" } else { "failed" }, "error": error})),
                }
            }
        });
        let digest = kotoclip_nlp::external::text_digest(text);
        let cache = &self.cache;
        let cancellation = &self.cancellation;
        std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for (id, config, request_id, worker) in jobs {
                let digest = digest.clone();
                handles.push(scope.spawn(move || {
                    let started = Instant::now();
                    let mut cache_hit = false;
                    let result = (|| {
                        let resource = worker.lock().unwrap().manifest["resource_digest"].as_str().unwrap().to_string();
                        let key = format!("{id}:{resource}:{digest}");
                        if let Some(artifact) = cache.lock().unwrap().get(&key) { cache_hit = true; return Ok((*artifact).clone()); }
                        let artifact = worker.lock().unwrap().analyze(text, &request_id, config.timeout_seconds, cancellation, generation)?;
                        cache.lock().unwrap().insert(key, Arc::new(artifact.clone()));
                        Ok::<_, String>(artifact)
                    })();
                    (id, request_id, started.elapsed().as_millis(), cache_hit, result)
                }));
            }
            for handle in handles {
                match handle.join().unwrap() {
                    (id, request_id, elapsed, cache_hit, Ok(artifact)) => { diagnostics.push(json!({"id": id, "status": "ready", "elapsed_ms": elapsed, "request_id": request_id, "cache_hit": cache_hit})); artifacts.push(artifact); }
                    (id, request_id, _, _, Err(error)) => { self.workers.remove(&id); diagnostics.push(json!({"id": id, "status": if self.cancellation.load(Ordering::Relaxed) != generation { "cancelled" } else { "failed" }, "error": error, "request_id": request_id})); }
                }
            }
        });
        artifacts.sort_by(|a, b| a.provider.id.cmp(&b.provider.id));
        diagnostics.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        (artifacts, diagnostics)
    }
}
