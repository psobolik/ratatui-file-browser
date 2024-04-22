/*
 * Copyright (c) 2024 Paul Sobolik
 * Created 2024-04-03
 */

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Margin, Rect};
use ratatui::widgets::{List, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

use crate::stateful_list::StatefulList;
use crate::util;

use super::components;
use super::list_pane::ListPane;
use super::preview_pane;
use super::preview_pane::PreviewPane;

#[derive(Default)]
pub(super) struct Folder {
    area: Rect,
    inner_area: Rect,

    // The folder's directory entry
    entry: Option<PathBuf>,

    // The folder's contents
    entry_list: StatefulList<PathBuf>,
    scrollbar_state: ScrollbarState,
}

impl ListPane<PathBuf> for Folder {
    fn init(&mut self, entry: Option<&PathBuf>, items: Vec<PathBuf>) {
        self.entry = entry.cloned();
        self.entry_list = StatefulList::with_items(items);
        self.set_scrollbar_state();
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if util::is_up_key(key_event) {
            // Scroll up one line
            if !self.entry_list.at_offset_first() {
                self.entry_list.previous_offset();
                self.sync_scrollbar_position();
            }
        } else if util::is_down_key(key_event) {
            // Scroll down one line
            if self.entry_list.offset() < self.vertical_page_limit() {
                self.entry_list.next_offset();
                self.sync_scrollbar_position();
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
        self.inner_area = area.inner(&Margin {
            vertical: 1,
            horizontal: 2,
        });
    }
}

impl PreviewPane for Folder {
    fn render(&mut self, frame: &mut Frame<'_>, has_focus: bool) -> Result<(), std::io::Error> {
        if let Some(entry) = &self.entry {
            let title = preview_pane::folder_title(entry, self.entry_list.len())?;
            let block = components::component_block(has_focus).title(title);

            let items = util::list_items(&self.entry_list, self.inner_area.height as usize);
            let list = List::new(items);
            frame.render_widget(block, self.area);
            frame.render_stateful_widget(list, self.inner_area, &mut self.entry_list.state);

            let scrollbar = Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);
            frame.render_stateful_widget(
                scrollbar,
                self.area.inner(&Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut self.scrollbar_state,
            );
        }
        Ok(())
    }
    fn handle_resize_event(&mut self, rect: Rect) {
        self.set_area(rect);
        self.set_scrollbar_state();
    }
}

impl Folder {
    fn vertical_page_limit(&self) -> usize {
        <Self as PreviewPane>::page_limit(self.entry_list.len(), self.inner_area.height as usize)
    }

    fn sync_scrollbar_position(&mut self) {
        self.scrollbar_state = self.scrollbar_state.position(self.entry_list.offset());
    }

    fn set_scrollbar_state(&mut self) {
        let height = self.inner_area.height as usize;
        let len = if self.entry_list.len() <= height {
            0
        } else {
            self.entry_list.len()
        };
        self.scrollbar_state = ScrollbarState::new(len)
            .position(0)
            .viewport_content_length(height);
    }
}
