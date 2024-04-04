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
pub(super) struct Other {
    // The file's directory entry
    entry: Option<PathBuf>,
}

impl MessagePane for Other {
    fn init(&mut self, entry: Option<&PathBuf>) {
        self.entry = entry.cloned();
    }
}

impl PreviewPane for Other {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, has_focus: bool) -> Result<(), Error> {
        if let Some(entry) = &self.entry {
            <Self as MessagePane>::render_message(
                entry,
                "Unsupported File Type",
                has_focus,
                styles::OTHER_FILE_STYLE,
                frame,
                area,
            )?;
        }
        Ok(())
    }
}
