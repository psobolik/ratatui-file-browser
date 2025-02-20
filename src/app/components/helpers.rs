use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use ratatui::widgets::{Block, BorderType, Padding};
use tokio::fs;

use crate::app::styles;

/// Returns the contents of a file as an array of Strings
pub(crate) async fn read_file(path: &Path) -> std::io::Result<Vec<String>> {
    let contents = fs::read_to_string(path).await?;
    Ok(contents.lines().map(|f| f.to_string()).collect())
}

/// Returns the contents of a directory sorted by name, with directories first
pub(crate) async fn read_directory(path: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut paths: Vec<(bool, PathBuf)> = vec![];
    let mut entries = fs::read_dir(&path).await?;
    while let Some(dir_entry) = entries.next_entry().await? {
        let entry = dir_entry.path();
        paths.push((entry.is_dir(), entry));
    }
    // Sort by name, directories first
    paths.sort_unstable_by(|(lhs_is_dir, lhs_path), (rhs_is_dir, rhs_path)| {
        if *lhs_is_dir && !*rhs_is_dir {
            Ordering::Less
        } else if !*lhs_is_dir && *rhs_is_dir {
            Ordering::Greater
        } else {
            lhs_path.file_name().cmp(&rhs_path.file_name())
        }
    });
    Ok(paths.iter().map(|(_, path)| path.clone()).collect())
}

/// Returns a ratatui::widgets::Block styled according to whether or not the component has focus
pub(crate) fn component_block<'a>(has_focus: bool) -> Block<'a> {
    if has_focus {
        Block::bordered()
            .border_type(BorderType::Double)
            .border_style(styles::FOCUSED_BLOCK_STYLE)
            .padding(Padding::horizontal(1))
    } else {
        Block::bordered().padding(Padding::horizontal(1))
    }
}
