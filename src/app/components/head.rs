/*
 * Copyright (c) 2024 Paul Sobolik
 * Created 2024-03-17
 */

use crate::util;
use ratatui::{layout::Rect, widgets::Paragraph, Frame};
use std::path::PathBuf;

#[derive(Default)]
pub struct Head {
    path: Option<PathBuf>,
}

impl Head {
    pub fn set_path(&mut self, path: Option<PathBuf>) {
        self.path = path;
    }

    pub fn handle_resize_event(&mut self, _rect: Rect) {
        // Do nothing
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let text = if let Some(path) = &self.path {
            util::entry_path(path.as_path())
        } else {
            String::new()
        };
        let text = format!("[{text}]");
        frame.render_widget(
            Paragraph::new(util::clip_string(&text, area.width as usize)),
            area,
        );
    }
}
