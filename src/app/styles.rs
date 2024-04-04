/*
 * Copyright (c) 2024 Paul Sobolik
 * Created 2024-03-18
 */

use ratatui::prelude::{Color, Style};

pub(crate) const OTHER_FILE_STYLE: Style = Style::new().fg(Color::Black).bg(Color::Yellow);
pub(crate) const OVERSIZE_FILE_STYLE: Style = Style::new().fg(Color::Black).bg(Color::Yellow);
pub(crate) const BINARY_FILE_STYLE: Style = Style::new().fg(Color::Black).bg(Color::Yellow);
pub(crate) const ERROR_STYLE: Style = Style::new().fg(Color::Red);
pub(crate) const LIST_HIGHLIGHT_STYLE: Style = Style::new().fg(Color::Black).bg(Color::Gray);
