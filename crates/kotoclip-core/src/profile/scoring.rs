use super::ProfileEngine;
use crate::models::AnnotatedToken;
use crate::performance::TimingCollector;
use std::time::Instant;

impl ProfileEngine {
    /// 遍历 annotated tokens，结合 SQLite 画像库，标注生词评分 (Novelty) 并决定是否降级为 Plain Text
    pub fn annotate_tokens(
        &self,
        tokens: Vec<AnnotatedToken>,
    ) -> Result<Vec<AnnotatedToken>, Box<dyn std::error::Error>> {
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
        let exposures = self.get_all_exposures()?;
        report(0, total);
        for (index, token) in tokens.iter_mut().enumerate() {
            if token.display_class != "content" {
                token.novelty_score = 0.0;
                token.is_known = true;
                report_if_needed(&mut report, index + 1, total, report_step);
                continue;
            }
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
            if let Some(record) = exposures.get(base_form).and_then(|items| items.get(reading)) {
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

    /// 性能诊断版评分。查询和汉字推断均在实际调用点计时，
    /// 与常规评分保持相同的分支和结果。
    pub fn annotate_tokens_profiled(
        &self,
        mut tokens: Vec<AnnotatedToken>,
        timings: &mut TimingCollector,
    ) -> Result<Vec<AnnotatedToken>, Box<dyn std::error::Error>> {
        let exposure_started = Instant::now();
        let exposures = self.get_all_exposures()?;
        timings.add("画像曝光查询", exposure_started.elapsed());
        for token in &mut tokens {
            if token.display_class != "content" {
                token.novelty_score = 0.0;
                token.is_known = true;
                continue;
            }
            let base_form = &token.bunsetsu.head_word.base_form;
            let reading = &token.bunsetsu.head_word.reading;
            let pos_tag = &token.bunsetsu.head_word.pos;

            if pos_tag.major == "記号" {
                token.novelty_score = 0.0;
                token.is_known = true;
                continue;
            }

            let exposure = exposures.get(base_form).and_then(|items| items.get(reading));

            if let Some(record) = exposure {
                if record.is_known {
                    token.novelty_score = 0.0;
                    token.is_known = true;
                    continue;
                }

                let inference_started = Instant::now();
                let (kanji_conf, reason) = self.infer_novelty_confidence(base_form, reading)?;
                timings.add("汉字熟悉度推断", inference_started.elapsed());
                let exposure_decay = (-(record.exposure_count as f32) / 5.0).exp();
                token.novelty_score = 0.5 * (1.0 - kanji_conf) + 0.5 * exposure_decay;
                token.inference_reason = reason;
            } else {
                let inference_started = Instant::now();
                let (kanji_conf, reason) = self.infer_novelty_confidence(base_form, reading)?;
                timings.add("汉字熟悉度推断", inference_started.elapsed());
                token.novelty_score = 0.5 * (1.0 - kanji_conf) + 0.5;
                token.inference_reason = reason;
            }

            if token.novelty_score < 0.2 {
                token.is_known = true;
            }
        }

        Ok(tokens)
    }
}

fn report_if_needed<F: FnMut(usize, usize)>(
    report: &mut F,
    completed: usize,
    total: usize,
    step: usize,
) {
    if completed == total || completed % step == 0 {
        report(completed, total);
    }
}
