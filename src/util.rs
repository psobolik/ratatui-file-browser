/*
 * Copyright (c) 2024 Paul Sobolik
 * Created 2024-03-17
 */

use std::path::{Path, PathBuf};

use crossterm::{
    event::KeyCode::Char,
    event::{KeyCode, KeyEvent, KeyModifiers},
};
use ratatui::{prelude::Line, widgets::ListItem};

use crate::{constants, stateful_list::StatefulList};

pub fn clip_string(string: &String, width: usize) -> String {
    if string.len() > width {
        let start = string.len() - width + 1;
        format!("…{}", &string[start..])
    } else {
        string.to_string()
    }
}

pub fn entry_path(path: &Path) -> String {
    if path.ends_with(constants::PARENT_DIRECTORY) {
        let mut pb = path.to_path_buf();
        pb.pop();
        pb = PathBuf::from(entry_path(pb.as_path()));
        pb.push(constants::PARENT_DIRECTORY);
        pb.to_str().unwrap().to_string()
    } else if let Some(path_string) = path.to_str() {
        path_string.to_string()
    } else {
        String::default() // Path has no name or it can't be converted
    }
}

pub fn list_items<'a>(paths: &StatefulList<PathBuf>, height: usize) -> Vec<ListItem<'a>> {
    let offset = paths.state.offset();
    paths
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            if index < offset || index > offset + height {
                ListItem::new("") // Off screen
            } else {
                ListItem::new(Line::from(format!(
                    "{} {}",
                    path_icon(entry),
                    entry_name(entry)
                )))
            }
        })
        .collect()
}

pub(crate) fn entry_name(entry: &Path) -> String {
    if entry.ends_with(constants::PARENT_DIRECTORY) {
        constants::PARENT_DIRECTORY.to_string()
    } else {
        match entry.file_name() {
            Some(file_name) => match file_name.to_str() {
                Some(file_name) => file_name.to_string(),
                _ => entry.display().to_string(),
            },
            _ => entry.display().to_string(),
        }
    }
}

fn path_icon(entry: &Path) -> char {
    if entry.is_dir() {
        constants::DIRECTORY_ICON
    } else if entry.is_file() {
        constants::DOCUMENT_ICON
    } else {
        constants::UNKNOWN_ICON
    }
}

pub fn is_up_key(key_event: KeyEvent) -> bool {
    key_event.code == KeyCode::Up
        || (Char('p') == key_event.code && key_event.modifiers == KeyModifiers::CONTROL)
}

pub fn is_down_key(key_event: KeyEvent) -> bool {
    key_event.code == KeyCode::Down
        || (Char('n') == key_event.code && key_event.modifiers == KeyModifiers::CONTROL)
}

pub fn find_match_by_char<T>(
    list: &[T],
    ch: char,
    selected: usize,
    match_char: fn(entry: &T) -> Option<char>,
) -> Option<usize> {
    // First, try to find a matching item that's after the selected item
    if let Some(idx) = find_match_by_char_from(list, ch, selected + 1, match_char) {
        Some(idx)
    } else {
        // If there's no matching item after the selected item, try to find one starting from the top
        find_match_by_char_from(list, ch, 0, match_char)
    }
}

fn find_match_by_char_from<T>(
    list: &[T],
    ch: char,
    from: usize,
    match_char: fn(entry: &T) -> Option<char>,
) -> Option<usize> {
    let ch = ch.to_ascii_lowercase();
    list[from..]
        .iter()
        .enumerate()
        .find(|(_index, entry)| {
            if let Some(first_char) = match_char(entry) {
                first_char.to_ascii_lowercase() == ch
            } else {
                false
            }
        })
        .map(|(index, _)| from + index)
}

pub fn file_size(path: &Path) -> u64 {
    if let Ok(metadata) = path.metadata() {
        metadata.len()
    } else {
        0
    }
}

