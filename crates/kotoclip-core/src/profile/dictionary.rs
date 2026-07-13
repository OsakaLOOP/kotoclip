use super::ProfileEngine;

impl ProfileEngine {
    pub fn dictionary_choice(&self, query_key: &str) -> Option<String> {
        self.dictionary_choice_cache
            .lock()
            .ok()?
            .get(query_key)
            .cloned()
    }

    pub fn set_dictionary_choice(
        &self,
        query_key: &str,
        selected_target: &str,
    ) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO user_dictionary_choices(query_key, selected_target, selected_at) VALUES (?1, ?2, datetime('now')) ON CONFLICT(query_key) DO UPDATE SET selected_target = excluded.selected_target, selected_at = excluded.selected_at",
            [query_key, selected_target],
        )?;
        if let Ok(mut cache) = self.dictionary_choice_cache.lock() {
            cache.insert(query_key.to_string(), selected_target.to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ProfileEngine;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn dictionary_choice_is_cached_and_persisted() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("kotoclip-profile-choice-{nonce}.sqlite"));
        {
            let profile = ProfileEngine::new(&path).unwrap();
            profile
                .set_dictionary_choice("いる\u{1f}イル", "いる【居る】")
                .unwrap();
            assert_eq!(
                profile.dictionary_choice("いる\u{1f}イル").as_deref(),
                Some("いる【居る】")
            );
        }
        let reopened = ProfileEngine::new(&path).unwrap();
        assert_eq!(
            reopened.dictionary_choice("いる\u{1f}イル").as_deref(),
            Some("いる【居る】")
        );
        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }
}
