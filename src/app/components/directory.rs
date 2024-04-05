/*
 * Copyright (c) 2024 Paul Sobolik
 * Created 2024-03-17
 */

use crate::app::components::Component;
use crate::app::{components, styles};
use crate::tui::Event;
use crate::{constants, stateful_list::StatefulList, util};
use crossterm::{
    event::KeyCode::Char,
    event::{KeyCode, KeyEvent},
};
use ratatui::{layout::Rect, widgets::List, Frame};
use std::path::PathBuf;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Default)]
pub struct Directory {
    items: StatefulList<PathBuf>,
    has_focus: bool,
    area: Rect,
    event_tx: Option<UnboundedSender<Event>>,
}

impl Component for Directory {
    fn has_focus(&self) -> bool {
        self.has_focus
    }

    fn set_focus(&mut self, focus: bool) {
        self.has_focus = focus;
    }

    fn hit_test(&self, x: u16, y: u16) -> bool {
        util::is_in_rect(x, y, self.area)
    }

    fn handle_resize_event(&mut self, area: Rect) {
        self.area = area;
    }

    async fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<(), std::io::Error> {
        // If nothing is selected, select the first item before processing the key
        if self.items.selected().is_none() {
            self.items.set_selected(Some(0));
            // Don't process the Down key, though, so the first item stays selected in that case
            if util::is_down_key(key_event) {
                return Ok(());
            }
        }
        let mut selection_changed = false;
        let mut directory_changed = false;

        if util::is_up_key(key_event) {
            // Move selection up one entry
            selection_changed = self.items.previous();
        } else if util::is_down_key(key_event) {
            // Move selection down one entry
            selection_changed = self.items.next();
        } else {
            match key_event.code {
                KeyCode::Home => {
                    // Move selection to first entry
                    selection_changed = self.items.first();
                }
                KeyCode::End => {
                    // Move selection to last entry
                    selection_changed = self.items.last();
                }
                KeyCode::PageUp => {
                    // Move selection up one page
                    selection_changed = self.items.retreat(self.area.height as usize);
                }
                KeyCode::PageDown => {
                    // Move selection down one page
                    selection_changed = self.items.advance(self.area.height as usize)
                }
                // Open selected item if it's a folder
                KeyCode::Enter => {
                    if self.cd()? {
                        selection_changed = true;
                        directory_changed = true;
                    }
                }
                key_code => {
                    // Move selection to item starting with character
                    if let Char(c) = key_code {
                        self.select_by_char(c);
                        selection_changed = true;
                    }
                }
            };
        }
        if directory_changed {
            self.load_cwd().await?;
        }
        if selection_changed {
            self.event_tx
                .as_ref()
                .unwrap()
                .send(Event::SelectionChanged)
                .expect("Panic sending selection changed event");
        }
        Ok(())
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) -> Result<(), std::io::Error> {
        self.area = area;

        let items = util::list_items(&self.items, frame.size().height as usize);
        // Don't include parent directory in count
        let mut item_count = self.items.len();
        if (util::entry_name(&self.items[0]) == constants::PARENT_DIRECTORY) && item_count > 0 {
            item_count -= 1;
        }
        let item_count_string = format!("[{item_count} items]");
        let block = if self.has_focus {
            util::focused_block()
        } else {
            util::default_block()
        }
        .title(item_count_string);
        let list = List::new(items)
            .block(block)
            .highlight_style(styles::LIST_HIGHLIGHT_STYLE);
        frame.render_stateful_widget(list, area, &mut self.items.state);

        Ok(())
    }
}

impl Directory {
    pub fn set_event_tx(&mut self, event_tx: Option<UnboundedSender<Event>>) {
        self.event_tx = event_tx;
    }

    pub fn set_items(&mut self, items: Vec<PathBuf>) -> &mut Directory {
        self.items = StatefulList::with_items(items);
        self.items.first(); // Because no line is selected by default
        self
    }

    pub fn is_selected(&self, index: usize) -> bool {
        match self.items.state.selected() {
            Some(selected) => selected == index,
            None => false,
        }
    }

    pub fn index_from_row(&self, row: u16) -> Option<usize> {
        let index = (row - self.area.y - 1) as usize + self.items.state.offset();
        if (index >= self.items.lower_bound()) && (index <= self.items.upper_bound()) {
            Some(index)
        } else {
            None
        }
    }

    pub async fn load_cwd(&mut self) -> Result<(), std::io::Error> {
        let cwd = std::env::current_dir()?;
        let entries = components::read_directory(&cwd).await?;
        let mut result = vec![];
        // Prepend parent directory entry if there is one
        if cwd.parent().is_some() {
            let mut p = cwd.clone();
            p.push(constants::PARENT_DIRECTORY);
            result.push(p);
        }
        result.extend(entries);
        self.set_items(result);
        self.event_tx
            .as_ref()
            .unwrap()
            .send(Event::DirectoryChanged)
            .expect("Panic sending directory changed event");
        Ok(())
    }

    fn cd(&mut self) -> Result<bool, std::io::Error> {
        if let Some(selected) = self.selected_item() {
            if selected.is_dir() {
                std::env::set_current_dir(selected)?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn set_selected(&mut self, selected: usize) -> bool {
        if Some(selected) == self.items.selected() {
            false
        } else {
            self.items.set_selected(Some(selected));
            true
        }
    }

    fn select_by_char(&mut self, ch: char) -> bool {
        let selected = self.items.selected().unwrap_or(0);

        let index =
            util::find_match_by_char(self.items.iter().as_slice(), ch, selected, |path_buf| {
                // This returns the first character of the path's file name if it can
                if let Some(file_name) = path_buf.file_name() {
                    if let Some(file_name) = file_name.to_str() {
                        file_name.chars().next()
                    } else {
                        None
                    }
                } else {
                    None
                }
            });
        // Don't change the selection unless a match was made
        if let Some(index) = index {
            self.set_selected(index)
        } else {
            false
        }
    }

    pub fn selected_item(&self) -> Option<PathBuf> {
        self.items
            .selected()
            .map(|selected| self.items[selected].clone())
    }
}
