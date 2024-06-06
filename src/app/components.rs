/*
 * Copyright (c) 2024 Paul Sobolik
 * Created 2024-03-17
 */

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::prelude::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Padding};
use ratatui::Frame;
use tokio::fs;

pub(crate) mod directory;
pub(crate) mod head;
pub(crate) mod preview;

pub(crate) trait Component {
    fn set_area(&mut self, area: Rect);
    fn has_focus(&self) -> bool;
    fn set_focus(&mut self, focus: bool);
    fn hit_test(&self, x: u16, y: u16) -> bool;
    async fn handle_mouse_event(&mut self, mouse_event: MouseEvent) -> Result<(), std::io::Error>;
    async fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<(), std::io::Error>;
    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) -> Result<(), std::io::Error>;
}

async fn read_file(path: &Path) -> std::io::Result<Vec<String>> {
    let contents = fs::read_to_string(path).await?;
    Ok(contents.lines().map(|f| f.to_string()).collect())
}

async fn read_directory(path: &Path) -> std::io::Result<Vec<PathBuf>> {
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

pub fn component_block<'a>(has_focus: bool) -> Block<'a> {
    if has_focus {
        focused_block()
    } else {
        default_block()
    }
}
fn focused_block<'a>() -> Block<'a> {
    const FOCUSED_BLOCK_STYLE: Style = Style::new()
        .fg(Color::LightBlue)
        .add_modifier(Modifier::BOLD);

    Block::bordered()
        .border_type(BorderType::Double)
        .border_style(FOCUSED_BLOCK_STYLE)
        .padding(Padding::horizontal(1))
}

fn default_block<'a>() -> Block<'a> {
    Block::bordered()
        .padding(Padding::horizontal(1))
}
