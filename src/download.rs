// Copyright (c) 2022 Jan Holthuis
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy
// of the MPL was not distributed with this file, You can obtain one at
// http://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

use crate::config::PodcastConfig;
use futures::lock::Mutex;
use futures::stream::StreamExt;
use linya::Progress;
use reqwest::{Client, Url};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

/// Represents a single episode that should be downloaded.
#[derive(Debug)]
pub struct EpisodeDownload {
    pub guid: String,
    pub url: Url,
    pub file_size: Option<usize>,
    pub file_path: PathBuf,
}

impl EpisodeDownload {
    /// Returns the file name from `file_path`.
    pub fn file_name(&self) -> &str {
        // Unwrap is safe here, because we ensures that there always is a file_path.
        self.file_path.file_name().unwrap().to_str().unwrap()
    }

    /// Returns the file size as human-readable string.
    pub fn human_file_size(&self) -> String {
        self.file_size.map_or_else(
            || String::from("unknown size"),
            |size| {
                let (human_size, human_size_suffix) = to_human_size(size);
                format!("{}{}", human_size, human_size_suffix)
            },
        )
    }
}

pub fn to_human_size(size: usize) -> (usize, char) {
    match size {
        _ if size >= 1_000_000_000 => (size / 1_000_000_000, 'G'),
        _ if size >= 1_000_000 => (size / 1_000_000, 'M'),
        _ if size >= 1000 => (size / 1000, 'K'),
        _ => (size, 'B'),
    }
}

/// Returns the content length of the given `url` (or `None` on failure).
///
/// *Note:* This performs a `HEAD` request.
pub async fn retrieve_content_length(client: &Client, url: &Url) -> Option<usize> {
    // We need to determine the file size before we download so we can create a ProgressBar
    // A Header request for the CONTENT_LENGTH header gets us the file size
    client
        .head(url.as_str())
        .send()
        .await
        .ok()
        .and_then(|resp| {
            if resp.status().is_success() {
                resp.headers() // Gives is the HeaderMap
                    .get(reqwest::header::CONTENT_LENGTH) // Gives us an Option containing the HeaderValue
                    .and_then(|ct_len| ct_len.to_str().ok()) // Unwraps the Option as &str
                    .and_then(|ct_len| ct_len.parse().ok()) // Parses the Option as u64
                    .and_then(|ct_len| if ct_len > 0 { Some(ct_len) } else { None })
            } else {
                None
            }
        })
}

/// Download a file and display a progress bar for it.
///
/// If no `file_size` is specified, this tries to determine the file size from the `Content-Length`
/// header automatically.
pub async fn download_file(
    data: &mut impl Write,
    multibar: Arc<Mutex<Progress>>,
    url: &Url,
    file_size: Option<usize>,
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create a reqwest Client
    let client = Client::new();

    let file_size = match file_size {
        Some(_) => file_size,
        None => retrieve_content_length(&client, url).await,
    };

    // Here we build the actual Request with a RequestBuilder from the Client
    let request = client.get(url.as_str());

    // Create the ProgressBar with the acquired size from before
    // and add it to the multibar
    let bar_size = file_size.unwrap_or(1);
    let progress_bar = multibar.lock().await.bar(bar_size, label);

    // Do the actual request to download the file
    let mut download = request.send().await?;

    // Do an asynchronous, buffered copy of the download to the output file.
    //
    // We use the part from the reqwest-tokio example here on purpose
    // This way, we are able to increase the ProgressBar with every downloaded chunk
    while let Some(chunk) = download.chunk().await? {
        multibar
            .lock()
            .await
            .inc_and_draw(&progress_bar, chunk.len());
        data.write_all(&chunk)?; // Write chunk to output file
    }

    Ok(())
}

pub async fn fetch_sync_info(
    directory: PathBuf,
    podcasts: Vec<PodcastConfig>,
    max_jobs: usize,
) -> Vec<EpisodeDownload> {
    println!("Fetching podcast feeds...");
    let progress = std::sync::Arc::new(Mutex::new(linya::Progress::new()));
    let progress1 = progress.clone();
    let task_count = podcasts.len();
    let results: Vec<Result<(PodcastConfig, rss::Channel), Box<dyn std::error::Error>>> =
        futures::stream::iter(podcasts.into_iter())
            .enumerate()
            .map(move |(i, podcast)| {
                let prog1 = progress1.clone();
                async move {
                    let mut data: Vec<u8> = Vec::new();
                    let url = reqwest::Url::parse(&podcast.feed_url)?;
                    download_file(
                        &mut data,
                        prog1.clone(),
                        &url,
                        None,
                        format!("({}/{}) {}", i + 1, &task_count, &podcast.feed_url).as_ref(),
                    )
                    .await?;
                    let channel = rss::Channel::read_from(&data[..])?;
                    Ok((podcast, channel))
                }
            })
            .buffered(max_jobs)
            .collect()
            .await;

    results
        .into_iter()
        .flat_map(|result| {
            let (podcast, channel) = result.unwrap();
            let title = podcast.title.unwrap_or(channel.title);
            let mut path = directory.clone();
            path.push(title);

            channel
                .items
                .into_iter()
                .filter_map(move |item| {
                    let (url_string, file_size) = match item.enclosure {
                        Some(enc) => (
                            enc.url,
                            enc.length.parse().ok().and_then(|length| {
                                if length > 0 {
                                    Some(length)
                                } else {
                                    None
                                }
                            }),
                        ),
                        None => return None,
                    };

                    let url = match reqwest::Url::parse(&url_string) {
                        Ok(x) => x,
                        Err(_) => return None,
                    };

                    let guid = item.guid.map(|x| x.value).unwrap_or_else(|| url_string);

                    let file_name = PathBuf::from(url.path())
                        .file_name()
                        .and_then(|x| x.to_str())
                        .unwrap_or("episode.mp3")
                        .to_owned();
                    let mut file_path = path.clone();
                    file_path.push(file_name);
                    Some(EpisodeDownload {
                        guid,
                        url,
                        file_size,
                        file_path,
                    })
                })
                .take(1)
                .filter(|dl| !dl.file_path.exists())
        })
        .collect()
}
