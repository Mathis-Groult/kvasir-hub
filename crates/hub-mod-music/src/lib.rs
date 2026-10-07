//! Music module: Navidrome through the Subsonic API.
//!
//! Operations (JSON in, JSON out): `overview`, `albums {type, offset, size}`, `album {id}`,
//! `artists`, `artist {id}`, `playlists`, `playlist {id}`, `search {query}`,
//! `star {id, on}`, `scrobble {id}`. Media: `cover {id, size}`, `stream {id}`.
//!
//! Hides: the Subsonic protocol (see `subsonic.rs`) and the shape of its answers (`model.rs`).

mod model;
mod subsonic;

use hub_core::{Args, Factory, HubError, Manifest, MediaRequest, MediaResponse, Module, ModuleDef};
use serde::Serialize;
use serde_json::{Value, json};
use subsonic::Subsonic;

/// Album lists the UI may ask for (`getAlbumList2` types).
const ALBUM_LISTS: [&str; 6] = [
    "newest",
    "recent",
    "frequent",
    "random",
    "alphabeticalByName",
    "starred",
];
/// Albums per overview shelf.
const SHELF_SIZE: u32 = 12;
const DEFAULT_PAGE_SIZE: u64 = 40;
const MAX_PAGE_SIZE: u64 = 200;

/// Catalog entry. `rest_url` is e.g. `https://music.example.org/music/rest`.
pub fn definition(rest_url: &str) -> ModuleDef {
    let rest_url = rest_url.to_owned();
    let factory: Factory = Box::new(move |http, credentials| {
        Box::new(Music {
            client: Subsonic::new(http, &rest_url, &credentials),
        })
    });
    ModuleDef::new(
        Manifest {
            id: "music",
            title: "Music",
            summary: "Your library, playlists and a player.",
        },
        factory,
    )
}

struct Music {
    client: Subsonic,
}

fn to_json(value: impl Serialize) -> Result<Value, HubError> {
    serde_json::to_value(value).map_err(|e| HubError::malformed("Music", e))
}

impl Music {
    fn album_list(
        &self,
        kind: &str,
        size: u64,
        offset: u64,
    ) -> Result<Vec<model::Album>, HubError> {
        let answer: model::AlbumListAnswer = self.client.call(
            "getAlbumList2",
            &[
                ("type", kind),
                ("size", &size.to_string()),
                ("offset", &offset.to_string()),
            ],
        )?;
        Ok(model::albums(answer))
    }

    fn overview(&self) -> Result<Value, HubError> {
        let shelf = |kind: &str| self.album_list(kind, u64::from(SHELF_SIZE), 0);
        Ok(json!({
            "newest": shelf("newest")?,
            "recent": shelf("recent")?,
            "frequent": shelf("frequent")?,
        }))
    }

    fn albums(&self, args: &Args<'_>) -> Result<Value, HubError> {
        let kind = args.opt_str("type").unwrap_or("newest");
        if !ALBUM_LISTS.contains(&kind) {
            return Err(HubError::BadArgs {
                op: "albums".into(),
                reason: format!("unknown list '{kind}'"),
            });
        }
        let size = args
            .opt_u64("size")
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE);
        let offset = args.opt_u64("offset").unwrap_or(0);
        to_json(self.album_list(kind, size, offset)?)
    }

    fn set_star(&self, id: &str, on: bool) -> Result<Value, HubError> {
        let _: Value = self
            .client
            .call(if on { "star" } else { "unstar" }, &[("id", id)])?;
        Ok(json!({"ok": true}))
    }
}

impl Module for Music {
    fn call(&self, op: &str, args: &Value) -> Result<Value, HubError> {
        let args = Args::new(op, args);
        match op {
            "overview" => self.overview(),
            "albums" => self.albums(&args),
            "album" => to_json(model::album_detail(
                self.client.call("getAlbum", &[("id", args.str("id")?)])?,
            )),
            "artists" => to_json(model::artists(self.client.call("getArtists", &[])?)),
            "artist" => to_json(model::artist_detail(
                self.client.call("getArtist", &[("id", args.str("id")?)])?,
            )),
            "playlists" => to_json(model::playlists(self.client.call("getPlaylists", &[])?)),
            "playlist" => to_json(model::playlist_detail(
                self.client
                    .call("getPlaylist", &[("id", args.str("id")?)])?,
            )),
            "search" => {
                let query = args.str("query")?;
                to_json(model::search(self.client.call(
                    "search3",
                    &[
                        ("query", query),
                        ("artistCount", "6"),
                        ("albumCount", "12"),
                        ("songCount", "30"),
                    ],
                )?))
            }
            "star" => self.set_star(args.str("id")?, args.opt_bool("on").unwrap_or(true)),
            "scrobble" => {
                let _: Value = self.client.call(
                    "scrobble",
                    &[("id", args.str("id")?), ("submission", "true")],
                )?;
                Ok(json!({"ok": true}))
            }
            _ => Err(HubError::UnknownOp {
                module: "music".into(),
                op: op.to_owned(),
            }),
        }
    }

    fn media(&self, request: &MediaRequest) -> Result<MediaResponse, HubError> {
        let id = request.params.get("id").ok_or_else(|| HubError::BadArgs {
            op: request.op.clone(),
            reason: "'id' is required".into(),
        })?;
        let response = match request.op.as_str() {
            "cover" => {
                let size = request.params.get("size").map_or("300", String::as_str);
                self.client
                    .raw("getCoverArt", &[("id", id), ("size", size)], None)?
            }
            "stream" => self
                .client
                .raw("stream", &[("id", id)], request.range.as_deref())?,
            _ => {
                return Err(HubError::UnknownOp {
                    module: "music".into(),
                    op: request.op.clone(),
                });
            }
        };
        Ok(MediaResponse {
            status: response.status,
            content_type: response
                .header("content-type")
                .unwrap_or("application/octet-stream")
                .to_owned(),
            content_length: response
                .header("content-length")
                .and_then(|v| v.parse().ok()),
            content_range: response.header("content-range").map(str::to_owned),
            file_name: None,
            body: response.body,
        })
    }
}

#[cfg(test)]
mod tests;
