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

    pub fn render(&mut self, area: Rect, frame: &mut Frame) {
        let text = if let Some(path) = &self.path {
            let path_str = util::entry_path(path.as_path());
            // Trim path string so that (with brackets) it will fit width of window
            util::clip_text(path_str.as_str(), (area.width - 2) as usize)
        } else {
            String::new()
        };
        frame.render_widget(Paragraph::new(format!("[{text}]")), area);
    }
}
