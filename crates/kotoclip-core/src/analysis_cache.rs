//! 分析产物按序列化体积和条目数限制容量。
use serde::Serialize;
use std::{collections::{HashMap, VecDeque}, sync::Arc};

pub struct AnalysisCache<T> {
    entries: HashMap<String, (Arc<T>, usize)>,
    order: VecDeque<String>,
    bytes: usize,
    limit: usize,
}

impl<T: Serialize> AnalysisCache<T> {
    pub fn new(limit: usize) -> Self { Self { entries: HashMap::new(), order: VecDeque::new(), bytes: 0, limit } }

    pub fn get(&mut self, key: &str) -> Option<Arc<T>> {
        let value = self.entries.get(key)?.0.clone();
        self.order.retain(|item| item != key);
        self.order.push_back(key.into());
        Some(value)
    }

    pub fn insert(&mut self, key: String, value: Arc<T>) {
        let size = serde_json::to_vec(value.as_ref()).expect("分析产物序列化").len();
        if let Some((_, old)) = self.entries.remove(&key) { self.bytes -= old; }
        self.order.retain(|item| item != &key);
        self.bytes += size;
        self.entries.insert(key.clone(), (value, size));
        self.order.push_back(key);
        while (self.bytes > self.limit || self.entries.len() > 256) && self.entries.len() > 1 {
            let key = self.order.pop_front().unwrap();
            self.bytes -= self.entries.remove(&key).unwrap().1;
        }
    }

    pub fn clear(&mut self) { self.entries.clear(); self.order.clear(); self.bytes = 0; }
}
