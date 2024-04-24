/*
 * Copyright (c) 2024 Paul Sobolik
 * Created 2024-04-03
 */

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Margin, Position, Rect};
use ratatui::prelude::Line;
use ratatui::widgets::{
    Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarPosition, ScrollbarState,
};
use ratatui::Frame;

use crate::util;

use super::components;
use super::list_pane::ListPane;
use super::preview_pane;
use super::preview_pane::PreviewPane;

#[derive(Default)]
pub(super) struct Text<'a> {
    area: Rect,
    inner_area: Rect,

    // The file's directory entry
    entry: Option<PathBuf>,

    // The file's contents
    file_text: Vec<String>,

    // Horizontal scrollbar stuff
    widest_line_len: usize,
    horizontal_scrollbar: Scrollbar<'a>,
    horizontal_scrollbar_state: ScrollbarState,
    horizontal_scrollbar_area: Rect,
    horizontal_offset: usize,

    // Vertical scrollbar stuff
    vertical_scrollbar: Scrollbar<'a>,
    vertical_scrollbar_state: ScrollbarState,
    vertical_scrollbar_area: Rect,
    vertical_offset: usize,
}

impl<'a> ListPane<String> for Text<'a> {
    fn init(&mut self, entry: Option<&PathBuf>, lines: Vec<String>, area: Rect) {
        self.set_area(area);

        self.entry = entry.cloned();
        self.widest_line_len = Self::widest_line_length(&lines);
        self.file_text = lines;

        self.vertical_scrollbar =
            Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);
        self.horizontal_scrollbar =
            Scrollbar::default().orientation(ScrollbarOrientation::HorizontalBottom);
        self.set_horizontal_scrollbar_state();
        self.set_vertical_scrollbar_state();
    }

    fn clear(&mut self) {
        self.entry = None;
        self.file_text = vec![];

        self.set_horizontal_scrollbar_state();
        self.set_vertical_scrollbar_state();
    }

    fn handle_mouse_event(&mut self, mouse_event: MouseEvent) {
        match mouse_event.kind {
            MouseEventKind::Down(mouse_button) => {
                if mouse_button == MouseButton::Left {
                    let position = Position {
                        x: mouse_event.column,
                        y: mouse_event.row,
                    };

                    match self.vertical_scrollbar.hit_test(
                        position,
                        self.vertical_scrollbar_area,
                        &self.vertical_scrollbar_state,
                    ) {
                        None => {}
                        Some(scrollbar_position) => {
                            match scrollbar_position {
                                ScrollbarPosition::Begin => self.handle_key_event(KeyEvent::new(
                                    KeyCode::Up,
                                    KeyModifiers::NONE,
                                )),
                                ScrollbarPosition::TrackLow => self.handle_key_event(
                                    KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
                                ),
                                // ScrollbarPosition::Thumb => {}
                                ScrollbarPosition::TrackHigh => self.handle_key_event(
                                    KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
                                ),
                                ScrollbarPosition::End => self.handle_key_event(KeyEvent::new(
                                    KeyCode::Down,
                                    KeyModifiers::NONE,
                                )),
                                _ => {}
                            }
                        }
                    }
                    match self.horizontal_scrollbar.hit_test(
                        position,
                        self.horizontal_scrollbar_area,
                        &self.horizontal_scrollbar_state,
                    ) {
                        None => {}
                        Some(scrollbar_position) => {
                            match scrollbar_position {
                                ScrollbarPosition::Begin => self.handle_key_event(KeyEvent::new(
                                    KeyCode::Left,
                                    KeyModifiers::NONE,
                                )),
                                ScrollbarPosition::TrackLow => self.handle_key_event(
                                    KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL),
                                ),
                                // ScrollbarPosition::Thumb => {}
                                ScrollbarPosition::TrackHigh => self.handle_key_event(
                                    KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL),
                                ),
                                ScrollbarPosition::End => self.handle_key_event(KeyEvent::new(
                                    KeyCode::Right,
                                    KeyModifiers::NONE,
                                )),
                                _ => {}
                            }
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                let key_event = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
                self.handle_key_event(key_event);
            }
            MouseEventKind::ScrollDown => {
                let key_event = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
                self.handle_key_event(key_event);
            }
            _ => { /* ignore */ }
        }
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if util::is_up_key(key_event) {
            if self.can_scroll_vertically() && self.vertical_offset > 0 {
                // Scroll up one line
                self.vertical_offset -= 1;
                self.vertical_scrollbar_state.prev();
            }
        } else if util::is_down_key(key_event) {
            if self.can_scroll_vertically() {
                // Scroll down one line
                if self.vertical_offset < self.vertical_page_limit() {
                    self.vertical_offset += 1;
                    self.vertical_scrollbar_state.next();
                }
            }
        } else {
            match key_event.code {
                KeyCode::Home => {
                    if self.can_scroll_vertically() && key_event.modifiers == KeyModifiers::CONTROL
                    {
                        // Scroll to top of file
                        self.vertical_offset = 0;
                        self.vertical_scrollbar_state.first();
                    } else if self.can_scroll_horizontally()
                        && key_event.modifiers != KeyModifiers::CONTROL
                    {
                        // Go to beginning of line
                        self.horizontal_offset = 0;
                        self.horizontal_scrollbar_state.first();
                    }
                }
                KeyCode::End => {
                    if self.can_scroll_vertically() && key_event.modifiers == KeyModifiers::CONTROL
                    {
                        // Scroll to bottom of file
                        self.vertical_offset = self.vertical_page_limit();
                        self.vertical_scrollbar_state.last();
                    } else if self.can_scroll_horizontally()
                        && key_event.modifiers != KeyModifiers::CONTROL
                    {
                        // Scroll to end of line
                        self.horizontal_offset = self.horizontal_page_limit();
                        self.horizontal_scrollbar_state.last();
                    }
                }
                KeyCode::PageUp => {
                    if self.can_scroll_vertically() {
                        // Scroll up one page
                        let frame_height = self.inner_area.height as usize;
                        if self.vertical_offset > frame_height {
                            self.vertical_offset -= frame_height;
                            self.vertical_scrollbar_state =
                                self.vertical_scrollbar_state.position(self.vertical_offset);
                        } else {
                            self.vertical_offset = 0;
                            self.vertical_scrollbar_state.first();
                        }
                    }
                }
                KeyCode::PageDown => {
                    if self.can_scroll_vertically() {
                        // Scroll down one page
                        let frame_height = self.inner_area.height as usize;
                        let limit = self.vertical_page_limit();
                        if self.vertical_offset + frame_height < limit {
                            self.vertical_offset += frame_height;
                            self.vertical_scrollbar_state =
                                self.vertical_scrollbar_state.position(self.vertical_offset);
                        } else {
                            self.vertical_offset = limit;
                            self.vertical_scrollbar_state.last();
                        }
                    }
                }
                KeyCode::Left => {
                    if self.can_scroll_horizontally()
                        && key_event.modifiers == KeyModifiers::CONTROL
                    {
                        // Scroll left one page
                        let frame_width = self.inner_area.width as usize;
                        if self.horizontal_offset > frame_width {
                            self.horizontal_offset -= frame_width;
                            self.horizontal_scrollbar_state = self
                                .horizontal_scrollbar_state
                                .position(self.horizontal_offset);
                        } else {
                            self.horizontal_offset = 0;
                            self.horizontal_scrollbar_state.first();
                        }
                    } else if self.can_scroll_horizontally()
                        && key_event.modifiers != KeyModifiers::CONTROL
                        && self.horizontal_offset > 0
                    {
                        self.horizontal_offset -= 1;
                        self.horizontal_scrollbar_state.prev();
                    }
                }
                KeyCode::Right => {
                    if self.can_scroll_horizontally()
                        && key_event.modifiers == KeyModifiers::CONTROL
                    {
                        // Scroll right one page
                        let frame_width = self.inner_area.width as usize;
                        let limit = self.horizontal_page_limit();
                        if self.horizontal_offset + frame_width < limit {
                            self.horizontal_offset += frame_width;
                            self.horizontal_scrollbar_state = self
                                .horizontal_scrollbar_state
                                .position(self.horizontal_offset);
                        } else {
                            self.horizontal_offset = limit;
                            self.horizontal_scrollbar_state.last();
                        }
                    } else if self.can_scroll_horizontally()
                        && key_event.modifiers != KeyModifiers::CONTROL
                        && self.horizontal_offset < self.horizontal_page_limit()
                    {
                        // Scroll right one character
                        self.horizontal_offset += 1;
                        self.horizontal_scrollbar_state.next();
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
        self.vertical_scrollbar_area = area.inner(&Margin {
            vertical: 1,
            horizontal: 0,
        });
        self.horizontal_scrollbar_area = self.area.inner(&Margin {
            vertical: 0,
            horizontal: 1,
        });
    }
}

impl<'a> PreviewPane for Text<'a> {
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
            let paragraph = Paragraph::new(items.clone())
                .scroll((self.vertical_offset as u16, self.horizontal_offset as u16));
            frame.render_widget(block, self.area);
            frame.render_widget(paragraph, self.inner_area);

            frame.render_stateful_widget(
                self.vertical_scrollbar.clone(),
                self.vertical_scrollbar_area,
                &mut self.vertical_scrollbar_state,
            );

            frame.render_stateful_widget(
                self.horizontal_scrollbar.clone(),
                self.horizontal_scrollbar_area,
                &mut self.horizontal_scrollbar_state,
            );
        }
        Ok(())
    }
}
impl<'a> Text<'a> {
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
        let frame_length = self.inner_area.width as usize;
        let content_length = if self.widest_line_len <= frame_length {
            0
        } else {
            self.widest_line_len - frame_length
        };
        self.horizontal_scrollbar_state = ScrollbarState::new(content_length)
            .position(0)
            .viewport_content_length(frame_length);
    }

    fn set_vertical_scrollbar_state(&mut self) {
        let frame_length = self.inner_area.height as usize;
        let content_length = if self.file_text.len() <= frame_length {
            0
        } else {
            self.file_text.len() - frame_length
        };
        self.vertical_scrollbar_state = ScrollbarState::new(content_length)
            .position(0)
            .viewport_content_length(frame_length);
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
