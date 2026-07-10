use crate::models::DictEntry;

/// 根据用户的偏好设置，对多本词典的检索结果进行优先级重排与聚合整理
pub fn sort_definitions(mut entries: Vec<DictEntry>, priority_list: &[String]) -> Vec<DictEntry> {
    if priority_list.is_empty() || entries.is_empty() {
        return entries;
    }

    entries.sort_by(|a, b| {
        // 获取两本词典在优先级列表中的位置 (索引越小，优先级越高)
        let pos_a = priority_list
            .iter()
            .position(|name| name.eq_ignore_ascii_case(&a.dict_name))
            .unwrap_or(usize::MAX);
            
        let pos_b = priority_list
            .iter()
            .position(|name| name.eq_ignore_ascii_case(&b.dict_name))
            .unwrap_or(usize::MAX);

        pos_a.cmp(&pos_b)
    });

    entries
}
