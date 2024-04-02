/*
 * Copyright (c) 2024 Paul Sobolik
 * Created 2024-03-18
 */

use crate::{app::styles, stateful_list::StatefulList, util};
use chrono::{DateTime, Local};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use number_prefix::NumberPrefix;
use ratatui::{
    layout::{Alignment, Margin, Rect},
    prelude::{Line, Text},
    widgets::{Block, List, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};
use std::{
    fs::Metadata,
    path::{Path, PathBuf},
    time::SystemTime,
};

enum PreviewItem {
    Folder,
    TextFile,
    OversizeTextFile,
    BinaryFile,
    Error(String),
}

#[derive(Default)]
pub struct Preview {
    has_focus: bool,
    area: Rect,
    inner_area: Rect,

    // The entry being previewed
    entry: Option<PathBuf>,

    // What kind of item the entry is
    preview_item: Option<PreviewItem>,

    // If the entry is a folder, this holds its entries
    file_folder_list: StatefulList<PathBuf>,
    folder_scrollbar_state: ScrollbarState,

    // If the entry is a text file, this holds its contents
    file_text: Vec<String>,
    widest_line_len: usize,
    file_horizontal_scrollbar_state: ScrollbarState,
    file_vertical_scrollbar_state: ScrollbarState,
    file_horizontal_offset: usize,
    file_vertical_offset: usize,
}

impl Preview {
    pub fn has_focus(&self) -> bool {
        self.has_focus
    }

    pub fn set_focus(&mut self, focus: bool) -> &mut Self {
        self.has_focus = focus;
        self
    }

    pub fn hit_test(&self, x: u16, y: u16) -> bool {
        util::is_in_rect(x, y, self.area)
    }

    pub fn handle_resize_event(&mut self, rect: Rect) {
        self.set_area(rect);

        self.set_scrollbar_state();
        self.set_horizontal_scrollbar_state();
        self.set_vertical_scrollbar_state();
    }

    pub fn clear(&mut self) {
        self.entry = None;
        self.preview_item = None;
    }

    pub fn set_error(&mut self, entry: &Path, message: String) {
        self.entry = Some(PathBuf::from(entry));
        self.preview_item = Some(PreviewItem::Error(message));
    }

    pub fn set_folder_items(&mut self, entry: &Path, items: Vec<PathBuf>) {
        self.entry = Some(PathBuf::from(entry));
        self.file_folder_list = StatefulList::with_items(items);
        self.set_scrollbar_state();
        self.preview_item = Some(PreviewItem::Folder);
    }

    pub fn set_text_file(&mut self, entry: &Path, lines: Vec<String>) {
        self.entry = Some(PathBuf::from(entry));
        self.widest_line_len = widest_line_length(&lines);
        self.file_text = lines;

        self.file_horizontal_offset = 0;
        self.set_horizontal_scrollbar_state();

        self.file_vertical_offset = 0;
        self.set_vertical_scrollbar_state();

        self.preview_item = Some(PreviewItem::TextFile);
    }

    pub fn set_oversize_text_file(&mut self, entry: &Path) {
        self.entry = Some(PathBuf::from(entry));
        self.preview_item = Some(PreviewItem::OversizeTextFile);
    }

    pub fn set_binary_file(&mut self, entry: &Path) {
        self.entry = Some(PathBuf::from(entry));
        self.preview_item = Some(PreviewItem::BinaryFile);
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) {
        if let Some(file_contents) = &self.preview_item {
            match file_contents {
                PreviewItem::Folder => self.handle_folder_key_event(key_event),
                PreviewItem::TextFile => self.handle_text_key_event(key_event),
                _ => {}
            }
        }
    }

    fn handle_folder_key_event(&mut self, key_event: KeyEvent) {
        if util::is_up_key(key_event) {
            // Scroll up one line
            if !self.file_folder_list.at_offset_first() {
                self.file_folder_list.previous_offset();
                self.sync_scrollbar_position();
            }
        } else if util::is_down_key(key_event) {
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
                    if self.file_folder_list.len() > self.inner_area.height as usize {
                        self.file_folder_list
                            .set_offset(self.folder_vertical_page_limit());
                        self.folder_scrollbar_state.last();
                    }
                }
                KeyCode::PageUp => {
                    // Scroll up one page
                    let frame_height = self.inner_area.height as usize;
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
                    let frame_height = self.inner_area.height as usize;
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
        page_limit(self.file_folder_list.len(), self.inner_area.height as usize)
    }

    fn sync_scrollbar_position(&mut self) {
        self.folder_scrollbar_state = self
            .folder_scrollbar_state
            .position(self.file_folder_list.offset());
    }

    fn handle_text_key_event(&mut self, key_event: KeyEvent) {
        if util::is_up_key(key_event) {
            if self.can_scroll_vertically() && self.file_vertical_offset > 0 {
                // Scroll up one line
                self.file_vertical_offset -= 1;
                self.file_vertical_scrollbar_state.prev();
            }
        } else if util::is_down_key(key_event) {
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
                    } else if self.can_scroll_horizontally()
                        && key_event.modifiers != KeyModifiers::CONTROL
                    {
                        // Go to beginning of line
                        self.file_horizontal_offset = 0;
                        self.file_horizontal_scrollbar_state.first();
                    }
                }
                KeyCode::End => {
                    if self.can_scroll_vertically() && key_event.modifiers == KeyModifiers::CONTROL
                    {
                        // Scroll to bottom of file
                        self.file_vertical_offset = self.file_vertical_page_limit();
                        self.file_vertical_scrollbar_state.last();
                    } else if self.can_scroll_horizontally()
                        && key_event.modifiers != KeyModifiers::CONTROL
                    {
                        // Scroll to end of line
                        self.file_horizontal_offset = self.file_horizontal_page_limit();
                        self.file_horizontal_scrollbar_state.last();
                    }
                }
                KeyCode::PageUp => {
                    if self.can_scroll_vertically() {
                        // Scroll up one page
                        let frame_height = self.inner_area.height as usize;
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
                        let frame_height = self.inner_area.height as usize;
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
        self.widest_line_len > self.inner_area.width as usize
    }

    fn can_scroll_vertically(&self) -> bool {
        self.file_text.len() > self.inner_area.height as usize
    }

    fn file_vertical_page_limit(&self) -> usize {
        page_limit(self.file_text.len(), self.inner_area.height as usize)
    }

    fn file_horizontal_page_limit(&self) -> usize {
        page_limit(self.widest_line_len, self.inner_area.width as usize)
    }

    fn set_scrollbar_state(&mut self) {
        let height = self.inner_area.height as usize;
        let len = self.file_folder_list.upper_bound() + 1;
        if len > height {
            self.folder_scrollbar_state = ScrollbarState::new(len)
                .position(0)
                .viewport_content_length(height);
        } else {
            // Hide unneeded scrollbar
            self.folder_scrollbar_state = ScrollbarState::default();
        }
    }

    fn set_horizontal_scrollbar_state(&mut self) {
        let line_width = self.widest_line_len;
        let frame_width = self.inner_area.width as usize;
        if line_width > self.inner_area.width as usize {
            self.file_horizontal_scrollbar_state = ScrollbarState::new(line_width)
                .position(0)
                .viewport_content_length(frame_width);
        } else {
            // Hide unneeded scrollbar
            self.file_horizontal_scrollbar_state = ScrollbarState::default();
        };
    }

    fn set_vertical_scrollbar_state(&mut self) {
        let height = self.inner_area.height as usize;
        let len = self.file_text.len();
        if len > height {
            self.file_vertical_scrollbar_state = ScrollbarState::new(len)
                .position(0)
                .viewport_content_length(height);
        } else {
            // Hide unneeded scrollbar
            self.file_vertical_scrollbar_state = ScrollbarState::default();
        }
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect) -> Result<(), std::io::Error> {
        self.set_area(area);

        if let Some(entry) = &self.entry {
            let block = if self.has_focus {
                util::focused_block()
            } else {
                util::default_block()
            };
            if let Some(PreviewItem::Error(message)) = &self.preview_item {
                self.render_file_error(message.to_string(), block, frame);
                return Ok(());
            }
            let metadata = entry.metadata()?;
            {
                let title = self.title(&metadata);
                let block = block.title(title);

                if let Some(file_contents) = &self.preview_item {
                    match file_contents {
                        PreviewItem::Folder => self.render_folder_list(block, frame),
                        PreviewItem::TextFile => self.render_text_file(block, frame),
                        PreviewItem::OversizeTextFile => {
                            self.render_oversize_text_file(block, frame)
                        }
                        PreviewItem::BinaryFile => self.render_binary_file(block, frame),
                        PreviewItem::Error(message) => {
                            self.render_file_error((&message).to_string(), block, frame)
                        }
                    }
                }
                Ok(())
            }
        } else {
            Ok(())
        }
    }

    fn title(&mut self, metadata: &Metadata) -> String {
        let size_string = if metadata.is_dir() {
            format!("{} items", self.file_folder_list.len())
        } else {
            metadata_size_string(metadata)
        };
        let title = format!("[{} - {}]", metadata_modified_string(metadata), size_string,);
        title
    }

    fn render_folder_list(&mut self, block: Block<'_>, frame: &mut Frame<'_>) {
        let items = util::list_items(&self.file_folder_list, self.inner_area.height as usize);
        let list = List::new(items);
        frame.render_widget(block, self.area);
        frame.render_stateful_widget(list, self.inner_area, &mut self.file_folder_list.state);

        let scrollbar = Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(
            scrollbar,
            self.area.inner(&Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut self.folder_scrollbar_state,
        );
    }

    fn render_text_file(&mut self, block: Block<'_>, frame: &mut Frame<'_>) {
        let items: Vec<Line> = self
            .file_text
            .iter()
            .map(|item| Line::from(item.to_string()))
            .collect();
        let paragraph = Paragraph::new(items.clone()).scroll((
            self.file_vertical_offset as u16,
            self.file_horizontal_offset as u16,
        ));
        frame.render_widget(block, self.area);
        frame.render_widget(paragraph, self.inner_area);

        let vertical_scrollbar =
            Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(
            vertical_scrollbar,
            self.area.inner(&Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut self.file_vertical_scrollbar_state,
        );

        let horizontal_scrollbar =
            Scrollbar::default().orientation(ScrollbarOrientation::HorizontalBottom);
        frame.render_stateful_widget(
            horizontal_scrollbar,
            self.area.inner(&Margin {
                vertical: 0,
                horizontal: 1,
            }),
            &mut self.file_horizontal_scrollbar_state,
        );
    }

    fn render_oversize_text_file(&mut self, block: Block<'_>, frame: &mut Frame<'_>) {
        frame.render_widget(block, self.area);

        frame.render_widget(
            Paragraph::new(" Oversize Text File (Max 50 kb) ")
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: false })
                .style(styles::OVERSIZE_FILE_STYLE),
            Rect::new(self.area.x + 2, self.area.y + 2, self.area.width - 4, 1),
        )
    }

    fn render_binary_file(&mut self, block: Block<'_>, frame: &mut Frame<'_>) {
        frame.render_widget(block, self.area);
        frame.render_widget(
            Paragraph::new(" Binary File ")
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: false })
                .style(styles::BINARY_FILE_STYLE),
            Rect::new(self.area.x + 2, self.area.y + 2, self.area.width - 4, 1),
        )
    }

    fn render_file_error(&mut self, message: String, block: Block<'_>, frame: &mut Frame<'_>) {
        frame.render_widget(block, self.area);
        frame.render_widget(
            Paragraph::new(Text::from(message.as_str()))
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

    fn set_area(&mut self, area: Rect) {
        self.area = area;
        // Give the content some horizontal padding
        self.inner_area = area.inner(&Margin {
            vertical: 1,
            horizontal: 2,
        });
    }
}

fn page_limit(total_size: usize, page_size: usize) -> usize {
    if total_size > page_size {
        total_size - page_size
    } else {
        0
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
