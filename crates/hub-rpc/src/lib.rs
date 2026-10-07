//! The wire protocol between the web UI and a shell, shared by every shell.
//!
//! Three kinds of traffic: JSON commands (`dispatch`), media URLs (`parse_media_target`) and
//! binary uploads (`upload`). Hides: the envelope format, command names and URL layout, so
//! the desktop/mobile shell and the dev web host cannot drift apart.

use hub_core::{Credentials, Hub, HubError, MediaRequest};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// One answer to the UI: `{"ok":true,"data":..}` or `{"ok":false,"error":{..}}`.
#[derive(Debug, Serialize, PartialEq)]
#[serde(untagged)]
pub enum Envelope {
    Ok {
        ok: bool,
        data: Value,
    },
    Err {
        ok: bool,
        error: hub_core::ErrorView,
    },
}

impl Envelope {
    fn ok(data: Value) -> Self {
        Envelope::Ok { ok: true, data }
    }

    fn from_result(result: Result<Value, HubError>) -> Self {
        match result {
            Ok(data) => Self::ok(data),
            Err(e) => Envelope::Err {
                ok: false,
                error: e.view(),
            },
        }
    }

    /// The envelope as JSON text.
    pub fn to_json(&self) -> String {
        // Both variants hold only JSON-representable data.
        serde_json::to_string(self).expect("envelope serialises")
    }
}

fn me_json(me: &hub_wire::Me) -> Value {
    json!({"username": me.username, "role": me.role, "apps": me.apps})
}

fn bad(cmd: &str, reason: &str) -> HubError {
    HubError::BadArgs {
        op: cmd.to_owned(),
        reason: reason.to_owned(),
    }
}

/// Runs one UI command.
///
/// Commands: `restore`, `sign_in {username,password}`, `sign_out`, `modules`,
/// `call {module,op,args}`.
pub fn dispatch(hub: &Hub, cmd: &str, args: &Value) -> Envelope {
    Envelope::from_result(run(hub, cmd, args))
}

fn run(hub: &Hub, cmd: &str, args: &Value) -> Result<Value, HubError> {
    let text = |key: &str| {
        args.get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| bad(cmd, &format!("'{key}' must be a string")))
    };
    match cmd {
        "restore" => Ok(hub.restore()?.map_or(Value::Null, |me| me_json(&me))),
        "sign_in" => {
            let me = hub.sign_in(Credentials::new(text("username")?, text("password")?))?;
            Ok(me_json(&me))
        }
        "sign_out" => hub.sign_out().map(|()| Value::Null),
        "modules" => {
            let manifests = hub.manifests()?;
            Ok(json!(
                manifests
                    .iter()
                    .map(|m| json!({"id": m.id, "title": m.title, "summary": m.summary}))
                    .collect::<Vec<_>>()
            ))
        }
        "call" => {
            let empty = json!({});
            hub.call(
                text("module")?,
                text("op")?,
                args.get("args").unwrap_or(&empty),
            )
        }
        _ => Err(bad(cmd, "unknown command")),
    }
}

/// Splits `module/op?key=value&..` (as in `/media/music/stream?id=1`) into a media request.
pub fn parse_media_target(
    path_and_query: &str,
    range: Option<&str>,
) -> Result<(String, MediaRequest), HubError> {
    let (path, query) = path_and_query
        .split_once('?')
        .unwrap_or((path_and_query, ""));
    let mut parts = path.trim_matches('/').split('/');
    let (Some(module), Some(op), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err(bad("media", "expected /<module>/<op>"));
    };
    Ok((
        module.to_owned(),
        MediaRequest {
            op: op.to_owned(),
            params: parse_query(query),
            range: range.map(str::to_owned),
        },
    ))
}

/// Runs a binary upload: `module/op?arg=value&..` plus the raw body.
pub fn upload(hub: &Hub, path_and_query: &str, body: Vec<u8>) -> Envelope {
    let result = parse_media_target(path_and_query, None).and_then(|(module, request)| {
        let args: serde_json::Map<String, Value> = request
            .params
            .into_iter()
            .map(|(k, v)| (k, Value::String(v)))
            .collect();
        hub.upload(&module, &request.op, &Value::Object(args), body)
    });
    Envelope::from_result(result)
}

/// `a=1&b=x%20y` -> map. Later duplicates win.
pub fn parse_query(query: &str) -> BTreeMap<String, String> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (percent_decode(key), percent_decode(value))
        })
        .collect()
}

/// Decodes `%XX` and `+`; invalid escapes are kept literally.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len()
                && hex(bytes[i + 1]).is_some()
                && hex(bytes[i + 2]).is_some() =>
            {
                out.push(hex(bytes[i + 1]).unwrap_or(0) * 16 + hex(bytes[i + 2]).unwrap_or(0));
                i += 2;
            }
            byte => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(byte: u8) -> Option<u8> {
    char::from(byte)
        .to_digit(16)
        .and_then(|d| u8::try_from(d).ok())
}

#[cfg(test)]
mod tests;
