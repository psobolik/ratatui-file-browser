use ratatui::widgets::{Block, BorderType, Padding};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::app::styles;

/// Returns the contents of a file as an array of Strings
pub(crate) async fn read_file(path: &Path) -> std::io::Result<Vec<String>> {
    let contents = fs::read_to_string(path).await?;
    Ok(contents.lines().map(|f| f.to_string()).collect())
}

/// Returns the contents of a directory sorted by name, with directories first
pub(crate) async fn read_directory(path: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut paths: Vec<(bool, PathBuf)> = vec![];
    let mut entries = fs::read_dir(&path).await?;
    while let Some(dir_entry) = entries.next_entry().await? {
        let entry = dir_entry.path();
        paths.push((entry.is_dir(), entry));
    }
    // Sort by name, directories first
    paths.sort_unstable_by(|(lhs_is_dir, lhs_path), (rhs_is_dir, rhs_path)| {
        if *lhs_is_dir && !*rhs_is_dir {
            Ordering::Less
        } else if !*lhs_is_dir && *rhs_is_dir {
            Ordering::Greater
        } else {
            lhs_path.file_name().cmp(&rhs_path.file_name())
        }
    });
    Ok(paths.iter().map(|(_, path)| path.clone()).collect())
}

/// Returns a ratatui::widgets::Block styled according to whether or not the component has focus
pub(crate) fn component_block<'a>(has_focus: bool) -> Block<'a> {
    if has_focus {
        Block::bordered()
            .border_type(BorderType::Double)
            .border_style(styles::FOCUSED_BLOCK_STYLE)
            .padding(Padding::horizontal(1))
    } else {
        Block::bordered().padding(Padding::horizontal(1))
    }
}
// Returns an array of bytes formatted as lines of hex
// 02cef870  4e f9 96 01 00 00 00 00  b9 ea e2 00 00 00 00 00  |N...............|
pub fn binary_to_lines(bytes: Vec<u8>) -> Vec<String> {
    const BYTES_PER_LINE: usize = 16;
    const BYTES_PER_HALF_LINE: usize = BYTES_PER_LINE / 2;

    fn format_chunk(index: usize, chunk: &[u8]) -> String {
        fn byte_slice_to_hex(byte_slice: &[u8]) -> String {
            fn byte_to_hex(byte: &u8) -> String {
                format!("{byte:02x}")
            }
            // Format the first group of bytes
            let len = std::cmp::min(byte_slice.len(), BYTES_PER_HALF_LINE);
            let mut chunk1 = byte_slice[0..len]
                .iter()
                .map(byte_to_hex)
                .collect::<Vec<String>>();
            // If there are fewer than BYTES_PER_HALF_LINE, pad the list with blanks
            if len < BYTES_PER_HALF_LINE {
                chunk1.extend(vec![String::from("  "); BYTES_PER_HALF_LINE - len]);
            }
            // Format the second group of bytes, if there are any
            let mut chunk2 = if byte_slice.len() > BYTES_PER_HALF_LINE {
                byte_slice[BYTES_PER_HALF_LINE..]
                    .iter()
                    .map(byte_to_hex)
                    .collect::<Vec<String>>()
            } else {
                vec![]
            };
            // If there are fewer than BYTES_PER_LINE total, pad the list with blanks
            if byte_slice.len() < BYTES_PER_LINE {
                let shortage = if byte_slice.len() <= BYTES_PER_HALF_LINE {
                    BYTES_PER_HALF_LINE
                } else {
                    BYTES_PER_LINE - byte_slice.len()
                };
                chunk2.extend(vec![String::from("  "); shortage]);
            }
            format!("{}  {}", chunk1.join(" "), chunk2.join(" "))
        }
        fn byte_slice_to_ascii(byte_slice: &[u8]) -> String {
            let mut chars = vec![];
            for byte in byte_slice {
                let char = *byte as char;
                if (' '..='~').contains(&char) {
                    chars.push(char)
                } else {
                    chars.push('.')
                }
            }
            // chars.append()
            chars.extend(vec![' '; BYTES_PER_LINE - byte_slice.len()]);
            chars.iter().collect()
        }
        format!(
            "{index:08x}  {}  |{}|",
            byte_slice_to_hex(chunk),
            byte_slice_to_ascii(chunk)
        )
    }
    let mut lines = vec![];
    bytes.chunks(BYTES_PER_LINE).fold(0, |index, chunk| {
        lines.push(format_chunk(index, chunk));
        index + BYTES_PER_LINE
    });
    lines
}
