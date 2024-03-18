/*
 * Copyright (c) 2024 Paul Sobolik
 * Created 2024-03-18
 */

use std::fmt::{Debug, Display, Formatter};

pub enum FsError {
    Metadata(std::io::Error),
    Directory(std::io::Error),
}

impl Display for FsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            FsError::Metadata(error) => error.to_string(),
            FsError::Directory(error) => error.to_string(),
        };
        write!(f, "{}", str)
    }
}

impl Debug for FsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}
