//! Module behaviour against canned Subsonic answers shaped like Navidrome 0.64's.

use super::*;
use hub_core::{Credentials, HttpPort, HttpRequest, HttpResponse};
use std::collections::BTreeMap;
use std::io::Read;
use std::sync::{Arc, Mutex};

/// Answers by the Subsonic method named in the URL; records every URL.
struct FakeNavidrome {
    answers: Vec<(&'static str, u16, &'static str)>,
    urls: Mutex<Vec<String>>,
}

impl HttpPort for FakeNavidrome {
    fn send(&self, _: &str, request: HttpRequest) -> Result<HttpResponse, HubError> {
        self.urls.lock().unwrap().push(request.url.clone());
        let (_, status, body) = self
            .answers
            .iter()
            .find(|(method, _, _)| request.url.contains(&format!("/rest/{method}?")))
            .unwrap_or_else(|| panic!("no canned answer for {}", request.url));
        let headers = if *status == 206 {
            vec![
                ("content-type".to_owned(), "audio/flac".to_owned()),
                ("content-length".to_owned(), "100".to_owned()),
                ("content-range".to_owned(), "bytes 0-99/24716686".to_owned()),
            ]
        } else {
            vec![("content-type".to_owned(), "application/json".to_owned())]
        };
        Ok(HttpResponse::from_bytes(
            *status,
            headers,
            body.as_bytes().to_vec(),
        ))
    }
}

fn module(
    answers: Vec<(&'static str, u16, &'static str)>,
) -> (Box<dyn Module>, Arc<FakeNavidrome>) {
    let fake = Arc::new(FakeNavidrome {
        answers,
        urls: Mutex::new(vec![]),
    });
    let def = definition("https://music.example/music/rest");
    let built = (def.factory)(fake.clone(), Credentials::new("alice", "pw"));
    (built, fake)
}

const ALBUM_LIST: &str = r#"{"subsonic-response":{"status":"ok","albumList2":{"album":[
 {"id":"a1","name":"Away From The Sea","artist":"Yuksek","artistId":"r1","coverArt":"al-a1_x","songCount":1,"duration":193,"year":2009},
 {"id":"a2","name":"No Year","artist":"Anon","artistId":"r2","songCount":2,"duration":300}]}}}"#;

#[test]
fn albums_are_reshaped_for_the_ui() {
    let (music, _) = module(vec![("getAlbumList2", 200, ALBUM_LIST)]);
    let out = music.call("albums", &json!({"type": "newest"})).unwrap();
    assert_eq!(
        out[0],
        json!({"id":"a1","name":"Away From The Sea","artist":"Yuksek","artistId":"r1","year":2009,"songCount":1,"duration":193,"cover":"al-a1_x"})
    );
    // Missing optional fields default instead of failing the whole list.
    assert_eq!(out[1]["year"], 0);
    assert_eq!(out[1]["cover"], "");
}

#[test]
fn albums_asks_for_the_requested_page_and_clamps_the_size() {
    let (music, fake) = module(vec![("getAlbumList2", 200, ALBUM_LIST)]);
    music
        .call(
            "albums",
            &json!({"type": "random", "size": 5000, "offset": 40}),
        )
        .unwrap();
    let url = fake.urls.lock().unwrap()[0].clone();
    assert!(url.contains("&type=random&size=200&offset=40"), "{url}");
}

#[test]
fn an_unknown_album_list_is_a_bad_argument_not_a_server_call() {
    let (music, fake) = module(vec![]);
    let err = music
        .call("albums", &json!({"type": "../../etc"}))
        .unwrap_err();
    assert_eq!(err.code(), "bad_args");
    assert!(fake.urls.lock().unwrap().is_empty());
}

#[test]
fn overview_returns_three_shelves() {
    let (music, fake) = module(vec![("getAlbumList2", 200, ALBUM_LIST)]);
    let out = music.call("overview", &json!({})).unwrap();
    assert_eq!(out["newest"].as_array().unwrap().len(), 2);
    assert!(out.get("recent").is_some() && out.get("frequent").is_some());
    assert_eq!(fake.urls.lock().unwrap().len(), 3);
}

#[test]
fn album_detail_carries_songs_with_defaults() {
    let body = r#"{"subsonic-response":{"status":"ok","album":{"id":"a1","name":"Tonight","artist":"Yuksek","artistId":"r1","songCount":2,"duration":400,"year":2009,"coverArt":"al-a1",
     "song":[{"id":"s1","title":"Intro","artist":"Yuksek","album":"Tonight","albumId":"a1","track":1,"discNumber":1,"duration":200,"coverArt":"mf-s1","suffix":"flac","bitRate":1021,"starred":"2026-10-07T07:00:00Z"},
             {"id":"s2","title":"Tonight (Radio Edit)","track":2,"duration":193}]}}}"#;
    let (music, _) = module(vec![("getAlbum", 200, body)]);
    let out = music.call("album", &json!({"id": "a1"})).unwrap();
    assert_eq!(out["album"]["name"], "Tonight");
    assert_eq!(out["songs"][0]["starred"], true);
    assert_eq!(out["songs"][0]["format"], "flac");
    assert_eq!(out["songs"][1]["starred"], false);
    assert_eq!(
        out["songs"][1]["disc"], 1,
        "missing disc number reads as disc 1"
    );
}

#[test]
fn artists_flatten_the_alphabetical_index() {
    let body = r#"{"subsonic-response":{"status":"ok","artists":{"index":[
      {"name":"A","artist":[{"id":"r1","name":"Alpha","albumCount":2}]},
      {"name":"Y","artist":[{"id":"r2","name":"Yuksek","albumCount":1,"coverArt":"ar-r2"}]}]}}}"#;
    let (music, _) = module(vec![("getArtists", 200, body)]);
    let out = music.call("artists", &json!({})).unwrap();
    assert_eq!(
        out,
        json!([
        {"id":"r1","name":"Alpha","albumCount":2,"cover":""},
        {"id":"r2","name":"Yuksek","albumCount":1,"cover":"ar-r2"}])
    );
}

#[test]
fn an_empty_playlist_answer_is_an_empty_list() {
    // Navidrome sends `"playlists":{}` (no array) when the user has none.
    let (music, _) = module(vec![(
        "getPlaylists",
        200,
        r#"{"subsonic-response":{"status":"ok","playlists":{}}}"#,
    )]);
    assert_eq!(music.call("playlists", &json!({})).unwrap(), json!([]));
}

#[test]
fn playlist_detail_maps_entries_to_songs() {
    let body = r#"{"subsonic-response":{"status":"ok","playlist":{"id":"p1","name":"Most played","songCount":1,"duration":193,"entry":[{"id":"s2","title":"Tonight (Radio Edit)","duration":193}]}}}"#;
    let (music, _) = module(vec![("getPlaylist", 200, body)]);
    let out = music.call("playlist", &json!({"id": "p1"})).unwrap();
    assert_eq!(out["playlist"]["name"], "Most played");
    assert_eq!(out["songs"][0]["id"], "s2");
}

#[test]
fn search_groups_artists_albums_and_songs() {
    let body = r#"{"subsonic-response":{"status":"ok","searchResult3":{"artist":[{"id":"r2","name":"Yuksek"}],"song":[{"id":"s2","title":"Tonight"}]}}}"#;
    let (music, fake) = module(vec![("search3", 200, body)]);
    let out = music.call("search", &json!({"query": "yuk sek"})).unwrap();
    assert_eq!(out["artists"][0]["name"], "Yuksek");
    assert_eq!(out["albums"], json!([]));
    assert!(fake.urls.lock().unwrap()[0].contains("query=yuk%20sek"));
}

#[test]
fn star_and_unstar_pick_the_right_method() {
    let ok = r#"{"subsonic-response":{"status":"ok"}}"#;
    let (music, fake) = module(vec![("star", 200, ok), ("unstar", 200, ok)]);
    music
        .call("star", &json!({"id": "s1", "on": true}))
        .unwrap();
    music
        .call("star", &json!({"id": "s1", "on": false}))
        .unwrap();
    let urls = fake.urls.lock().unwrap();
    assert!(urls[0].contains("/rest/star?") && urls[1].contains("/rest/unstar?"));
}

#[test]
fn expired_credentials_surface_as_denied() {
    let body = r#"{"subsonic-response":{"status":"failed","error":{"code":40,"message":"Wrong"}}}"#;
    let (music, _) = module(vec![("getArtists", 200, body)]);
    assert_eq!(
        music.call("artists", &json!({})).unwrap_err().code(),
        "denied"
    );
}

#[test]
fn unknown_operation_is_named() {
    let (music, _) = module(vec![]);
    assert_eq!(
        music.call("explode", &json!({})).unwrap_err().code(),
        "unknown_op"
    );
}

#[test]
fn stream_forwards_range_and_returns_partial_content() {
    let (music, fake) = module(vec![("stream", 206, "x".repeat(100).leak())]);
    let request = MediaRequest {
        op: "stream".into(),
        params: BTreeMap::from([("id".to_owned(), "s1".to_owned())]),
        range: Some("bytes=0-99".into()),
    };
    let mut response = music.media(&request).unwrap();
    assert_eq!(response.status, 206);
    assert_eq!(response.content_type, "audio/flac");
    assert_eq!(
        response.content_range.as_deref(),
        Some("bytes 0-99/24716686")
    );
    assert_eq!(response.content_length, Some(100));
    let mut bytes = Vec::new();
    response.body.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes.len(), 100);
    assert!(fake.urls.lock().unwrap()[0].contains("/rest/stream?"));
}

#[test]
fn cover_defaults_to_a_small_size_and_needs_an_id() {
    let (music, fake) = module(vec![("getCoverArt", 200, "jpegbytes")]);
    let request = MediaRequest {
        op: "cover".into(),
        params: BTreeMap::from([("id".to_owned(), "al-a1".to_owned())]),
        range: None,
    };
    music.media(&request).unwrap();
    assert!(fake.urls.lock().unwrap()[0].contains("id=al-a1&size=300"));
    let no_id = MediaRequest {
        op: "cover".into(),
        ..MediaRequest::default()
    };
    assert_eq!(music.media(&no_id).err().unwrap().code(), "bad_args");
}

#[test]
fn the_manifest_id_is_the_kvasir_app_id() {
    assert_eq!(definition("https://x/rest").manifest.id, "music");
}
