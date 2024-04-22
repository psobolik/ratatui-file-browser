/*
 * Copyright (c) 2024 Paul Sobolik
 * Created 2024-04-03
 */

use std::path::PathBuf;

use crossterm::event::KeyEvent;
use ratatui::layout::Rect;

pub trait ListPane<T> {
    fn init(&mut self, entry: Option<&PathBuf>, items: Vec<T>, area: Rect);

    fn clear(&mut self);

    fn handle_key_event(&mut self, key_event: KeyEvent);

    fn set_area(&mut self, area: Rect);
}
