// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! `flightctl cloud-profiles` — community profile repository commands.

use crate::{client_manager::ClientManager, output::OutputFormat};
use clap::Subcommand;
use flight_cloud_profiles::{
    ClientConfig, CloudProfileClient, ListFilter, ProfileSortOrder, PublishMeta, VoteDirection,
    sanitize_for_upload,
};
use serde_json::json;

/// Subcommands for `flightctl cloud-profiles`
#[derive(Subcommand)]
pub enum CloudProfilesAction {
    /// List community profiles
    List {
        /// Filter by simulator (msfs, xplane, dcs, …)
        #[arg(long, short = 's')]
        sim: Option<String>,
        /// Filter by aircraft ICAO type code (e.g. C172)
        #[arg(long, short = 'a')]
        aircraft: Option<String>,
        /// Free-text search
        #[arg(long, short = 'q')]
        query: Option<String>,
        /// Sort order: top-rated (default), newest, most-downloaded
        #[arg(long, default_value = "top-rated")]
        sort: String,
        /// Page number (1-based)
        #[arg(long, default_value = "1")]
        page: u32,
    },
    /// Show full details and download a specific profile
    Get {
        /// Profile ID
        id: String,
    },
    /// Publish a local profile to the community repository
    Publish {
        /// Path to the profile JSON file to publish
        profile_path: std::path::PathBuf,
        /// Title for the community profile
        #[arg(long, short = 't')]
        title: String,
        /// Optional description
        #[arg(long, short = 'd')]
        description: Option<String>,
        /// Skip sanitization check (not recommended)
        #[arg(long, hide = true)]
        no_sanitize: bool,
    },
    /// Vote on a community profile
    Vote {
        /// Profile ID
        id: String,
        /// Vote direction: up or down
        direction: String,
    },
    /// Remove your vote from a community profile
    Unvote {
        /// Profile ID
        id: String,
    },
    /// Clear the local profile cache
    ClearCache,
}

pub async fn execute(
    action: &CloudProfilesAction,
    output: OutputFormat,
    _verbose: bool,
    _client: &ClientManager,
) -> anyhow::Result<Option<String>> {
    let config = ClientConfig::default();
    let api = CloudProfileClient::new(config)?;

    match action {
        CloudProfilesAction::List {
            sim,
            aircraft,
            query,
            sort,
            page,
        } => {
            let sort_order = parse_sort_order(sort);
            let filter = ListFilter {
                sim: sim.clone(),
                aircraft_icao: aircraft.clone(),
                query: query.clone(),
                sort: sort_order,
                page: *page,
                per_page: 25,
            };
            let page_result = api.list_page(filter).await?;
            match output {
                OutputFormat::Json => Ok(Some(serde_json::to_string_pretty(&page_result)?)),
                OutputFormat::Human => {
                    if page_result.items.is_empty() {
                        return Ok(Some("No profiles found.".to_string()));
                    }
                    let mut out = format!(
                        "{} profile(s) found (page {}/{}):\n",
                        page_result.total,
                        page_result.page,
                        page_result.total_pages()
                    );
                    for p in &page_result.items {
                        let sim_str = p.sim.as_deref().unwrap_or("any");
                        let acft = p.aircraft_icao.as_deref().unwrap_or("any");
                        out.push_str(&format!(
                            "  [{score:+4}] {id:<20} {title:<32} [{sim}/{acft}] by {author}\n",
                            score = p.score(),
                            id = &p.id[..p.id.len().min(20)],
                            title = truncate(&p.title, 32),
                            sim = sim_str,
                            acft = acft,
                            author = &p.author_handle,
                        ));
                    }
                    Ok(Some(out))
                }
            }
        }

        CloudProfilesAction::Get { id } => {
            let profile = api.get(id).await?;
            match output {
                OutputFormat::Json => Ok(Some(serde_json::to_string_pretty(&profile)?)),
                OutputFormat::Human => {
                    let mut out = format!("ID:           {}\n", profile.id);
                    out.push_str(&format!("Title:        {}\n", profile.title));
                    if let Some(desc) = &profile.description {
                        out.push_str(&format!("Description:  {}\n", desc));
                    }
                    out.push_str(&format!("Author:       {}\n", profile.author_handle));
                    out.push_str(&format!(
                        "Votes:        +{} / -{} (score: {})\n",
                        profile.upvotes,
                        profile.downvotes,
                        profile.score()
                    ));
                    out.push_str(&format!("Downloads:    {}\n", profile.download_count));
                    out.push_str(&format!(
                        "Simulator:    {}\n",
                        profile.profile.sim.as_deref().unwrap_or("any")
                    ));
                    out.push_str(&format!(
                        "Aircraft:     {}\n",
                        profile
                            .profile
                            .aircraft
                            .as_ref()
                            .map(|a| a.icao.as_str())
                            .unwrap_or("any")
                    ));
                    out.push_str(&format!(
                        "Axes:         {} configured\n",
                        profile.profile.axes.len()
                    ));
                    Ok(Some(out))
                }
            }
        }

        CloudProfilesAction::Publish {
            profile_path,
            title,
            description,
            no_sanitize,
        } => {
            let raw = std::fs::read_to_string(profile_path)?;
            let mut profile: flight_cloud_profiles::Profile = serde_json::from_str(&raw)?;
            if !no_sanitize {
                profile = sanitize_for_upload(&profile);
            }
            // Validate before sending
            flight_cloud_profiles::sanitize::validate_for_publish(&profile, title)
                .map_err(|e| anyhow::anyhow!("Profile validation failed: {e}"))?;

            let meta = PublishMeta {
                title: title.clone(),
                description: description.clone(),
            };
            let published = api.publish(&profile, meta).await?;
            match output {
                OutputFormat::Json => Ok(Some(serde_json::to_string_pretty(&published)?)),
                OutputFormat::Human => Ok(Some(format!(
                    "Published profile '{}' with ID: {}\n",
                    published.title, published.id
                ))),
            }
        }

        CloudProfilesAction::Vote { id, direction } => {
            let dir = parse_vote_direction(direction)?;
            let result = api.vote(id, dir).await?;
            match output {
                OutputFormat::Json => Ok(Some(serde_json::to_string_pretty(&result)?)),
                OutputFormat::Human => Ok(Some(format!(
                    "Vote recorded. New score: {} (+{} / -{})\n",
                    result.score(),
                    result.upvotes,
                    result.downvotes
                ))),
            }
        }

        CloudProfilesAction::Unvote { id } => {
            api.remove_vote(id).await?;
            match output {
                OutputFormat::Json => Ok(Some(json!({"removed": true}).to_string())),
                OutputFormat::Human => Ok(Some(format!("Vote removed from profile {id}.\n"))),
            }
        }

        CloudProfilesAction::ClearCache => {
            let cache = flight_cloud_profiles::cache::ProfileCache::default_dir()?;
            cache.clear().await?;
            Ok(Some("Cloud profile cache cleared.\n".to_string()))
        }
    }
}

fn parse_sort_order(s: &str) -> ProfileSortOrder {
    match s.to_ascii_lowercase().as_str() {
        "newest" | "new" => ProfileSortOrder::Newest,
        "most-downloaded" | "downloads" => ProfileSortOrder::MostDownloaded,
        _ => ProfileSortOrder::TopRated,
    }
}

fn parse_vote_direction(s: &str) -> anyhow::Result<VoteDirection> {
    match s.to_ascii_lowercase().as_str() {
        "up" | "+" | "1" => Ok(VoteDirection::Up),
        "down" | "-" | "-1" => Ok(VoteDirection::Down),
        other => Err(anyhow::anyhow!(
            "unknown vote direction '{other}'; use 'up' or 'down'"
        )),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    }
}
