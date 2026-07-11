use super::ProfileEngine;
use crate::models::AnnotatedToken;

impl ProfileEngine {
    /// 遍历 annotated tokens，结合 SQLite 画像库，标注生词评分 (Novelty) 并决定是否降级为 Plain Text
    pub fn annotate_tokens(&self, tokens: Vec<AnnotatedToken>) -> Result<Vec<AnnotatedToken>, Box<dyn std::error::Error>> {
        self.annotate_tokens_with_progress(tokens, |_, _| {})
    }

    pub fn annotate_tokens_with_progress<F>(
        &self,
        mut tokens: Vec<AnnotatedToken>,
        mut report: F,
    ) -> Result<Vec<AnnotatedToken>, Box<dyn std::error::Error>>
    where
        F: FnMut(usize, usize),
    {
        let total = tokens.len();
        let report_step = (total / 100).max(1);
        report(0, total);
        for (index, token) in tokens.iter_mut().enumerate() {
            let base_form = &token.bunsetsu.head_word.base_form;
            let reading = &token.bunsetsu.head_word.reading;
            let pos_tag = &token.bunsetsu.head_word.pos;

            // 1. 如果该文节的核心词为标点符号，默认直接标记为“已知” (Plain Text)，无需胶囊
            if pos_tag.major == "記号" {
                token.novelty_score = 0.0;
                token.is_known = true;
                report_if_needed(&mut report, index + 1, total, report_step);
                continue;
            }

            // 2. 查询该词的历史曝光和标记记录
            if let Some(record) = self.get_exposure(base_form, reading)? {
                // 如果用户已经主动将其标记为已知，则直接赋予 0 生词权重
                if record.is_known {
                    token.novelty_score = 0.0;
                    token.is_known = true;
                    report_if_needed(&mut report, index + 1, total, report_step);
                    continue;
                }

                // 曝光衰减公式：exp(-count / 5.0)
                let count = record.exposure_count as f32;
                let exposure_decay = (-count / 5.0).exp();

                // 汉字掌握置信度
                let (kanji_conf, reason) = self.infer_novelty_confidence(base_form, reading)?;
                let kanji_novelty = 1.0 - kanji_conf;

                // 权重融合：曝光权重 0.5 + 汉字新颖度 0.5
                let score = 0.5 * kanji_novelty + 0.5 * exposure_decay;
                token.novelty_score = score;
                token.inference_reason = reason;
            } else {
                // 无曝光历史 (Cold Start)，纯依靠汉字掌握度推断
                let (kanji_conf, reason) = self.infer_novelty_confidence(base_form, reading)?;
                let kanji_novelty = 1.0 - kanji_conf;

                // 曝光衰减为 1.0 (最大新颖度)
                let score = 0.5 * kanji_novelty + 0.5 * 1.0;
                token.novelty_score = score;
                token.inference_reason = reason;
            }

            // 3. 设定阈值自动标记已知以脱去胶囊外衣 (Novelty < 0.2 时，判定为已知)
            if token.novelty_score < 0.2 {
                token.is_known = true;
            }
            report_if_needed(&mut report, index + 1, total, report_step);
        }

        Ok(tokens)
    }
}

fn report_if_needed<F: FnMut(usize, usize)>(report: &mut F, completed: usize, total: usize, step: usize) {
    if completed == total || completed % step == 0 {
        report(completed, total);
    }
}
