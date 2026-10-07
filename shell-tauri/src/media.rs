//! `hubmedia://` responses and file saving. Tauri buffers a response body in memory, so large
//! media is served in bounded slices (`hub_rpc::bounded_range`) and the player asks for more.

use hub_core::{Hub, HubError, MediaRequest, MediaResponse};
use hub_rpc::{
    MEDIA_CHUNK_BYTES, bounded_range, numbered_name, parse_media_target, prefix_content_range,
    safe_file_name,
};
use std::io::Read;
use std::path::Path;
use tauri::http::{Request, Response, StatusCode, header};

fn plain(status: StatusCode, text: &str) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(text.as_bytes().to_vec())
        .expect("static response is valid")
}

fn status_of(error: &HubError) -> StatusCode {
    match error {
        HubError::NotSignedIn => StatusCode::UNAUTHORIZED,
        HubError::Forbidden { .. } => StatusCode::FORBIDDEN,
        HubError::NotFound { .. } | HubError::UnknownModule(_) => StatusCode::NOT_FOUND,
        HubError::BadArgs { .. } => StatusCode::BAD_REQUEST,
        _ => StatusCode::BAD_GATEWAY,
    }
}

/// Answers one `hubmedia://localhost/<module>/<op>?...` request with ONE upstream request.
///
/// With a `Range` header the range is bounded before it is sent. Without one, at most one
/// slice is read from the stream; when the file is bigger the answer becomes `206` with a
/// prefix `Content-Range`, which makes the player continue with ranged requests.
pub fn serve(hub: &Hub, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let target = format!("{}?{}", request.uri().path(), request.uri().query().unwrap_or_default());
    let range = request.headers().get(header::RANGE).and_then(|v| v.to_str().ok());
    let bounded = range.map(|r| bounded_range(Some(r), MEDIA_CHUNK_BYTES));
    let result = parse_media_target(&target, bounded.as_deref())
        .and_then(|(module, media_request)| hub.media(&module, &media_request))
        .and_then(read_slice);
    match result {
        Ok((media, bytes)) => respond(media, bytes),
        Err(error) => plain(status_of(&error), &error.view().message),
    }
}

/// Reads up to one slice of the body; a longer unranged `200` is turned into a `206` prefix.
fn read_slice(mut media: MediaResponse) -> Result<(MediaResponse, Vec<u8>), HubError> {
    let mut bytes = Vec::new();
    (&mut media.body)
        .take(MEDIA_CHUNK_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|e| HubError::unavailable("media", e))?;
    let got = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if media.status == 200 && media.content_length.is_some_and(|total| total > got) {
        media.status = 206;
        media.content_range = media.content_length.map(|total| prefix_content_range(got, total));
    }
    Ok((media, bytes))
}

fn respond(media: MediaResponse, bytes: Vec<u8>) -> Response<Vec<u8>> {
    let mut builder = Response::builder()
        .status(media.status)
        .header(header::CONTENT_TYPE, media.content_type)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, bytes.len());
    if let Some(range) = media.content_range {
        builder = builder.header(header::CONTENT_RANGE, range);
    }
    builder
        .body(bytes)
        .unwrap_or_else(|e| plain(StatusCode::BAD_GATEWAY, &e.to_string()))
}

/// Streams a Files download into `folder` under a collision-free name; returns the final path.
pub fn save_download(hub: &Hub, folder: &Path, repo: &str, path: &str) -> Result<String, String> {
    let request = MediaRequest {
        op: "download".to_owned(),
        params: [("repo", repo), ("path", path)].map(|(k, v)| (k.to_owned(), v.to_owned())).into(),
        range: None,
    };
    let mut media = hub.media("files", &request).map_err(|e| e.view().message)?;
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let name = safe_file_name(media.file_name.as_deref().unwrap_or("download"));
    let (mut file, target) = (1..1000)
        .find_map(|attempt| {
            let target = folder.join(numbered_name(&name, attempt));
            std::fs::OpenOptions::new().write(true).create_new(true).open(&target).ok().map(|f| (f, target))
        })
        .ok_or("could not find a free file name")?;
    std::io::copy(&mut media.body, &mut file).map_err(|e| e.to_string())?;
    Ok(target.display().to_string())
}
