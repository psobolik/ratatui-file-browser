/*
 * Copyright (c) 2024 Paul Sobolik
 * Created 2024-04-02
 */

use std::io::Error;
use std::path::PathBuf;

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app::styles;

use super::message_pane::MessagePane;
use super::preview_pane::PreviewPane;

#[derive(Default)]
pub(super) struct Binary {
    // The file's directory entry
    entry: Option<PathBuf>,
    area: Rect,
}

impl MessagePane for Binary {
    fn init(&mut self, entry: Option<&PathBuf>) {
        self.entry = entry.cloned();
    }
}

impl PreviewPane for Binary {
    fn render(&mut self, frame: &mut Frame<'_>, has_focus: bool) -> Result<(), Error> {
        if let Some(entry) = &self.entry {
            <Self as MessagePane>::render_message(
                entry,
                "Binary File",
                has_focus,
                styles::BINARY_FILE_STYLE,
                frame,
                self.area,
            )?;
        }
        Ok(())
    }

    fn handle_resize_event(&mut self, rect: Rect) {
        self.area = rect;
    }
}
