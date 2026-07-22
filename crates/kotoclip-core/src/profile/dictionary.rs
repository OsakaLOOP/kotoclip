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
}
