//! UI-facing shapes (camelCase JSON) and their conversion from Subsonic answers.
//!
//! Hides: which Subsonic fields exist and which of them the UI needs.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawAlbum {
    id: String,
    name: String,
    #[serde(default)]
    artist: String,
    #[serde(default)]
    artist_id: String,
    #[serde(default)]
    year: u32,
    #[serde(default)]
    song_count: u32,
    #[serde(default)]
    duration: u32,
    #[serde(default)]
    cover_art: String,
    #[serde(default)]
    song: Vec<RawSong>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawSong {
    id: String,
    title: String,
    #[serde(default)]
    artist: String,
    #[serde(default)]
    album: String,
    #[serde(default)]
    album_id: String,
    #[serde(default)]
    track: u32,
    #[serde(default)]
    disc_number: u32,
    #[serde(default)]
    duration: u32,
    #[serde(default)]
    cover_art: String,
    #[serde(default)]
    suffix: String,
    #[serde(default)]
    bit_rate: u32,
    #[serde(default)]
    starred: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawArtist {
    id: String,
    name: String,
    #[serde(default)]
    album_count: u32,
    #[serde(default)]
    cover_art: String,
    #[serde(default)]
    album: Vec<RawAlbum>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawPlaylist {
    id: String,
    name: String,
    #[serde(default)]
    song_count: u32,
    #[serde(default)]
    duration: u32,
    #[serde(default)]
    cover_art: String,
    #[serde(default)]
    entry: Vec<RawSong>,
}

// Answer envelopes: one struct per Subsonic method family.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AlbumListAnswer {
    #[serde(default)]
    album_list2: AlbumList,
}
#[derive(Debug, Default, Deserialize)]
pub(crate) struct AlbumList {
    #[serde(default)]
    album: Vec<RawAlbum>,
}
#[derive(Debug, Deserialize)]
pub(crate) struct AlbumAnswer {
    album: RawAlbum,
}
#[derive(Debug, Deserialize)]
pub(crate) struct ArtistAnswer {
    artist: RawArtist,
}
#[derive(Debug, Deserialize)]
pub(crate) struct ArtistsAnswer {
    #[serde(default)]
    artists: ArtistIndexes,
}
#[derive(Debug, Default, Deserialize)]
pub(crate) struct ArtistIndexes {
    #[serde(default)]
    index: Vec<ArtistIndex>,
}
#[derive(Debug, Deserialize)]
pub(crate) struct ArtistIndex {
    #[serde(default)]
    artist: Vec<RawArtist>,
}
#[derive(Debug, Deserialize)]
pub(crate) struct PlaylistsAnswer {
    #[serde(default)]
    playlists: PlaylistList,
}
#[derive(Debug, Default, Deserialize)]
pub(crate) struct PlaylistList {
    #[serde(default)]
    playlist: Vec<RawPlaylist>,
}
#[derive(Debug, Deserialize)]
pub(crate) struct PlaylistAnswer {
    playlist: RawPlaylist,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchAnswer {
    #[serde(default)]
    search_result3: RawSearch,
}
#[derive(Debug, Default, Deserialize)]
pub(crate) struct RawSearch {
    #[serde(default)]
    artist: Vec<RawArtist>,
    #[serde(default)]
    album: Vec<RawAlbum>,
    #[serde(default)]
    song: Vec<RawSong>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Album {
    id: String,
    name: String,
    artist: String,
    artist_id: String,
    year: u32,
    song_count: u32,
    duration: u32,
    cover: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Song {
    id: String,
    title: String,
    artist: String,
    album: String,
    album_id: String,
    track: u32,
    disc: u32,
    duration: u32,
    cover: String,
    format: String,
    bit_rate: u32,
    starred: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Artist {
    id: String,
    name: String,
    album_count: u32,
    cover: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Playlist {
    id: String,
    name: String,
    song_count: u32,
    duration: u32,
    cover: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct AlbumDetail {
    album: Album,
    songs: Vec<Song>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct ArtistDetail {
    artist: Artist,
    albums: Vec<Album>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct PlaylistDetail {
    playlist: Playlist,
    songs: Vec<Song>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct SearchResult {
    artists: Vec<Artist>,
    albums: Vec<Album>,
    songs: Vec<Song>,
}

impl From<RawAlbum> for Album {
    fn from(raw: RawAlbum) -> Self {
        Self {
            id: raw.id,
            name: raw.name,
            artist: raw.artist,
            artist_id: raw.artist_id,
            year: raw.year,
            song_count: raw.song_count,
            duration: raw.duration,
            cover: raw.cover_art,
        }
    }
}

impl From<RawSong> for Song {
    fn from(raw: RawSong) -> Self {
        Self {
            id: raw.id,
            title: raw.title,
            artist: raw.artist,
            album: raw.album,
            album_id: raw.album_id,
            track: raw.track,
            disc: raw.disc_number.max(1),
            duration: raw.duration,
            cover: raw.cover_art,
            format: raw.suffix,
            bit_rate: raw.bit_rate,
            starred: raw.starred.is_some(),
        }
    }
}

impl From<RawArtist> for Artist {
    fn from(raw: RawArtist) -> Self {
        Self {
            id: raw.id,
            name: raw.name,
            album_count: raw.album_count,
            cover: raw.cover_art,
        }
    }
}

impl From<RawPlaylist> for Playlist {
    fn from(raw: RawPlaylist) -> Self {
        Self {
            id: raw.id,
            name: raw.name,
            song_count: raw.song_count,
            duration: raw.duration,
            cover: raw.cover_art,
        }
    }
}

pub(crate) fn albums(answer: AlbumListAnswer) -> Vec<Album> {
    answer
        .album_list2
        .album
        .into_iter()
        .map(Album::from)
        .collect()
}

pub(crate) fn album_detail(answer: AlbumAnswer) -> AlbumDetail {
    let mut raw = answer.album;
    let songs = std::mem::take(&mut raw.song)
        .into_iter()
        .map(Song::from)
        .collect();
    AlbumDetail {
        album: raw.into(),
        songs,
    }
}

pub(crate) fn artist_detail(answer: ArtistAnswer) -> ArtistDetail {
    let mut raw = answer.artist;
    let albums = std::mem::take(&mut raw.album)
        .into_iter()
        .map(Album::from)
        .collect();
    ArtistDetail {
        artist: raw.into(),
        albums,
    }
}

pub(crate) fn artists(answer: ArtistsAnswer) -> Vec<Artist> {
    answer
        .artists
        .index
        .into_iter()
        .flat_map(|i| i.artist)
        .map(Artist::from)
        .collect()
}

pub(crate) fn playlists(answer: PlaylistsAnswer) -> Vec<Playlist> {
    answer
        .playlists
        .playlist
        .into_iter()
        .map(Playlist::from)
        .collect()
}

pub(crate) fn playlist_detail(answer: PlaylistAnswer) -> PlaylistDetail {
    let mut raw = answer.playlist;
    let songs = std::mem::take(&mut raw.entry)
        .into_iter()
        .map(Song::from)
        .collect();
    PlaylistDetail {
        playlist: raw.into(),
        songs,
    }
}

pub(crate) fn search(answer: SearchAnswer) -> SearchResult {
    let raw = answer.search_result3;
    SearchResult {
        artists: raw.artist.into_iter().map(Artist::from).collect(),
        albums: raw.album.into_iter().map(Album::from).collect(),
        songs: raw.song.into_iter().map(Song::from).collect(),
    }
}
