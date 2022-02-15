// Copyright (c) 2022 Jan Holthuis
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy
// of the MPL was not distributed with this file, You can obtain one at
// http://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Methods used for locating and loading the configuration.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Represents the configuration file.
#[derive(Debug, Deserialize)]
pub struct Config {
    /// Directory to download files to.
    pub download_dir: PathBuf,
    /// Podcasts that should be downloaded.
    pub podcast: Vec<PodcastConfig>,
}

impl Config {
    /// Load a config object from a custom location.
    pub fn from_path(path: &dyn AsRef<Path>) -> std::io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    /// Load a config object from the default location.
    pub fn from_default_path() -> std::io::Result<Self> {
        let config_path = find_config_path()?;
        Self::from_path(&config_path)
    }
}

/// Represents the configuration for a single podcast.
#[derive(Debug, Deserialize)]
pub struct PodcastConfig {
    /// Podcast RSS Feed URL
    pub feed_url: String,
}

fn find_config_path() -> std::io::Result<PathBuf> {
    dirs::config_dir()
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Unable to find application config base directory!",
            )
        })
        .map(|mut path| {
            path.push("podcatcher-rs");
            path.push("config.toml");
            path
        })
}
