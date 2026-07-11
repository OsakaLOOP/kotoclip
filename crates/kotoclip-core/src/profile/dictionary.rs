use super::ProfileEngine;

impl ProfileEngine {
    pub fn dictionary_choice(&self, query_key: &str) -> Option<String> {
        self.dictionary_choice_cache.lock().ok()?.get(query_key).cloned()
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
