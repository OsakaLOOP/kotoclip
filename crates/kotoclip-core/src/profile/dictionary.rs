use super::ProfileEngine;

impl ProfileEngine {
    pub fn default_dictionary(&self) -> Option<String> {
        self.default_dictionary_cache.lock().ok()?.clone()
    }

    pub fn dictionary_order(&self) -> Vec<String> {
        self.dictionary_order_cache
            .lock()
            .map(|order| order.clone())
            .unwrap_or_default()
    }

    pub fn set_dictionary_order(&self, order: &[String]) -> Result<(), rusqlite::Error> {
        let transaction = self.conn.unchecked_transaction()?;
        transaction.execute("DELETE FROM user_dictionary_priority", [])?;
        for (position, dictionary) in order.iter().enumerate() {
            transaction.execute(
                "INSERT INTO user_dictionary_priority(dictionary_name, position) VALUES (?1, ?2)",
                (dictionary, position as i64),
            )?;
        }
        let default_dictionary = order.first().map(String::as_str);
        transaction.execute(
            "INSERT INTO user_dictionary_settings(id, default_dictionary, updated_at) VALUES (1, ?1, datetime('now')) ON CONFLICT(id) DO UPDATE SET default_dictionary = excluded.default_dictionary, updated_at = excluded.updated_at",
            [default_dictionary],
        )?;
        transaction.commit()?;
        if let Ok(mut cache) = self.dictionary_order_cache.lock() {
            *cache = order.to_vec();
        }
        if let Ok(mut cache) = self.default_dictionary_cache.lock() {
            *cache = default_dictionary.map(str::to_string);
        }
        Ok(())
    }

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
