/*
 * Copyright (c) 2023 Paul Sobolik
 * Created 2023-12-16
 */

use crate::stateful_list::StatefulList;
use crate::tui::Event;
use chrono::{DateTime, Local};
use crossterm::event::KeyCode::Char;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use number_prefix::NumberPrefix;
use probably_binary::{entry_type, EntryType, FileType};
use ratatui::{prelude::*, widgets::*};
use std::cmp::Ordering;
use std::fs::Metadata;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::fs;

const PARENT_DIRECTORY: &str = "..";
const DIRECTORY_ICON: char = '📁';
const DOCUMENT_ICON: char = '📄';
const UNKNOWN_ICON: char = '❔';

const OVERSIZE_FILE_STYLE: Style = Style::new().bg(Color::Yellow).fg(Color::Black);
const BINARY_FILE_STYLE: Style = Style::new().bg(Color::Yellow).fg(Color::Black);
const LIST_HIGHLIGHT_STYLE: Style = Style::new().fg(Color::Black).bg(Color::Gray);
const ERROR_STYLE: Style = Style::new().fg(Color::Red);
const FOCUSED_BLOCK_STYLE: Style = Style::new()
    .fg(Color::LightBlue)
    .add_modifier(Modifier::BOLD);

struct FrameSet {
    head: Rect,
    directory: Rect,
    file: Rect,
}

#[derive(Default, PartialEq)]
enum FocusedFrame {
    #[default]
    Directory,
    Files,
}

enum FolderItem {
    Folder,
    TextFile,
    OversizeTextFile,
    BinaryFile,
    Error(String),
}

enum FsError {
    Metadata(String),
    Other(String),
}

#[derive(Default)]
pub struct App {
    pub should_quit: bool,
    fs_error: Option<FsError>,
    text_frame: Rect,
    widest_line_len: usize,
    directory_list: StatefulList<PathBuf>,
    cwd: Option<PathBuf>,
    focused_frame: FocusedFrame,
    // This tells us what kind of thing the selected item represents
    folder_item: Option<FolderItem>,
    // If the item selected is a folder, this holds its entries
    file_folder_list: StatefulList<String>,
    folder_scrollbar_state: ScrollbarState,
    // If the item selected is a text file, this holds its contents
    file_text: Vec<String>,
    file_horizontal_scrollbar_state: ScrollbarState,
    file_vertical_scrollbar_state: ScrollbarState,
    file_horizontal_offset: usize,
    file_vertical_offset: usize,
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
        self.load_selected_file().await;
    }

    fn handle_resize_event(&mut self, width: u16, height: u16) {
        let frame_set = calculate_frames(Rect::new(0, 0, width, height));
        // Account for the padding applied when the file is rendered
        self.text_frame = frame_set.file.inner(&Margin {
            vertical: 1,
            horizontal: 2,
        });
        self.set_scrollbar_state();
        self.set_horizontal_scrollbar_state();
        self.set_vertical_scrollbar_state();
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
                self.directory_list.unselect();
            }
            self.fs_error = None;
            return;
        }
        match key_event.code {
            KeyCode::Esc => self.quit(),
            KeyCode::Tab => self.toggle_focus(),
            _ => {
                if self.focused_frame == FocusedFrame::Directory {
                    self.handle_directory_key_event(key_event).await;
                } else {
                    self.handle_file_key_event(key_event);
                }
            }
        }
    }
    async fn handle_directory_key_event(&mut self, key_event: KeyEvent) {
        // If nothing is selected, select the first item before processing the key
        if self.directory_list.selected().is_none() {
            self.set_selected(0);
            self.load_selected_file().await;
            // Don't process the Down key, though, so the first item stays selected in that case
            if is_down_key(key_event) {
                return;
            }
        }
        let mut selection_changed = false;

        if is_up_key(key_event) {
            // Move selection up one entry
            if !self.directory_list.is_first() {
                self.directory_list.previous();
                selection_changed = true;
            }
        } else if is_down_key(key_event) {
            // Move selection down one entry
            if !self.directory_list.is_last() {
                self.directory_list.next();
                selection_changed = true;
            }
        } else {
            match key_event.code {
                KeyCode::Home => {
                    // Move selection to first entry
                    if !self.directory_list.is_first() {
                        self.directory_list.first();
                        selection_changed = true;
                    }
                }
                KeyCode::End => {
                    // Move selection to last entry
                    if !self.directory_list.is_last() {
                        self.directory_list.last();
                        selection_changed = true;
                    }
                }
                KeyCode::PageUp => {
                    // Move selection up one page
                    if !self.directory_list.is_first() {
                        self.directory_list.retreat(self.text_frame.height as usize);
                        selection_changed = true;
                    }
                }
                KeyCode::PageDown => {
                    // Move selection down one page
                    if !self.directory_list.is_last() {
                        self.directory_list.advance(self.text_frame.height as usize);
                        selection_changed = true;
                    }
                }
                // Open selected item if it's a folder
                KeyCode::Enter => selection_changed = self.cd().await,
                key_code => {
                    // Move selection to item starting with character
                    if let Char(c) = key_code {
                        self.select_by_char(c).await;
                        selection_changed = true;
                    }
                }
            };
        }
        if selection_changed {
            self.load_selected_file().await;
        }
    }

    fn handle_file_key_event(&mut self, key_event: KeyEvent) {
        if let Some(file_contents) = &self.folder_item {
            match file_contents {
                FolderItem::Folder => self.handle_folder_key_event(key_event),
                FolderItem::TextFile => self.handle_text_key_event(key_event),
                _ => {}
            }
        }
    }

    fn handle_folder_key_event(&mut self, key_event: KeyEvent) {
        if is_up_key(key_event) {
            // Scroll up one line
            if !self.file_folder_list.at_offset_first() {
                self.file_folder_list.previous_offset();
                self.sync_scrollbar_position();
            }
        } else if is_down_key(key_event) {
            // Scroll down one line
            if self.file_folder_list.offset() < self.folder_vertical_page_limit() {
                self.file_folder_list.next_offset();
                self.sync_scrollbar_position();
            } else {
                self.folder_scrollbar_state.last();
            }
        } else {
            match key_event.code {
                KeyCode::Home => {
                    // Scroll to top of list
                    if !self.file_folder_list.at_offset_first() {
                        self.file_folder_list.offset_first();
                        self.folder_scrollbar_state.first();
                    }
                }
                KeyCode::End => {
                    // Scroll to end of list
                    if self.file_folder_list.len() > self.text_frame.height as usize {
                        self.file_folder_list
                            .set_offset(self.folder_vertical_page_limit());
                        self.folder_scrollbar_state.last();
                    }
                }
                KeyCode::PageUp => {
                    // Scroll up one page
                    let frame_height = self.text_frame.height as usize;
                    if self.file_folder_list.offset() > frame_height {
                        self.file_folder_list
                            .set_offset(self.file_folder_list.offset() - frame_height);
                        self.sync_scrollbar_position();
                    } else {
                        self.file_folder_list.offset_first();
                        self.folder_scrollbar_state.first();
                    };
                }
                KeyCode::PageDown => {
                    // Scroll down one page
                    let frame_height = self.text_frame.height as usize;
                    let max_offset = self.folder_vertical_page_limit();
                    let offset = self.file_folder_list.offset() + frame_height;
                    if offset < max_offset {
                        self.file_folder_list.set_offset(offset);
                        self.sync_scrollbar_position();
                    } else {
                        self.file_folder_list.set_offset(max_offset);
                        self.folder_scrollbar_state.last();
                    };
                }
                _ => {}
            }
        }
    }

    fn folder_vertical_page_limit(&self) -> usize {
        page_limit(self.file_folder_list.len(), self.text_frame.height as usize)
    }

    fn sync_scrollbar_position(&mut self) {
        self.folder_scrollbar_state = self
            .folder_scrollbar_state
            .position(self.file_folder_list.offset());
    }

    fn handle_text_key_event(&mut self, key_event: KeyEvent) {
        if is_up_key(key_event) {
            if self.can_scroll_vertically() && self.file_vertical_offset > 0 {
                // Scroll up one line
                self.file_vertical_offset -= 1;
                self.file_vertical_scrollbar_state.prev();
            }
        } else if is_down_key(key_event) {
            if self.can_scroll_vertically() {
                // Scroll down one line
                if self.file_vertical_offset < self.file_vertical_page_limit() {
                    self.file_vertical_offset += 1;
                    self.file_vertical_scrollbar_state.next();
                }
            }
        } else {
            match key_event.code {
                KeyCode::Home => {
                    if self.can_scroll_vertically() && key_event.modifiers == KeyModifiers::CONTROL
                    {
                        // Scroll to top of file
                        self.file_vertical_offset = 0;
                        self.file_vertical_scrollbar_state.first();
                    } else if self.can_scroll_horizontally() {
                        // Go to beginning of line
                        self.file_horizontal_offset = 0;
                        self.file_horizontal_scrollbar_state.first();
                    }
                }
                KeyCode::End => {
                    if self.can_scroll_vertically() && key_event.modifiers == KeyModifiers::CONTROL
                    {
                        // Scroll to bottom of file
                        if self.file_text.len() > self.text_frame.height as usize {
                            self.file_vertical_offset = self.file_vertical_page_limit();
                            self.file_vertical_scrollbar_state.last();
                        }
                    } else if self.can_scroll_horizontally() {
                        // Scroll to end of line
                        self.file_horizontal_offset = self.file_horizontal_page_limit();
                        self.file_horizontal_scrollbar_state.last();
                    }
                }
                KeyCode::PageUp => {
                    if self.can_scroll_vertically() {
                        // Scroll up one page
                        let frame_height = self.text_frame.height as usize;
                        if self.file_vertical_offset > frame_height {
                            self.file_vertical_offset -= frame_height;
                            self.file_vertical_scrollbar_state = self
                                .file_vertical_scrollbar_state
                                .position(self.file_vertical_offset);
                        } else {
                            self.file_vertical_offset = 0;
                            self.file_vertical_scrollbar_state.first();
                        }
                    }
                }
                KeyCode::PageDown => {
                    // Scroll down one page
                    if self.can_scroll_vertically() {
                        let frame_height = self.text_frame.height as usize;
                        let limit = self.file_vertical_page_limit();
                        if self.file_vertical_offset + frame_height < limit {
                            self.file_vertical_offset += frame_height;
                            self.file_vertical_scrollbar_state = self
                                .file_vertical_scrollbar_state
                                .position(self.file_vertical_offset);
                        } else {
                            self.file_vertical_offset = limit;
                            self.file_vertical_scrollbar_state.last();
                        }
                    }
                }
                KeyCode::Left => {
                    // Scroll left one character
                    if self.can_scroll_horizontally() && self.file_horizontal_offset > 0 {
                        self.file_horizontal_offset -= 1;
                        self.file_horizontal_scrollbar_state.prev();
                    }
                }
                KeyCode::Right => {
                    // Scroll right one character
                    if self.can_scroll_horizontally()
                        && self.file_horizontal_offset < self.file_horizontal_page_limit()
                    {
                        self.file_horizontal_offset += 1;
                        self.file_horizontal_scrollbar_state.next();
                    }
                }
                _ => {}
            }
        }
    }

    fn can_scroll_horizontally(&self) -> bool {
        self.widest_line_len > self.text_frame.width as usize
    }

    fn can_scroll_vertically(&self) -> bool {
        self.file_text.len() > self.text_frame.height as usize
    }

    fn file_vertical_page_limit(&self) -> usize {
        page_limit(self.file_text.len(), self.text_frame.height as usize)
    }

    fn file_horizontal_page_limit(&self) -> usize {
        page_limit(self.widest_line_len, self.text_frame.width as usize)
    }

    fn selected_item(&self) -> Option<PathBuf> {
        self.directory_list
            .selected()
            .map(|selected| self.directory_list[selected].clone())
    }

    async fn select_by_char(&mut self, ch: char) -> bool {
        let selected = self
            .directory_list
            .selected()
            .unwrap_or_else(|| self.directory_list.lower_bound());
        if let Some(index) = self.find_by_char(ch, selected + 1).await {
            self.set_selected(index)
        } else if let Some(index) = self.find_by_char(ch, 0).await {
            self.set_selected(index)
        } else {
            false
        }
    }

    async fn find_by_char(&mut self, ch: char, from: usize) -> Option<usize> {
        let cn = ch.to_ascii_lowercase() as u8;

        self.directory_list
            .iter()
            .enumerate()
            .find(|(index, entry)| {
                if let Some(file_name) = entry.file_name() {
                    index >= &from && file_name.as_encoded_bytes()[0].to_ascii_lowercase() == cn
                } else {
                    false
                }
            })
            .map(|(index, _)| index)
    }

    fn set_selected(&mut self, selected: usize) -> bool {
        if Some(selected) == self.directory_list.selected() {
            false
        } else {
            self.directory_list.set_selected(Some(selected));
            true
        }
    }

    fn quit(&mut self) {
        self.should_quit = true;
    }

    fn toggle_focus(&mut self) {
        if self.focused_frame == FocusedFrame::Directory {
            self.focused_frame = FocusedFrame::Files
        } else {
            self.focused_frame = FocusedFrame::Directory
        }
    }

    async fn load_selected_file(&mut self) {
        if let Some(entry) = self.selected_item() {
            let entry = entry.as_path();
            self.folder_item = Some(match entry_type(entry) {
                Ok(entry_type) => match entry_type {
                    EntryType::Directory => self.load_folder(entry).await,
                    EntryType::File(file_type) => self.load_file(file_type, entry).await,
                },
                Err(error) => FolderItem::Error(format!("Error selecting file: {error}")),
            });
        } else {
            self.folder_item = None;
        }
    }

    async fn read_cwd(&mut self) -> io::Result<(PathBuf, Vec<PathBuf>)> {
        let cwd = std::env::current_dir()?;
        match read_directory(&cwd).await {
            Ok(entries) => {
                let mut result = vec![];
                if cwd.parent().is_some() {
                    let mut p = cwd.clone();
                    p.push(PARENT_DIRECTORY);
                    result.push(p);
                }
                result.extend(entries);
                Ok((cwd, result))
            }
            Err(error) => Err(error),
        }
    }

    async fn load_cwd(&mut self) {
        match self.read_cwd().await {
            Ok((cwd, items)) => {
                self.directory_list = StatefulList::with_items(items);
                self.cwd = Some(cwd);
                self.directory_list.first(); // Because no line is selected by default
            }
            Err(error) => {
                self.fs_error = Some(FsError::Other(format!("Error loading directory: {error}")))
            }
        }
    }

    async fn load_folder(&mut self, entry: &Path) -> FolderItem {
        match read_directory(entry).await {
            Ok(items) => {
                let strings = items
                    .iter()
                    .map(|entry| format!("{} {}", path_icon(entry), entry_name(entry)))
                    .collect();
                self.file_folder_list = StatefulList::with_items(strings);
                self.set_scrollbar_state();
                FolderItem::Folder
            }
            Err(error) => FolderItem::Error(error.to_string()),
        }
    }

    fn set_scrollbar_state(&mut self) {
        let height = self.text_frame.height as usize;
        let len = self.file_folder_list.upper_bound() + 1;
        if len > height {
            self.folder_scrollbar_state = ScrollbarState::new(len - height).position(0);
        } else {
            self.folder_scrollbar_state = ScrollbarState::default();
        }
    }

    async fn load_file(&mut self, file_type: FileType, entry: &Path) -> FolderItem {
        match file_type {
            FileType::Text => {
                if file_size(entry) >= 50_000 {
                    FolderItem::OversizeTextFile
                } else {
                    match read_file(entry).await {
                        Ok(lines) => {
                            self.widest_line_len = widest_line_length(&lines);
                            self.file_text = lines;

                            self.file_horizontal_offset = 0;
                            self.set_horizontal_scrollbar_state();

                            self.file_vertical_offset = 0;
                            self.set_vertical_scrollbar_state();
                            FolderItem::TextFile
                        }
                        Err(error) => FolderItem::Error(error.to_string()),
                    }
                }
            }
            FileType::Binary => FolderItem::BinaryFile,
        }
    }

    fn set_horizontal_scrollbar_state(&mut self) {
        let line_width = self.widest_line_len;
        let frame_width = self.text_frame.width as usize;
        if line_width > self.text_frame.width as usize {
            self.file_horizontal_scrollbar_state =
                ScrollbarState::new(line_width - frame_width).position(0);
        } else {
            self.file_horizontal_scrollbar_state = ScrollbarState::default();
            // Hides scrollbar
        };
    }

    fn set_vertical_scrollbar_state(&mut self) {
        let height = self.text_frame.height as usize;
        let len = self.file_text.len();
        if len > height {
            self.file_vertical_scrollbar_state = ScrollbarState::new(len - height).position(0);
        } else {
            self.file_vertical_scrollbar_state = ScrollbarState::default();
            // Hides scrollbar
        }
    }

    async fn cd(&mut self) -> bool {
        if let Some(selected) = self.selected_item() {
            if selected.is_dir() {
                match std::env::set_current_dir(selected) {
                    Ok(_) => self.load_cwd().await,
                    Err(error) => {
                        self.fs_error =
                            Some(FsError::Other(format!("Error changing directory: {error}")))
                    }
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn render(&mut self, frame: &mut Frame<'_>) {
        let frame_rect = frame.size();

        let frame_set = calculate_frames(frame_rect);
        // Account for the padding applied when the file is rendered
        self.text_frame = frame_set.file.inner(&Margin {
            vertical: 1,
            horizontal: 2,
        });

        self.render_directory(frame, frame_set.directory);

        if let Some(cwd) = self.cwd.clone() {
            self.render_head(&cwd, frame, frame_set.head);
        }
        if let Some(entry) = self.selected_item() {
            self.render_file(&entry, frame, frame_set.file);
        }
        if let Some(fs_error) = &self.fs_error {
            let message = match fs_error {
                FsError::Metadata(message) => message,
                FsError::Other(message) => message,
            };
            self.render_error(message, frame, frame_rect);
        }
    }

    fn render_head(&mut self, entry: &Path, frame: &mut Frame, area: Rect) {
        frame.render_widget(
            Paragraph::new(clip_string(&entry_path(entry), area.width as usize)),
            area,
        );
    }

    fn render_file(&mut self, entry: &Path, frame: &mut Frame<'_>, area: Rect) {
        match entry.metadata() {
            Ok(metadata) => {
                let size_string = if metadata.is_dir() {
                    format!("{} items", self.file_folder_list.len())
                } else {
                    metadata_size_string(&metadata)
                };
                let title = format!(
                    "[{} - {}]",
                    metadata_modified_string(&metadata),
                    size_string,
                );
                let block = if self.focused_frame == FocusedFrame::Files {
                    focused_block()
                } else {
                    default_block()
                }
                .title(title);

                if let Some(file_contents) = &self.folder_item {
                    match file_contents {
                        FolderItem::Folder => self.render_list_items_file(block, frame, area),
                        FolderItem::TextFile => self.render_text_file(block, frame, area),
                        FolderItem::OversizeTextFile => {
                            self.render_oversize_text_file(block, frame, area)
                        }
                        FolderItem::BinaryFile => self.render_binary_file(block, frame, area),
                        FolderItem::Error(message) => {
                            self.render_file_error((&message).to_string(), block, frame, area)
                        }
                    }
                }
            }
            // Why would there be no metadata? Possibly due to converting directory name from OS?
            Err(error) => {
                self.fs_error = Some(FsError::Metadata(format!(
                    "Error reading metadata: {error}"
                )))
            }
        }
    }

    fn render_list_items_file(&mut self, block: Block<'_>, frame: &mut Frame<'_>, area: Rect) {
        let items: Vec<ListItem> = self
            .file_folder_list
            .iter()
            .map(|item| ListItem::new(Line::from(item.to_string())))
            .collect();
        let list = List::new(items).block(block);
        frame.render_stateful_widget(list, area, &mut self.file_folder_list.state);

        let scrollbar = Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(
            scrollbar,
            area.inner(&Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut self.folder_scrollbar_state,
        );
    }

    fn render_text_file(&mut self, block: Block<'_>, frame: &mut Frame<'_>, area: Rect) {
        let items: Vec<Line> = self
            .file_text
            .iter()
            .map(|item| Line::from(item.to_string()))
            .collect();
        let paragraph = Paragraph::new(items.clone())
            .scroll((
                self.file_vertical_offset as u16,
                self.file_horizontal_offset as u16,
            ))
            .block(block);

        frame.render_widget(paragraph, area);

        let vertical_scrollbar =
            Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(
            vertical_scrollbar,
            area.inner(&Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut self.file_vertical_scrollbar_state,
        );

        let horizontal_scrollbar =
            Scrollbar::default().orientation(ScrollbarOrientation::HorizontalBottom);
        frame.render_stateful_widget(
            horizontal_scrollbar,
            area.inner(&Margin {
                vertical: 0,
                horizontal: 1,
            }),
            &mut self.file_horizontal_scrollbar_state,
        );
    }

    fn render_oversize_text_file(&mut self, block: Block<'_>, frame: &mut Frame<'_>, area: Rect) {
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(" Oversize Text File (Max 50 kb) ")
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: false })
                .style(OVERSIZE_FILE_STYLE),
            Rect::new(area.x + 2, area.y + 2, area.width - 4, 1),
        )
    }

    fn render_binary_file(&mut self, block: Block<'_>, frame: &mut Frame<'_>, area: Rect) {
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(" Binary File ")
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: false })
                .style(BINARY_FILE_STYLE),
            Rect::new(area.x + 2, area.y + 2, area.width - 4, 1),
        )
    }

    fn render_file_error(
        &mut self,
        message: String,
        block: Block<'_>,
        frame: &mut Frame<'_>,
        area: Rect,
    ) {
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(Text::from(message.as_str()))
                .style(ERROR_STYLE)
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: false }),
            Rect::new(area.x + 2, area.y + 2, area.width - 4, area.height - 4),
        )
    }

    fn render_directory(&mut self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .directory_list
            .iter()
            .map(|entry| {
                ListItem::new(Line::from(format!(
                    "{} {}",
                    path_icon(entry),
                    entry_name(entry)
                )))
            })
            .collect();
        // Don't include parent directory in count
        let mut item_count = items.len();
        if (entry_name(&self.directory_list[0]) == PARENT_DIRECTORY) && item_count > 0 {
            item_count -= 1;
        }
        let item_count_string = format!("[{item_count} items]");
        let block = if self.focused_frame == FocusedFrame::Directory {
            focused_block()
        } else {
            default_block()
        }
        .title(item_count_string);
        let list = List::new(items)
            .block(block)
            .highlight_style(LIST_HIGHLIGHT_STYLE);
        frame.render_stateful_widget(list, area, &mut self.directory_list.state);
    }

    fn render_error(&self, error: &String, frame: &mut Frame, frame_size: Rect) {
        let text = Paragraph::new(Text::from(error.as_str())).style(ERROR_STYLE);
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
    let mut paths: Vec<PathBuf> = vec![];
    let mut entries = fs::read_dir(&path).await?;
    while let Some(entry) = entries.next_entry().await? {
        paths.push(entry.path());
    }
    // Sort by name, directories first
    paths.sort_unstable_by(|lhs, rhs| {
        if lhs.is_dir() && !rhs.is_dir() {
            Ordering::Less
        } else if !lhs.is_dir() && rhs.is_dir() {
            Ordering::Greater
        } else {
            lhs.file_name().cmp(&rhs.file_name())
        }
    });
    Ok(paths)
}

fn focused_block<'a>() -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(FOCUSED_BLOCK_STYLE)
        .padding(Padding::horizontal(1))
}

fn default_block<'a>() -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(1))
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

fn file_size(path: &Path) -> u64 {
    if let Ok(metadata) = path.metadata() {
        metadata.len()
    } else {
        0
    }
}

fn clip_string(string: &String, width: usize) -> String {
    if string.len() > width {
        let start = string.len() - width + 1;
        format!("…{}", &string[start..])
    } else {
        string.to_string()
    }
}

fn entry_name(entry: &Path) -> String {
    if entry.ends_with(PARENT_DIRECTORY) {
        PARENT_DIRECTORY.to_string()
    } else {
        match entry.file_name() {
            Some(file_name) => match file_name.to_str() {
                Some(file_name) => file_name.to_string(),
                _ => entry.display().to_string(),
            },
            _ => entry.display().to_string(),
        }
    }
}

fn entry_path(path: &Path) -> String {
    if path.ends_with(PARENT_DIRECTORY) {
        let mut pb = path.to_path_buf();
        pb.pop();
        pb = PathBuf::from(entry_path(pb.as_path()));
        pb.push(PARENT_DIRECTORY);
        pb.to_str().unwrap().to_string()
    } else if path.file_name().is_some() {
        if let Some(path_string) = path.to_str() {
            path_string.to_string()
        } else {
            "".into() // Path has a name that can't be converted?
        }
    } else {
        "".into() // Path has no name
    }
}

fn path_icon(entry: &Path) -> char {
    if entry.is_dir() {
        DIRECTORY_ICON
    } else if entry.is_file() {
        DOCUMENT_ICON
    } else {
        UNKNOWN_ICON
    }
}

fn widest_line_length(lines: &[String]) -> usize {
    lines.iter().fold(
        0,
        |acc, line| {
            if acc < line.len() {
                line.len()
            } else {
                acc
            }
        },
    )
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
        file: main[1],
    }
}

fn page_limit(total_size: usize, page_size: usize) -> usize {
    if total_size > page_size {
        total_size - page_size
    } else {
        0
    }
}

fn is_up_key(key_event: KeyEvent) -> bool {
    key_event.code == KeyCode::Up
        || (Char('p') == key_event.code && key_event.modifiers == KeyModifiers::CONTROL)
}

fn is_down_key(key_event: KeyEvent) -> bool {
    key_event.code == KeyCode::Down
        || (Char('n') == key_event.code && key_event.modifiers == KeyModifiers::CONTROL)
}
