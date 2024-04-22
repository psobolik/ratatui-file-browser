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
    area: Rect,
}

impl Head {
    pub fn set_path(&mut self, path: Option<PathBuf>) {
        self.path = path;
    }

    pub fn handle_resize_event(&mut self, rect: Rect) {
        self.area = rect;
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let text = if let Some(path) = &self.path {
            util::entry_path(path.as_path())
        } else {
            String::new()
        };
        let text = format!("[{text}]");
        frame.render_widget(
            Paragraph::new(util::clip_string(&text, self.area.width as usize)),
            self.area,
        );
    }
}
