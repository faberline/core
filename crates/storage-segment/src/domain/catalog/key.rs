use crate::domain::{Result, SegmentError};

pub(crate) fn validate_catalog_key(key: &str) -> Result<()> {
    if key.is_empty() || key.len() > 1024 || key.contains('\0') {
        return Err(SegmentError::InvalidCatalogKey {
            key: key.to_string(),
        });
    }
    Ok(())
}

pub(crate) fn lexicographic_successor(value: &str) -> Option<String> {
    for (index, character) in value.char_indices().rev() {
        let mut scalar = u32::from(character).checked_add(1)?;
        if (0xd800..=0xdfff).contains(&scalar) {
            scalar = 0xe000;
        }
        if let Some(next) = char::from_u32(scalar) {
            let mut successor = value[..index].to_string();
            successor.push(next);
            return Some(successor);
        }
    }
    None
}
