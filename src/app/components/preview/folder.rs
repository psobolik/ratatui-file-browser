/*
 * Copyright (c) 2024 Paul Sobolik
 * Created 2024-04-03
 */

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Margin, Position, Rect};
use ratatui::widgets::{List, Scrollbar, ScrollbarOrientation, ScrollbarPosition, ScrollbarState};
use ratatui::Frame;

use crate::stateful_list::StatefulList;
use crate::util;

use super::components;
use super::list_pane::ListPane;
use super::preview_pane;
use super::preview_pane::PreviewPane;

#[derive(Default)]
pub(super) struct Folder<'a> {
    area: Rect,
    inner_area: Rect,

    // The folder's directory entry
    entry: Option<PathBuf>,

    // The folder's contents
    entry_list: StatefulList<PathBuf>,

    // Scrollbar stuff
    scrollbar: Scrollbar<'a>,
    scrollbar_state: ScrollbarState,
    scrollbar_area: Rect,
}

impl ListPane<PathBuf> for Folder<'_> {
    fn init(&mut self, entry: Option<&PathBuf>, items: Vec<PathBuf>, area: Rect) {
        self.set_area(area);

        self.entry = entry.cloned();
        self.entry_list = StatefulList::with_items(items);

        self.scrollbar = Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);
        self.set_scrollbar_state();
    }

    fn clear(&mut self) {
        self.entry = None;
        self.entry_list = StatefulList::with_items(vec![]);

        self.set_scrollbar_state();
    }

    fn handle_mouse_event(&mut self, mouse_event: MouseEvent) {
        match mouse_event.kind {
            MouseEventKind::Down(mouse_button) => {
                if mouse_button == MouseButton::Left {
                    let position = Position {
                        x: mouse_event.column,
                        y: mouse_event.row,
                    };
                    match self.scrollbar.hit_test(
                        position,
                        self.scrollbar_area,
                        &self.scrollbar_state,
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
            // Scroll up one line
            if !self.entry_list.at_offset_first() {
                self.entry_list.previous_offset();
                self.scrollbar_state.prev();
            }
        } else if util::is_down_key(key_event) {
            // Scroll down one line
            if self.entry_list.offset() < self.vertical_page_limit() {
                self.entry_list.next_offset();
                self.scrollbar_state.next();
            } else {
                self.scrollbar_state.last();
            }
        } else {
            match key_event.code {
                KeyCode::Home => {
                    // Scroll to top of list
                    if !self.entry_list.at_offset_first() {
                        self.entry_list.offset_first();
                        self.scrollbar_state.first();
                    }
                }
                KeyCode::End => {
                    // Scroll to end of list
                    if self.entry_list.len() > self.inner_area.height as usize {
                        self.entry_list.set_offset(self.vertical_page_limit());
                        self.scrollbar_state.last();
                    }
                }
                KeyCode::PageUp => {
                    // Scroll up one page
                    let frame_height = self.inner_area.height as usize;
                    if self.entry_list.offset() > frame_height {
                        self.entry_list
                            .set_offset(self.entry_list.offset() - frame_height);
                        self.sync_scrollbar_position();
                    } else {
                        self.entry_list.offset_first();
                        self.scrollbar_state.first();
                    };
                }
                KeyCode::PageDown => {
                    // Scroll down one page
                    let frame_height = self.inner_area.height as usize;
                    let max_offset = self.vertical_page_limit();
                    let offset = self.entry_list.offset() + frame_height;
                    if offset < max_offset {
                        self.entry_list.set_offset(offset);
                        self.sync_scrollbar_position();
                    } else {
                        self.entry_list.set_offset(max_offset);
                        self.scrollbar_state.last();
                    };
                }
                _ => {}
            }
        }
    }

    fn set_area(&mut self, area: Rect) {
        self.area = area;
        // Give the content some horizontal padding
        self.inner_area = area.inner(Margin {
            vertical: 1,
            horizontal: 2,
        });
        self.scrollbar_area = area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        });
        self.set_scrollbar_state();
    }
}

impl PreviewPane for Folder<'_> {
    fn render(
        &mut self,
        area: Rect,
        frame: &mut Frame<'_>,
        has_focus: bool,
    ) -> Result<(), std::io::Error> {
        self.set_area(area);

        if let Some(entry) = &self.entry {
            let title = preview_pane::folder_title(entry, self.entry_list.len())?;
            let block = components::helpers::component_block(has_focus).title(title);

            let items = util::list_items(&self.entry_list, self.inner_area.height as usize);
            let list = List::new(items);
            frame.render_widget(block, self.area);
            frame.render_stateful_widget(list, self.inner_area, &mut self.entry_list.state);

            frame.render_stateful_widget(
                self.scrollbar.clone(),
                self.scrollbar_area,
                &mut self.scrollbar_state,
            );
        }
        Ok(())
    }

    fn page_limit(total_size: usize, page_size: usize) -> usize {
        total_size.saturating_sub(page_size)
    }
}

impl Folder<'_> {
    fn vertical_page_limit(&self) -> usize {
        <Self as PreviewPane>::page_limit(self.entry_list.len(), self.inner_area.height as usize)
    }

    fn sync_scrollbar_position(&mut self) {
        self.scrollbar_state = self.scrollbar_state.position(self.entry_list.offset());
    }

    fn set_scrollbar_state(&mut self) {
        let frame_length = self.inner_area.height as usize;
        if self.entry_list.len() <= frame_length {
            // Hide scrollbar
            self.scrollbar_state = self.scrollbar_state.position(0).content_length(0);
            self.entry_list.first();
        } else {
            // Show scrollbar
            self.scrollbar_state = self
                .scrollbar_state
                .content_length(self.entry_list.len() - frame_length)
                .viewport_content_length(frame_length);
        };
    }
}
