/*
 * Copyright (c) 2023-2024 Paul Sobolik
 * Created 2024-03-18
 */
use std::io;

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use crossterm::{
    event::KeyCode::Char,
    event::{KeyCode, KeyEvent, KeyModifiers},
};
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use crate::app::{
    components::directory::Directory, components::head::Head, components::preview::Preview,
    components::Component,
};
use crate::tui::Event;

mod components;
mod styles;

struct FrameSet {
    head: Rect,
    directory: Rect,
    preview: Rect,
}

#[derive(Default)]
pub struct App {
    pub should_quit: bool,
    fs_error: Option<io::Error>,
    area: Rect,

    // Components
    head: Head,
    directory: Directory,
    preview: Preview,
}

impl App {
    pub fn set_event_tx(&mut self, event_tx: Option<UnboundedSender<Event>>) {
        self.directory.set_event_tx(event_tx);
    }

    pub async fn handle_event(&mut self, event: Event) {
        match event {
            Event::Key(key_event) => self.handle_key_event(key_event).await,
            Event::Init(width, height) => self.handle_init_event(width, height).await,
            Event::Resize(width, height) => self.handle_resize_event(width, height),
            Event::Mouse(mouse_event) => self.handle_mouse_event(mouse_event).await,
            Event::SelectionChanged => self.load_selected_item().await,
            Event::DirectoryChanged => self.handle_directory_changed(),
            _ => {}
        }
    }

    async fn handle_init_event(&mut self, width: u16, height: u16) {
        self.handle_resize_event(width, height);

        if let Err(error) = self.directory.load_cwd().await {
            self.fs_error = Some(error);
        }
        self.load_selected_item().await;
        self.directory.set_focus(true);
        self.preview.set_focus(false);
    }

    fn handle_resize_event(&mut self, width: u16, height: u16) {
        self.area = Rect::new(0, 0, width, height);
        let frame_set = Self::calculate_frames(self.area);
        self.head.handle_resize_event(frame_set.head);
        self.directory.handle_resize_event(frame_set.directory);
        self.preview.handle_resize_event(frame_set.preview);
    }

    async fn maybe_clear_error(&mut self) -> bool {
        if self.fs_error.is_some() {
            // If there's an error pending, clear it.
            self.fs_error = None;
            // If the current item is not valid anymore, reload the current folder and selected item
            if let Some(path) = self.directory.selected_item() {
                if path.metadata().is_err() {
                    self.directory.load_cwd().await.unwrap();
                    self.load_selected_item().await;
                }
            }
            true
        } else {
            false
        }
    }

    async fn handle_mouse_event(&mut self, mouse_event: MouseEvent) {
        match mouse_event.kind {
            // If there's an error showing, any mouse event that we might handle will clear it
            // instead.
            MouseEventKind::Down(..) | MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                if self.maybe_clear_error().await {
                    return;
                }
            }
            // Ignore mouse events we don't handle. (Why not?)
            _ => return,
        }

        match mouse_event.kind {
            MouseEventKind::Down(mouse_button) => {
                if mouse_button == MouseButton::Left {
                    let directory_focused = self.directory.has_focus();
                    if self.directory.hit_test(mouse_event.column, mouse_event.row) {
                        if directory_focused {
                            // A left click on the selected item in the focused directory pane is
                            // converted into an Enter key event. A left click on an unselected item
                            // selects it.
                            if let Some(index) = self.directory.index_from_row(mouse_event.row) {
                                if self.directory.is_selected(index) {
                                    let key_event =
                                        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
                                    self.handle_key_event(key_event).await;
                                } else {
                                    self.directory.set_selected(index);
                                    self.load_selected_item().await;
                                }
                            }
                        } else {
                            self.focus_directory();
                        }
                    } else if self.preview.hit_test(mouse_event.column, mouse_event.row)
                        && directory_focused
                    {
                        self.focus_preview();
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                let key_event = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
                self.handle_key_event_by_position(key_event, mouse_event.column, mouse_event.row)
                    .await;
            }
            MouseEventKind::ScrollDown => {
                let key_event = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
                self.handle_key_event_by_position(key_event, mouse_event.column, mouse_event.row)
                    .await;
            }
            _ => { /* ignore */ }
        }
    }

    // Send a key event to the pane containing column/row
    async fn handle_key_event_by_position(&mut self, key_event: KeyEvent, column: u16, row: u16) {
        if self.directory.hit_test(column, row) {
            if let Err(error) = self.directory.handle_key_event(key_event).await {
                self.fs_error = Some(error);
            }
        } else if self.preview.hit_test(column, row) {
            if let Err(error) = self.preview.handle_key_event(key_event).await {
                self.fs_error = Some(error);
            }
        }
    }

    // Handle a key event, or send it to the focused pane
    async fn handle_key_event(&mut self, key_event: KeyEvent) {
        // Ctrl+C closes the app, regardless of state
        if Char('c') == key_event.code && key_event.modifiers == KeyModifiers::CONTROL {
            self.quit();
            return;
        }
        // If there is an error showing, clear it and don't process the event.
        if self.maybe_clear_error().await {
            return;
        }
        match key_event.code {
            KeyCode::Esc => self.quit(),
            KeyCode::Tab => self.toggle_focus(),
            _ => {
                if self.directory.has_focus() {
                    if let Err(error) = self.directory.handle_key_event(key_event).await {
                        self.fs_error = Some(error);
                    }
                } else if self.preview.has_focus() {
                    if let Err(error) = self.preview.handle_key_event(key_event).await {
                        self.fs_error = Some(error);
                    }
                }
            }
        }
    }

    fn quit(&mut self) {
        self.should_quit = true;
    }

    fn toggle_focus(&mut self) {
        if self.directory.has_focus() {
            self.focus_preview()
        } else {
            self.focus_directory()
        }
    }

    fn focus_directory(&mut self) {
        if !self.directory.has_focus() {
            self.directory.set_focus(true);
            self.preview.set_focus(false);
        }
    }

    fn focus_preview(&mut self) {
        if !self.preview.has_focus() {
            self.directory.set_focus(false);
            self.preview.set_focus(true);
        }
    }

    fn handle_directory_changed(&mut self) {
        match std::env::current_dir() {
            Ok(cwd) => self.head.set_path(Some(cwd)),
            Err(error) => {
                self.head.set_path(None);
                self.fs_error = Some(error);
            }
        }
    }

    async fn load_selected_item(&mut self) {
        self.preview
            .load_entry(self.directory.selected_item())
            .await;
    }

    pub fn render(&mut self, frame: &mut Frame<'_>) {
        self.head.render(frame);
        if let Err(error) = self.directory.render(frame) {
            self.fs_error = Some(error);
        }
        if let Err(error) = self.preview.render(frame) {
            self.fs_error = Some(error);
        }
        if let Some(fs_error) = &self.fs_error {
            self.render_error_popup(&fs_error.to_string(), frame, self.area);
        }
    }

    fn render_error_popup(&self, error: &str, frame: &mut Frame, frame_size: Rect) {
        let text = Paragraph::new(Text::from(error)).style(styles::ERROR_STYLE);
        let block = Block::default().title("Error").borders(Borders::ALL);

        let error_len = error.len() as u16;
        let area = Self::centered_rect(error_len + 4, 3, frame_size);
        let error_area = Self::centered_rect(error_len, 1, area);

        frame.render_widget(Clear, area); // This clears the background underneath the popup
        frame.render_widget(block, area);
        frame.render_widget(text, error_area);
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
}
