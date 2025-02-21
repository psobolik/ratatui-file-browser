/*
 * Copyright (c) 2024 Paul Sobolik
 * Created 2024-03-17
 */

use std::path::PathBuf;

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use crossterm::{
    event::KeyCode::Char,
    event::{KeyCode, KeyEvent},
};
use ratatui::layout::Position;
use ratatui::{layout::Rect, widgets::List, Frame};
use tokio::sync::mpsc::UnboundedSender;

use crate::app::{components, styles};
use crate::tui::Event;
use crate::{constants, stateful_list::StatefulList, util};

use super::Component;

#[derive(Default)]
pub struct Directory {
    items: StatefulList<PathBuf>,
    has_focus: bool,
    area: Rect,
    event_tx: Option<UnboundedSender<Event>>,
}

impl Component for Directory {
    fn set_area(&mut self, area: Rect) {
        self.area = area;
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
        match mouse_event.kind {
            MouseEventKind::Down(mouse_button) => {
                // A left click on the selected item is converted into an Enter key event.
                // A left click on an unselected item selects it.
                if mouse_button == MouseButton::Left {
                    if let Some(index) = self.index_from_row(mouse_event.row) {
                        if self.is_selected(index) {
                            let key_event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
                            self.handle_key_event(key_event).await?;
                        } else {
                            self.set_selected(index);
                            self.event_tx
                                .as_ref()
                                .unwrap()
                                .send(Event::SelectionChanged)
                                .expect("Panic sending selection changed event");
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                let key_event = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
                self.handle_key_event(key_event).await?;
            }
            MouseEventKind::ScrollDown => {
                let key_event = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
                self.handle_key_event(key_event).await?;
            }
            _ => { /* ignore */ }
        }
        Ok(())
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
        let current = self.get_cwd();

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
                // If there's a parent directory open it
                KeyCode::Backspace => {
                    if self.has_parent_directory() {
                        self.set_selected(0);
                        if self.cd()? {
                            selection_changed = true;
                            directory_changed = true;
                        }
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
            if let Ok(current) = current {
                if let Some(selected) = self.items.index_of(&current) {
                    self.set_selected(selected);
                }
            }
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

    fn render(&mut self, area: Rect, frame: &mut Frame) -> Result<(), std::io::Error> {
        self.set_area(area);

        let items = util::list_items(&self.items, frame.size().height as usize);
        // Don't include parent directory in count
        let mut item_count = self.items.len();
        if self.has_parent_directory() {
            item_count -= 1;
        }
        let item_count_string = format!("[{item_count} items]");
        let block = components::helpers::component_block(self.has_focus).title(item_count_string);
        let list = List::new(items)
            .block(block)
            .highlight_style(styles::LIST_HIGHLIGHT_STYLE);
        frame.render_stateful_widget(list, self.area, &mut self.items.state);

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
        let index = (row - self.area.y) as usize + self.items.state.offset();
        if (index > self.items.lower_bound()) && (index <= self.items.len()) {
            Some(index - 1)
        } else {
            None
        }
    }

    pub async fn load_cwd(&mut self) -> Result<(), std::io::Error> {
        let cwd = self.get_cwd()?;
        let entries = components::helpers::read_directory(&cwd).await?;
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

    fn get_cwd(&self) -> Result<PathBuf, std::io::Error> {
        // Gets the current directory, unless it doesn't exist (because it was deleted?)
        // Then gets the current directory's first valid parent instead.
        let mut cwd: Option<PathBuf> = None;
        while cwd.is_none() {
            if let Ok(cd) = std::env::current_dir() {
                cwd = Some(cd);
            } else {
                std::env::set_current_dir(constants::PARENT_DIRECTORY)?
            }
        }
        if let Some(cwd) = cwd {
            Ok(cwd)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Can't find valid directory",
            ))
        }
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
    
    fn has_parent_directory(&self) -> bool {
        util::entry_name(&self.items[0]) == constants::PARENT_DIRECTORY && self.items.len() > 0
    }
}
