use wp_model_core::model::TagSet;

/// 解析 Vec<String> 形式的标签为 TagSet。
/// 支持形如 "k:v"、"k=v" 以及无分隔符的 flag（值视为 "true"）。
pub fn parse_tags(items: &[String]) -> TagSet {
    let mut tags = TagSet::default();
    for item in items {
        if let Some((k, v)) = item.split_once(':').or_else(|| item.split_once('=')) {
            tags.set_tag(k.trim(), v.trim().to_string());
        } else {
            tags.set_tag(item.trim(), "true".to_string());
        }
    }
    tags
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagValidationError {
    TooManyItems { got: usize },
    InvalidKey { index: usize, key: String },
    InvalidValue { index: usize, value: String },
}

impl std::fmt::Display for TagValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManyItems { got } => {
                write!(f, "tags must have at most 4 items (got {})", got)
            }
            Self::InvalidKey { index, key } => write!(
                f,
                "invalid tag key at index {}: '{}' (allowed: [A-Za-z0-9_.-], len 1..=32)",
                index, key
            ),
            Self::InvalidValue { index, value } => write!(
                f,
                "invalid tag value at index {}: '{}' (allowed: [A-Za-z0-9_.:/=@+,-,${}], len 0..=64)",
                index, value
            ),
        }
    }
}

impl std::error::Error for TagValidationError {}

/// 校验 tags 列表是否满足约束：
/// - 数量：最多 4 个
/// - key：1..=32，字符集 [A-Za-z0-9_.-]
/// - value：0..=64，字符集 [A-Za-z0-9_.:/=@+,-]
pub fn validate_tags(items: &[String]) -> Result<(), TagValidationError> {
    if items.len() > 4 {
        return Err(TagValidationError::TooManyItems { got: items.len() });
    }
    for (idx, item) in items.iter().enumerate() {
        let (k, v) = if let Some((k, v)) = item.split_once(':').or_else(|| item.split_once('=')) {
            (k.trim(), v.trim())
        } else {
            (item.trim(), "true")
        };
        if k.is_empty() || k.len() > 32 || !k.chars().all(is_valid_key_char) {
            return Err(TagValidationError::InvalidKey {
                index: idx,
                key: k.to_string(),
            });
        }
        if v.len() > 64 || !v.chars().all(is_valid_val_char) {
            return Err(TagValidationError::InvalidValue {
                index: idx,
                value: v.to_string(),
            });
        }
    }
    Ok(())
}

fn is_valid_key_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')
}
fn is_valid_val_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            '_' | '.' | ':' | '/' | '=' | '@' | '+' | ',' | '-' | '$' | '{' | '}'
        )
}
