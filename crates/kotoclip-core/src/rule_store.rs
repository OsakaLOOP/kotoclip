//! 用户语言规则的 UTF-8 JSON 存储与版本管理。
use kotoclip_nlp::rules::Rule;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Mutex, time::{SystemTime, UNIX_EPOCH}};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuleSnapshot {
    pub schema: String,
    pub version: u64,
    pub rules: Vec<Rule>,
}

pub struct RuleStore {
    path: PathBuf,
    state: Mutex<RuleSnapshot>,
}

impl RuleStore {
    pub fn open(path: PathBuf) -> Self {
        let state = std::fs::read_to_string(&path).ok()
            .and_then(|text| serde_json::from_str::<RuleSnapshot>(&text).ok())
            .filter(|state| state.schema == "kotoclip.user-language-rules.v1")
            .unwrap_or_else(|| RuleSnapshot { schema: "kotoclip.user-language-rules.v1".into(), version: 0, rules: Vec::new() });
        Self { path, state: Mutex::new(state) }
    }

    pub fn snapshot(&self) -> RuleSnapshot {
        self.state.lock().unwrap().clone()
    }

    pub fn save(&self, mut rule: Rule) -> Result<RuleSnapshot, String> {
        if rule.id.trim().is_empty() {
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|error| error.to_string())?.as_nanos();
            rule.id = format!("user.{stamp:x}");
        }
        if !rule.id.starts_with("user.") { return Err("用户规则 ID 必须以 user. 开头".into()); }
        rule.validate()?;
        let mut state = self.state.lock().unwrap();
        if let Some(index) = state.rules.iter().position(|item| item.id == rule.id) { state.rules[index] = rule; }
        else { state.rules.push(rule); }
        state.version += 1;
        self.persist(&state)?;
        Ok(state.clone())
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<RuleSnapshot, String> {
        let mut state = self.state.lock().unwrap();
        let rule = state.rules.iter_mut().find(|item| item.id == id).ok_or("用户规则不存在")?;
        rule.enabled = enabled;
        state.version += 1;
        self.persist(&state)?;
        Ok(state.clone())
    }

    pub fn delete(&self, id: &str) -> Result<RuleSnapshot, String> {
        let mut state = self.state.lock().unwrap();
        let previous = state.rules.len();
        state.rules.retain(|item| item.id != id);
        if state.rules.len() == previous { return Err("用户规则不存在".into()); }
        state.version += 1;
        self.persist(&state)?;
        Ok(state.clone())
    }

    fn persist(&self, state: &RuleSnapshot) -> Result<(), String> {
        if let Some(parent) = self.path.parent() { std::fs::create_dir_all(parent).map_err(|error| error.to_string())?; }
        let text = serde_json::to_string_pretty(state).map_err(|error| error.to_string())? + "\n";
        std::fs::write(&self.path, text).map_err(|error| format!("语言规则保存失败：{error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kotoclip_nlp::rules::Atom;

    fn rule() -> Rule {
        Rule { id: String::new(), label: "测试规则".into(), description: String::new(), kind: "idiom".into(),
            atoms: vec![Atom { surfaces: vec!["例".into()], ..Default::default() }], priority: 1, enabled: true,
            document_id: None, allow_whitespace: false, concept_id: None, sense_ids: Vec::new(), display_from: 0,
            display_to: None, source_refs: Vec::new(), gap_after_atom: None, gap_bunsetsu: None }
    }

    #[test]
    fn persists_updates_and_deletion() {
        let path = std::env::temp_dir().join(format!("kotoclip-rule-store-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let store = RuleStore::open(path.clone());
        let saved = store.save(rule()).unwrap();
        let id = saved.rules[0].id.clone();
        assert_eq!(saved.version, 1);
        assert!(!store.set_enabled(&id, false).unwrap().rules[0].enabled);
        assert!(RuleStore::open(path.clone()).snapshot().rules[0].id.starts_with("user."));
        assert!(store.delete(&id).unwrap().rules.is_empty());
        let _ = std::fs::remove_file(path);
    }
}
