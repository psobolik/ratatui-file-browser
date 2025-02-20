/*
 * Copyright (c) 2024 Paul Sobolik
 * Created 2024-04-03
 */

use std::fs::Metadata;
use std::path::Path;
use std::time::SystemTime;

use chrono::{DateTime, Local};
use number_prefix::NumberPrefix;
use ratatui::layout::Rect;
use ratatui::Frame;

pub trait PreviewPane {
    fn render(
        &mut self,
        area: Rect,
        frame: &mut Frame<'_>,
        has_focus: bool,
    ) -> Result<(), std::io::Error>;

    fn page_limit(total_size: usize, page_size: usize) -> usize {
        total_size.saturating_sub(page_size)
    }
}

pub fn file_title(entry: &Path) -> Result<String, std::io::Error> {
    let metadata = &entry.metadata()?;
    Ok(format!(
        "[{} - {}]",
        metadata_modified_string(metadata),
        metadata_size_string(metadata)
    ))
}

pub fn folder_title(entry: &Path, item_count: usize) -> Result<String, std::io::Error> {
    let metadata = &entry.metadata()?;
    Ok(format!(
        "[{} - {} item{}]",
        metadata_modified_string(metadata),
        item_count,
        if item_count != 1 { "s" } else { "" },
    ))
}

fn metadata_modified_string(metadata: &Metadata) -> String {
    match modified_datetime(metadata) {
        Some(modified) => {
            format!("{}", modified.format("%Y-%m-%d %H:%M"))
        }
        _ => "".to_string(),
    }
}

fn modified_datetime(metadata: &Metadata) -> Option<DateTime<Local>> {
    match metadata.modified() {
        Ok(modified) => {
            let dur = modified.duration_since(SystemTime::UNIX_EPOCH).unwrap();
            Some::<DateTime<Local>>(
                chrono::DateTime::from_timestamp(dur.as_secs() as i64, 0)
                    .unwrap()
                    .into(),
            )
        }
        _ => None, // No modified value
    }
}

fn metadata_size_string(metadata: &Metadata) -> String {
    // Not meant to be precise...
    match NumberPrefix::decimal(metadata.len() as f64) {
        NumberPrefix::Standalone(_) => "1 kB".into(),
        NumberPrefix::Prefixed(prefix, n) => {
            format!("{:.0} {}B", n, prefix.symbol())
        }
    }
}
