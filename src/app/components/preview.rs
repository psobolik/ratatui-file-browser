/*
 * Copyright (c) 2024 Paul Sobolik
 * Created 2024-03-18
 */

use std::path::{Path, PathBuf};

use crossterm::event::{KeyEvent, MouseEvent};
use probably_binary::{EntryType, FileType};
use ratatui::layout::{Alignment, Position};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::{layout::Rect, Frame};

use binary::Binary;
use folder::Folder;
use list_pane::ListPane;
use message_pane::MessagePane;
use other::Other;
use oversize::Oversize;
use preview_pane::PreviewPane;
use text::Text;

use crate::app::{components, styles};
use crate::util;

use super::Component;

mod binary;
mod folder;
mod list_pane;
mod message_pane;
mod other;
mod oversize;
mod preview_pane;
mod text;

enum PreviewType {
    Folder,
    TextFile,
    OversizeTextFile,
    BinaryFile,
    OtherFile,
    Error(String),
}

#[derive(Default)]
pub struct Preview<'a> {
    has_focus: bool,
    area: Rect,

    // The entry being previewed
    entry: Option<PathBuf>,

    // What kind of item the entry is
    preview_type: Option<PreviewType>,

    binary_pane: Binary,
    other_pane: Other,
    oversize_pane: Oversize,
    folder_pane: Folder<'a>,
    text_pane: Text<'a>,
}

impl Component for Preview<'_> {
    fn set_area(&mut self, area: Rect) {
        self.area = area;
        self.folder_pane.set_area(self.area);
        self.text_pane.set_area(self.area);
    }

    fn has_focus(&self) -> bool {
        self.has_focus
    }

    fn set_focus(&mut self, focus: bool) {
        self.has_focus = focus;
    }

    fn hit_test(&self, x: u16, y: u16) -> bool {
        self.area.contains(Position { x, y })
    }

    async fn handle_mouse_event(&mut self, mouse_event: MouseEvent) -> Result<(), std::io::Error> {
        if let Some(preview_type) = &self.preview_type {
            match preview_type {
                PreviewType::Folder => self.folder_pane.handle_mouse_event(mouse_event),
                PreviewType::TextFile => self.text_pane.handle_mouse_event(mouse_event),
                _ => {}
            }
        }
        Ok(())
    }

    async fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<(), std::io::Error> {
        if let Some(file_contents) = &self.preview_type {
            match file_contents {
                PreviewType::Folder => self.folder_pane.handle_key_event(key_event),
                PreviewType::TextFile => self.text_pane.handle_key_event(key_event),
                _ => {}
            }
        }
        Ok(())
    }

    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) -> Result<(), std::io::Error> {
        self.set_area(area);

        if let Some(file_contents) = &self.preview_type {
            match file_contents {
                PreviewType::Folder => {
                    self.folder_pane.render(self.area, frame, self.has_focus)?;
                }
                PreviewType::TextFile => {
                    self.text_pane.render(self.area, frame, self.has_focus)?;
                }
                PreviewType::OversizeTextFile => {
                    self.oversize_pane
                        .render(self.area, frame, self.has_focus())?;
                }
                PreviewType::BinaryFile => {
                    self.binary_pane.render(self.area, frame, self.has_focus)?;
                }
                PreviewType::OtherFile => {
                    self.other_pane.render(self.area, frame, self.has_focus())?;
                }
                PreviewType::Error(message) => {
                    self.render_error(message, frame);
                }
            }
        }
        Ok(())
    }
}

impl Preview<'_> {
    pub fn clear(&mut self) {
        self.entry = None;
        self.preview_type = None;

        self.binary_pane.clear();
        self.other_pane.clear();
        self.oversize_pane.clear();
        self.folder_pane.clear();
        self.text_pane.clear();
    }

    pub fn set_error(&mut self, entry: &Path, message: String) {
        self.clear();
        self.entry = Some(PathBuf::from(entry));
        self.preview_type = Some(PreviewType::Error(message));
    }

    pub fn set_folder_items(&mut self, entry: &Path, items: Vec<PathBuf>) {
        self.clear();
        self.entry = Some(PathBuf::from(entry));
        self.folder_pane
            .init(Some(&entry.to_path_buf()), items, self.area);
        self.preview_type = Some(PreviewType::Folder);
    }

    pub fn set_text_file(&mut self, entry: &Path, lines: Vec<String>) {
        self.clear();
        self.entry = Some(PathBuf::from(entry));
        self.text_pane
            .init(Some(&entry.to_path_buf()), lines, self.area);
        self.preview_type = Some(PreviewType::TextFile);
    }

    pub fn set_oversize_text_file(&mut self, entry: &Path) {
        self.clear();
        self.entry = Some(PathBuf::from(entry));
        self.oversize_pane.init(Some(&entry.to_path_buf()));
        self.preview_type = Some(PreviewType::OversizeTextFile);
    }

    pub fn set_binary_file(&mut self, entry: &Path) {
        self.clear();
        self.entry = Some(PathBuf::from(entry));
        self.binary_pane.init(Some(&entry.to_path_buf()));
        self.preview_type = Some(PreviewType::BinaryFile);
    }

    pub fn set_other_file(&mut self, entry: &Path) {
        self.clear();
        self.entry = Some(PathBuf::from(entry));
        self.other_pane.init(Some(&entry.to_path_buf()));
        self.preview_type = Some(PreviewType::OtherFile);
    }

    pub async fn load_entry(&mut self, entry: Option<PathBuf>) {
        self.clear();

        if let Some(entry) = entry.as_ref() {
            match probably_binary::entry_type(entry) {
                Ok(entry_type) => match entry_type {
                    EntryType::Directory => {
                        match components::helpers::read_directory(entry).await {
                            Ok(entries) => self.set_folder_items(entry, entries),
                            Err(error) => self.set_error(entry, error.to_string()),
                        };
                    }
                    EntryType::File(file_type) => self.load_file(file_type, entry).await,
                    EntryType::Other => self.set_other_file(entry),
                },
                Err(error) => {
                    self.set_error(entry, error.to_string());
                }
            }
        }
    }

    async fn load_file(&mut self, file_type: FileType, entry: &Path) {
        match file_type {
            FileType::Text => {
                if util::file_size(entry) >= 50_000 {
                    self.set_oversize_text_file(entry);
                } else {
                    match components::helpers::read_file(entry).await {
                        Ok(lines) => {
                            self.set_text_file(entry, lines);
                        }
                        Err(error) => self.set_error(entry, error.to_string()),
                    }
                }
            }
            FileType::Binary => self.set_binary_file(entry),
        }
    }

    fn render_error(&self, message: &str, frame: &mut Frame<'_>) {
        let block = components::helpers::component_block(self.has_focus);
        frame.render_widget(block, self.area);
        frame.render_widget(
            Paragraph::new(ratatui::prelude::Text::from(message))
                .style(styles::ERROR_STYLE)
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: false }),
            Rect::new(
                self.area.x + 2,
                self.area.y + 2,
                self.area.width - 4,
                self.area.height - 4,
            ),
        )
    }
}
