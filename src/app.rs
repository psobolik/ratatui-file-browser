/*
 * Copyright (c) 2023-2024 Paul Sobolik
 * Created 2024-03-18
 */
mod components;
mod fs_error;
mod styles;

use crate::app::{
    components::directory::Directory, components::head::Head, components::preview::Preview,
    fs_error::FsError,
};
use crate::{constants, tui::Event, util};
use crossterm::{
    event::KeyCode::Char,
    event::{KeyCode, KeyEvent, KeyModifiers},
};
use probably_binary::{entry_type, EntryType, FileType};
use ratatui::{prelude::*, widgets::*};
use std::{
    cmp::Ordering,
    io,
    path::{Path, PathBuf},
};
use tokio::fs;

struct FrameSet {
    head: Rect,
    directory: Rect,
    preview: Rect,
}

#[derive(Default)]
pub struct App {
    pub should_quit: bool,
    fs_error: Option<FsError>,
    // Components
    head: Head,
    directory: Directory,
    preview: Preview,
}

impl App {
    pub async fn handle_event(&mut self, event: Event) {
        match event {
            Event::Key(key_event) => self.handle_key_event(key_event).await,
            Event::Init => self.handle_init_event().await,
            Event::Resize(width, height) => self.handle_resize_event(width, height),
            _ => {}
        }
    }

    async fn handle_init_event(&mut self) {
        self.load_cwd().await;
        self.load_selected_item().await;
        self.directory.set_focus(true);
        self.preview.set_focus(false);
    }

    fn handle_resize_event(&mut self, width: u16, height: u16) {
        let frame_set = calculate_frames(Rect::new(0, 0, width, height));
        self.head.handle_resize_event(frame_set.head);
        self.directory.handle_resize_event(frame_set.directory);
        self.preview.handle_resize_event(frame_set.preview);
    }

    async fn handle_key_event(&mut self, key_event: KeyEvent) {
        // Ctrl+C closes the app, regardless of state
        if Char('c') == key_event.code && key_event.modifiers == KeyModifiers::CONTROL {
            self.quit();
            return;
        }
        // If there's an error pending, clear it on any keypress and stop processing the event.
        // If the error occurred reading the selected item's metadata, also clear the selection.
        if self.fs_error.is_some() {
            if let Some(FsError::Metadata(_)) = self.fs_error {
                self.directory.clear_selection();
            }
            self.fs_error = None;
            return;
        }
        match key_event.code {
            KeyCode::Esc => self.quit(),
            KeyCode::Tab => self.toggle_focus(),
            _ => {
                if self.directory.has_focus() {
                    self.handle_directory_key_event(key_event).await;
                }
                if self.preview.has_focus() {
                    self.preview.handle_key_event(key_event);
                }
            }
        }
    }

    async fn handle_directory_key_event(&mut self, key_event: KeyEvent) {
        match self.directory.handle_key_event(key_event) {
            Ok((selection_changed, directory_changed)) => {
                if directory_changed {
                    self.load_cwd().await;
                }
                if selection_changed {
                    self.load_selected_item().await;
                }
            }
            Err(error) => self.fs_error = Some(error),
        }
    }

    fn quit(&mut self) {
        self.should_quit = true;
    }

    fn toggle_focus(&mut self) {
        let directory_has_focus = self.directory.has_focus();
        self.directory.set_focus(!directory_has_focus);
        self.preview.set_focus(directory_has_focus);
    }

    async fn load_selected_item(&mut self) {
        if let Some(entry) = self.directory.selected_item() {
            let entry = entry.as_path();
            match entry_type(entry) {
                Ok(entry_type) => match entry_type {
                    // EntryType::Directory => self.load_folder(entry).await,
                    EntryType::Directory => {
                        match read_directory(entry).await {
                            Ok(entries) => self.preview.set_folder_items(entry, entries),
                            Err(error) => self.preview.set_error(entry, error.to_string()),
                        };
                    }
                    EntryType::File(file_type) => self.load_file(file_type, entry).await,
                },
                Err(error) => {
                    self.preview
                        .set_error(entry, format!("Error selecting file: {error}"));
                }
            };
        } else {
            self.preview.clear();
        }
    }

    pub async fn load_cwd(&mut self) {
        match std::env::current_dir() {
            Ok(cwd) => {
                match read_directory(&cwd).await {
                    Ok(entries) => {
                        let mut result = vec![];
                        // Prepend parent directory entry if there is one
                        if cwd.parent().is_some() {
                            let mut p = cwd.clone();
                            p.push(constants::PARENT_DIRECTORY);
                            result.push(p);
                        }
                        result.extend(entries);
                        self.directory.set_items(result);
                        self.head.set_path(Some(cwd));
                    }
                    Err(error) => self.fs_error = Some(FsError::Directory(error)),
                }
            }
            Err(error) => {
                self.head.set_path(None);
                self.fs_error = Some(FsError::Directory(error))
            }
        }
    }

    async fn load_file(&mut self, file_type: FileType, entry: &Path) {
        match file_type {
            FileType::Text => {
                if util::file_size(entry) >= 50_000 {
                    self.preview.set_oversize_text_file(entry);
                } else {
                    match read_file(entry).await {
                        Ok(lines) => {
                            self.preview.set_text_file(entry, lines);
                        }
                        Err(error) => self.preview.set_error(entry, error.to_string()),
                    }
                }
            }
            FileType::Binary => self.preview.set_binary_file(entry),
        }
    }

    pub fn render(&mut self, frame: &mut Frame<'_>) {
        let frame_rect = frame.size();

        let frame_set = calculate_frames(frame_rect);

        self.head.render(frame, frame_set.head);
        self.directory.render(frame, frame_set.directory);
        if let Err(error) = self.preview.render(frame, frame_set.preview) {
            self.fs_error = Some(error);
        }

        if let Some(fs_error) = &self.fs_error {
            let message = match fs_error {
                FsError::Metadata(message) => message,
                FsError::Directory(message) => message,
            };
            self.render_error(&message.to_string(), frame, frame_rect);
        }
    }

    fn render_error(&self, error: &str, frame: &mut Frame, frame_size: Rect) {
        let text = Paragraph::new(Text::from(error)).style(styles::ERROR_STYLE);
        let block = Block::default().title("Error").borders(Borders::ALL);

        let error_len = error.len() as u16;
        let area = centered_rect(error_len + 4, 3, frame_size);
        let error_area = centered_rect(error_len, 1, area);

        frame.render_widget(Clear, area); //this clears out the background
        frame.render_widget(block, area);
        frame.render_widget(text, error_area);
    }
}

async fn read_file(path: &Path) -> io::Result<Vec<String>> {
    let contents = fs::read_to_string(path).await?;
    Ok(contents.lines().map(|f| f.to_string()).collect())
}

async fn read_directory(path: &Path) -> io::Result<Vec<PathBuf>> {
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

fn centered_rect(width: u16, height: u16, rect: Rect) -> Rect {
    let vert_margin = (rect.height - height) / 2;
    let horiz_margin = (rect.width - width) / 2;
    let vert_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(vert_margin),
            Constraint::Length(height),
            Constraint::Length(vert_margin),
        ])
        .split(rect);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(horiz_margin),
            Constraint::Length(width),
            Constraint::Length(horiz_margin),
        ])
        .split(vert_layout[1])[1]
}

fn calculate_frames(frame_rect: Rect) -> FrameSet {
    let root = Layout::default()
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(frame_rect);
    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(root[1]);

    FrameSet {
        head: root[0],
        directory: main[0],
        preview: main[1],
    }
}
