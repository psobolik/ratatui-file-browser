/*
 * Copyright (c) 2023 Paul Sobolik
 * Created 2023-12-19
 */

use clap::Parser;

#[derive(Parser)]
#[command(version, long_about("A simple TUI File Browser"))]
pub struct Options {
    pub(super) init_path: Option<std::path::PathBuf>,
}
