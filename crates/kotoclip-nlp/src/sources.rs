use crate::model::*;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};
use vibrato::{dictionary::LexType, Dictionary, Tokenizer};

pub fn parse_fields(raw: &str, unknown: bool) -> Result<Vec<FeatureField>, String> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(raw.as_bytes());
    let record = reader
        .records()
        .next()
        .ok_or("UniDic 字段为空")?
        .map_err(|e| format!("UniDic CSV 错误：{e}"))?;
    if reader.records().next().is_some() {
        return Err("UniDic token 包含多条记录".into());
    }
    if (!unknown && record.len() != 29) || (unknown && record.len() != 6 && record.len() != 29) {
        return Err(format!(
            "UniDic 字段数量不符：{}，预期系统词 29 列或未知词 6 列",
            record.len()
        ));
    }
    Ok(FIELD_NAMES
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let raw = record.get(index).map(str::to_owned);
            FeatureField {
                index,
                name: name.to_string(),
                label: FIELD_LABELS[index].into(),
                value: raw
                    .as_ref()
                    .filter(|s| !s.is_empty() && s.as_str() != "*")
                    .cloned(),
                raw,
            }
        })
        .collect())
}

pub struct UniDicProvider {
    tokenizer: Tokenizer,
    metadata: ProviderMetadata,
}

impl UniDicProvider {
    pub fn open(register: Register, path: &Path) -> Result<Self, String> {
        let mut reader =
            BufReader::new(File::open(path).map_err(|e| format!("{}：{e}", path.display()))?);
        let mut digest = Sha256::new();
        let mut buffer = vec![0; 1024 * 1024];
        loop {
            let size = reader.read(&mut buffer).map_err(|e| e.to_string())?;
            if size == 0 {
                break;
            }
            digest.update(&buffer[..size]);
        }
        let dictionary_sha256 = format!("{:x}", digest.finalize());
        let expected = match register {
            Register::Cwj => "4add7eb2de073611ff9a01f62b1ca07e157a559381d7de9ab0d060f786fcc5e7",
            Register::Csj => "24ed4e27613479ac9a45ec8669dfbf1dadfc75679dd5689db36b8783da3f7330",
        };
        if dictionary_sha256 != expected {
            return Err(format!(
                "{} 资源摘要与 UniDic 2025.12 清单不符",
                register.name()
            ));
        }
        let dictionary =
            Dictionary::read(BufReader::new(File::open(path).map_err(|e| e.to_string())?))
                .map_err(|e| e.to_string())?;
        let tokenizer = Tokenizer::new(dictionary)
            .max_grouping_len(10)
            .ignore_space(true)
            .map_err(|e| e.to_string())?;
        Ok(Self {
            tokenizer,
            metadata: ProviderMetadata {
                id: format!("unidic-{}-202512", register.name()),
                version: "2025.12".into(),
                dictionary_sha256,
                field_schema: "unidic-node-29.v1".into(),
                max_grouping_length: 10,
                ignore_space: true,
            },
        })
    }

    pub fn analyze(&self, text: &str) -> Result<SourceAnalysis, String> {
        let mut worker = self.tokenizer.new_worker();
        let mut tokens = Vec::new();
        let mut byte_offset = 0;
        let mut char_offset = 0;
        // 按原始物理行调用来源，单次请求内复用 lattice，保留行末及空白坐标。
        for line in text.split_inclusive('\n') {
            if line.chars().count() > 8192 {
                return Err("单行最多支持 8,192 字符，请分段后分析".into());
            }
            worker.reset_sentence(line);
            worker.tokenize();
            for token in worker.token_iter() {
                let unknown = token.lex_type() == LexType::Unknown;
                let range = token.range_char();
                let bytes = token.range_byte();
                tokens.push(ProviderToken {
                    index: tokens.len(),
                    surface: token.surface().into(),
                    char_range: [char_offset + range.start, char_offset + range.end],
                    byte_range: [byte_offset + bytes.start, byte_offset + bytes.end],
                    lexicon_type: match token.lex_type() {
                        LexType::System => "system",
                        LexType::User => "user",
                        LexType::Unknown => "unknown",
                    }
                    .into(),
                    left_id: token.left_id(),
                    right_id: token.right_id(),
                    word_cost: token.word_cost(),
                    total_cost: token.total_cost(),
                    raw_feature: token.feature().into(),
                    fields: parse_fields(token.feature(), unknown)?,
                });
            }
            byte_offset += line.len();
            char_offset += line.chars().count();
        }
        Ok(SourceAnalysis {
            provider: self.metadata.clone(),
            tokens,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn csv_missing_values_and_large_identifiers_remain_exact() {
        let raw = "助動詞,*,*,*,助動詞-タ,終止形-一般,タ,た,た,タ,た,タ,和,*,*,*,*,*,*,助動詞,タ,タ,タ,タ,*,\"動詞%F2@1,形容詞%F4@-2\",*,9007199254740993,123";
        let fields = parse_fields(raw, false).unwrap();
        assert_eq!(fields.len(), 29);
        assert_eq!(fields[25].value.as_deref(), Some("動詞%F2@1,形容詞%F4@-2"));
        assert_eq!(fields[27].value.as_deref(), Some("9007199254740993"));
        assert_eq!(fields[1].raw.as_deref(), Some("*"));
        assert!(fields[1].value.is_none());
        let unknown = parse_fields("名詞,普通名詞,一般,*,*,*", true).unwrap();
        assert!(unknown[20].raw.is_none());
        assert!(parse_fields("名詞,一般,*,*,*,*", false).is_err());
    }
}
