use super::ProfileEngine;
use crate::models::{AnnotatedToken, Morpheme, SegmentationCandidate, SegmentationChoice};
use rusqlite::params;

impl ProfileEngine {
    pub fn set_segmentation_choice(
        &self,
        source: &AnnotatedToken,
        candidate: &SegmentationCandidate,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let surface = &source.bunsetsu.surface;
        let candidate_surface: String = candidate
            .tokens
            .iter()
            .map(|token| token.bunsetsu.surface.as_str())
            .collect();
        if candidate_surface != *surface {
            return Err("候选路径与原文表层不一致".into());
        }
        let offset = source.bunsetsu.char_range.0;
        let mut morphemes: Vec<Morpheme> = candidate
            .tokens
            .iter()
            .flat_map(|token| token.bunsetsu.morphemes.iter().cloned())
            .collect();
        for morpheme in &mut morphemes {
            morpheme.char_range.0 = morpheme.char_range.0.saturating_sub(offset);
            morpheme.char_range.1 = morpheme.char_range.1.saturating_sub(offset);
        }
        let json = serde_json::to_string(&morphemes)?;
        self.conn.execute(
            "INSERT INTO user_segmentation_choices
                (surface, morphemes_json, selected_cost, selected_at)
             VALUES (?1, ?2, ?3, datetime('now'))
             ON CONFLICT(surface) DO UPDATE SET
                morphemes_json = excluded.morphemes_json,
                selected_cost = excluded.selected_cost,
                selected_at = datetime('now')",
            params![surface, json, candidate.total_cost],
        )?;
        Ok(())
    }

    pub fn get_segmentation_choices(
        &self,
    ) -> Result<Vec<SegmentationChoice>, Box<dyn std::error::Error>> {
        let mut statement = self.conn.prepare(
            "SELECT surface, morphemes_json, selected_cost, selected_at
             FROM user_segmentation_choices ORDER BY selected_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        let mut choices = Vec::new();
        for row in rows {
            let (surface, morphemes_json, selected_cost, selected_at) = row?;
            choices.push(SegmentationChoice {
                surface,
                morphemes: serde_json::from_str(&morphemes_json)?,
                selected_cost,
                selected_at,
            });
        }
        Ok(choices)
    }

    pub fn delete_segmentation_choice(&self, surface: &str) -> rusqlite::Result<bool> {
        Ok(self.conn.execute(
            "DELETE FROM user_segmentation_choices WHERE surface = ?1",
            [surface],
        )? > 0)
    }
}
