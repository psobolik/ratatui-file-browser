/*
 * Copyright (c) 2024 Paul Sobolik
 * Created 2024-04-03
 */

use std::io::Error;
use std::path::{Path, PathBuf};

use ratatui::layout::{Alignment, Rect};
use ratatui::prelude::Style;
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::util;

use super::preview_pane;

pub trait MessagePane {
    fn init(&mut self, entry: Option<&PathBuf>);
    fn clear(&mut self) {
        self.init(None)
    }

    fn render_message(
        entry: &Path,
        message: &str,
        has_focus: bool,
        style: Style,
        frame: &mut Frame<'_>,
        area: Rect,
    ) -> Result<(), Error> {
        let block = if has_focus {
            util::focused_block()
        } else {
            util::default_block()
        };
        let metadata = entry.metadata()?;
        let title = preview_pane::file_title(&metadata);
        let block = block.title(title);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(message)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: false })
                .style(style),
            Rect::new(area.x + 2, area.y + 2, area.width - 4, 1),
        );
        Ok(())
    }
}
