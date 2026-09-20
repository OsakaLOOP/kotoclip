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
    sync::{atomic::{AtomicU64, Ordering}, mpsc::{self, Receiver}, Arc},
    time::{Duration, Instant},
};

const PROTOCOL: &str = "kotoclip.provider-process.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub python: PathBuf,
    pub model: String,
    pub enabled: bool,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSettings {
    pub ginza: ProviderConfig,
    pub kwja: ProviderConfig,
    pub kwja_cache: PathBuf,
    pub hf_cache: PathBuf,
}

impl ProviderSettings {
    pub fn development(root: &Path) -> Self {
        Self {
            ginza: ProviderConfig { python: root.join("experiments/ginza311/Scripts/python.exe"), model: "ja_ginza".into(), enabled: true, timeout_seconds: 120 },
            kwja: ProviderConfig { python: root.join("experiments/kwja311/Scripts/python.exe"), model: "tiny".into(), enabled: true, timeout_seconds: 120 },
            kwja_cache: root.join("experiments/kwja-cache"), hf_cache: root.join("experiments/hf-cache"),
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
    fn start(id: &str, config: &ProviderConfig, settings: &ProviderSettings, script: &Path, log: &Path, cancel: &AtomicU64, generation: u64) -> Result<Self, String> {
        let mut command = Command::new(&config.python);
        command.args(["-X", "utf8", "-u"]).arg(script)
            .args(["--provider", id, "--model", &config.model])
            .arg("--kwja-cache").arg(&settings.kwja_cache)
            .arg("--hf-cache").arg(&settings.hf_cache)
            .stdin(Stdio::piped()).stdout(Stdio::piped())
            .stderr(Stdio::from(File::create(log).map_err(|e| format!("无法创建来源日志：{e}"))?));
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
    workers: HashMap<String, Worker>,
    pub cancellation: Arc<AtomicU64>,
    sequence: u64,
}

impl ProviderManager {
    pub fn new(config_path: PathBuf, script: PathBuf, defaults: ProviderSettings) -> Self {
        Self { config_path, script, defaults, workers: HashMap::new(), cancellation: Arc::new(AtomicU64::new(0)), sequence: 0 }
    }

    pub fn settings(&self) -> Result<ProviderSettings, String> {
        match fs::read_to_string(&self.config_path) {
            Ok(contents) => serde_json::from_str(&contents).map_err(|e| format!("来源配置无效：{e}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(self.defaults.clone()),
            Err(e) => Err(format!("无法读取来源配置：{e}")),
        }
    }

    pub fn configure(&mut self, settings: ProviderSettings) -> Result<Value, String> {
        for (id, config) in [("ginza", &settings.ginza), ("kwja", &settings.kwja)] {
            if config.timeout_seconds == 0 || config.model.trim().is_empty() { return Err(format!("{id} 的模型和超时设置无效")); }
        }
        fs::create_dir_all(self.config_path.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::write(&self.config_path, serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        self.workers.clear();
        self.status()
    }

    pub fn status(&self) -> Result<Value, String> {
        let settings = self.settings()?;
        let providers: Vec<Value> = [("ginza", &settings.ginza), ("kwja", &settings.kwja)].into_iter().map(|(id, config)| {
            let worker = self.workers.get(id);
            json!({"id": id, "available": config.python.is_file() && self.script.is_file(), "enabled": config.enabled,
                "loaded": worker.is_some(), "pid": worker.map(|w| w.child.id()), "manifest": worker.map(|w| &w.manifest)})
        }).collect();
        Ok(json!({"settings": settings, "providers": providers, "config_path": self.config_path, "script": self.script}))
    }

    pub fn analyze(&mut self, text: &str, generation: u64) -> (Vec<SourceArtifact>, Vec<Value>) {
        let settings = match self.settings() { Ok(s) => s, Err(e) => return (vec![], vec![json!({"id": "configuration", "status": "failed", "error": e})]) };
        let mut artifacts = Vec::new();
        let mut diagnostics = Vec::new();
        for (id, config) in [("ginza", &settings.ginza), ("kwja", &settings.kwja)] {
            if !config.enabled { diagnostics.push(json!({"id": id, "status": "disabled"})); continue; }
            if self.cancellation.load(Ordering::Relaxed) != generation { diagnostics.push(json!({"id": id, "status": "cancelled"})); continue; }
            self.sequence += 1;
            let request_id = format!("{id}-{}", self.sequence);
            let started = Instant::now();
            let result = (|| {
                if !self.workers.contains_key(id) {
                    let log_dir = self.config_path.parent().unwrap().join("provider-logs");
                    fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;
                    let worker = Worker::start(id, config, &settings, &self.script, &log_dir.join(format!("{id}.log")), &self.cancellation, generation)?;
                    self.workers.insert(id.into(), worker);
                }
                self.workers.get_mut(id).unwrap().analyze(text, &request_id, config.timeout_seconds, &self.cancellation, generation)
            })();
            match result {
                Ok(artifact) => {
                    diagnostics.push(json!({"id": id, "status": "ready", "elapsed_ms": started.elapsed().as_millis(), "request_id": request_id}));
                    artifacts.push(artifact);
                }
                Err(error) => {
                    self.workers.remove(id);
                    diagnostics.push(json!({"id": id, "status": if self.cancellation.load(Ordering::Relaxed) != generation { "cancelled" } else { "failed" }, "error": error, "request_id": request_id}));
                }
            }
        }
        (artifacts, diagnostics)
    }
}
