/*
 * Copyright (c) 2024 Paul Sobolik
 * Created 2024-04-03
 */

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Margin, Rect};
use ratatui::prelude::Line;
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

use crate::util;

use super::components;
use super::list_pane::ListPane;
use super::preview_pane;
use super::preview_pane::PreviewPane;

#[derive(Default)]
pub(super) struct Text {
    area: Rect,
    inner_area: Rect,

    // The file's directory entry
    entry: Option<PathBuf>,

    // The file's contents
    file_text: Vec<String>,

    widest_line_len: usize,
    file_horizontal_scrollbar_state: ScrollbarState,
    file_vertical_scrollbar_state: ScrollbarState,
    file_horizontal_offset: usize,
    file_vertical_offset: usize,
}

impl ListPane<String> for Text {
    fn init(&mut self, entry: Option<&PathBuf>, lines: Vec<String>, area: Rect) {
        self.set_area(area);

        self.entry = entry.cloned();
        self.widest_line_len = Self::widest_line_length(&lines);
        self.file_text = lines;

        self.set_horizontal_scrollbar_state();
        self.set_vertical_scrollbar_state();
    }

    fn clear(&mut self) {
        self.entry = None;
        self.file_text = vec![];

        self.set_horizontal_scrollbar_state();
        self.set_vertical_scrollbar_state();
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if util::is_up_key(key_event) {
            if self.can_scroll_vertically() && self.file_vertical_offset > 0 {
                // Scroll up one line
                self.file_vertical_offset -= 1;
                self.file_vertical_scrollbar_state.prev();
            }
        } else if util::is_down_key(key_event) {
            if self.can_scroll_vertically() {
                // Scroll down one line
                if self.file_vertical_offset < self.vertical_page_limit() {
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
                        self.file_vertical_offset = self.vertical_page_limit();
                        self.file_vertical_scrollbar_state.last();
                    } else if self.can_scroll_horizontally()
                        && key_event.modifiers != KeyModifiers::CONTROL
                    {
                        // Scroll to end of line
                        self.file_horizontal_offset = self.horizontal_page_limit();
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
                        let limit = self.vertical_page_limit();
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
                        && self.file_horizontal_offset < self.horizontal_page_limit()
                    {
                        self.file_horizontal_offset += 1;
                        self.file_horizontal_scrollbar_state.next();
                    }
                }
                _ => {}
            }
        }
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

impl PreviewPane for Text {
    fn render(
        &mut self,
        area: Rect,
        frame: &mut Frame<'_>,
        has_focus: bool,
    ) -> Result<(), std::io::Error> {
        self.set_area(area);

        if let Some(entry) = &self.entry {
            let title = preview_pane::file_title(entry)?;
            let block = components::component_block(has_focus).title(title);

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
        Ok(())
    }
}
impl Text {
    fn can_scroll_horizontally(&self) -> bool {
        self.widest_line_len > self.inner_area.width as usize
    }

    fn can_scroll_vertically(&self) -> bool {
        self.file_text.len() > self.inner_area.height as usize
    }

    fn vertical_page_limit(&self) -> usize {
        <Self as PreviewPane>::page_limit(self.file_text.len(), self.inner_area.height as usize)
    }

    fn horizontal_page_limit(&self) -> usize {
        <Self as PreviewPane>::page_limit(self.widest_line_len, self.inner_area.width as usize)
    }

    fn set_horizontal_scrollbar_state(&mut self) {
        let frame_width = self.inner_area.width as usize;
        let line_width = if self.widest_line_len <= frame_width {
            0
        } else {
            self.widest_line_len
        };
        self.file_horizontal_scrollbar_state = ScrollbarState::new(line_width)
            .position(0)
            .viewport_content_length(frame_width);
    }

    fn set_vertical_scrollbar_state(&mut self) {
        let height = self.inner_area.height as usize;
        let len = if self.file_text.len() <= height {
            0
        } else {
            self.file_text.len()
        };
        self.file_vertical_scrollbar_state = ScrollbarState::new(len)
            .position(0)
            .viewport_content_length(height);
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
}
