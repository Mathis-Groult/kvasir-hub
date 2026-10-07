//! Files module: Seafile libraries, folders, downloads, thumbnails and uploads.
//!
//! Operations: `overview` (account + libraries), `ls {repo, path}`, `mkdir {repo, path}`,
//! `rename {repo, path, name, isDir}`, `delete {repo, path, isDir}`, upload `put {repo, dir, name}`.
//! Media: `download {repo, path}` (Range forwarded), `thumb {repo, path, size}`.
//!
//! Hides: the Seafile API (token handling in `seafile.rs`, answer shapes in `model.rs`).

mod model;
mod multipart;
mod seafile;

use hub_core::{
    Args, Factory, HttpRequest, HubError, Manifest, MediaRequest, MediaResponse, Method, Module,
    ModuleDef,
};
use seafile::{SERVICE, Seafile, q};
use serde::Serialize;
use serde_json::{Value, json};

const DEFAULT_THUMB: &str = "96";
/// Seafile answers a successful rename with a 301 and the body `"success"`.
const RENAME_OK: &[u16] = &[301];

/// Catalog entry. `base_url` is e.g. `https://files.example.org`.
pub fn definition(base_url: &str) -> ModuleDef {
    let base_url = base_url.to_owned();
    let factory: Factory = Box::new(move |http, credentials| {
        Box::new(Files {
            client: Seafile::new(http, &base_url, credentials),
        })
    });
    ModuleDef::new(
        Manifest {
            id: "files",
            title: "Files",
            summary: "Your Seafile libraries: browse, preview, upload.",
        },
        factory,
    )
}

struct Files {
    client: Seafile,
}

fn to_json(value: impl Serialize) -> Result<Value, HubError> {
    serde_json::to_value(value).map_err(|e| HubError::malformed(SERVICE, e))
}

/// A folder or file path: absolute, no parent traversal.
fn path_arg<'a>(args: &Args<'a>, key: &str) -> Result<&'a str, HubError> {
    let path = args.str(key)?;
    if !path.starts_with('/') || path.split('/').any(|part| part == "..") {
        return Err(HubError::BadArgs {
            op: key.to_owned(),
            reason: format!("'{key}' must be an absolute path without '..'"),
        });
    }
    Ok(path)
}

/// A single file or folder name: no separators.
fn name_arg<'a>(args: &Args<'a>, key: &str) -> Result<&'a str, HubError> {
    let name = args.str(key)?;
    if name.is_empty() || name.contains('/') || name == "." || name == ".." {
        return Err(HubError::BadArgs {
            op: key.to_owned(),
            reason: format!("'{key}' must be a plain name"),
        });
    }
    Ok(name)
}

/// Seafile repo ids are UUIDs; anything else never reaches a URL.
fn repo_arg<'a>(args: &Args<'a>) -> Result<&'a str, HubError> {
    let repo = args.str("repo")?;
    let valid = repo.len() == 36 && repo.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-');
    if valid {
        Ok(repo)
    } else {
        Err(HubError::BadArgs {
            op: "repo".to_owned(),
            reason: "'repo' must be a library id".to_owned(),
        })
    }
}

impl Files {
    fn overview(&self) -> Result<Value, HubError> {
        let account: model::AccountInfo = self.client.get_json("/api2/account/info/", "account")?;
        let repos: model::ReposAnswer = self.client.get_json("/api/v2.1/repos/", "libraries")?;
        Ok(json!({"account": model::Account::from(account), "libraries": model::libraries(repos)}))
    }

    fn ls(&self, args: &Args<'_>) -> Result<Value, HubError> {
        let repo = repo_arg(args)?;
        let path = path_arg(args, "path")?;
        let answer: model::DirAnswer = self.client.get_json(
            &format!(
                "/api/v2.1/repos/{repo}/dir/?p={}&with_thumbnail=true",
                q(path)
            ),
            "folder",
        )?;
        to_json(model::entries(answer, path))
    }

    fn mkdir(&self, args: &Args<'_>) -> Result<Value, HubError> {
        let repo = repo_arg(args)?;
        let path = path_arg(args, "path")?;
        self.client.act(
            Method::Post,
            &format!("/api2/repos/{repo}/dir/?p={}", q(path)),
            &[("operation", "mkdir")],
            "folder",
            &[],
        )?;
        Ok(json!({"ok": true}))
    }

    fn rename(&self, args: &Args<'_>) -> Result<Value, HubError> {
        let repo = repo_arg(args)?;
        let path = path_arg(args, "path")?;
        let name = name_arg(args, "name")?;
        let kind = if args.opt_bool("isDir").unwrap_or(false) {
            "dir"
        } else {
            "file"
        };
        self.client.act(
            Method::Post,
            &format!("/api2/repos/{repo}/{kind}/?p={}", q(path)),
            &[("operation", "rename"), ("newname", name)],
            "item",
            RENAME_OK,
        )?;
        Ok(json!({"ok": true}))
    }

    fn delete(&self, args: &Args<'_>) -> Result<Value, HubError> {
        let repo = repo_arg(args)?;
        let path = path_arg(args, "path")?;
        if path == "/" {
            return Err(HubError::BadArgs {
                op: "delete".into(),
                reason: "cannot delete the library root".into(),
            });
        }
        let kind = if args.opt_bool("isDir").unwrap_or(false) {
            "dir"
        } else {
            "file"
        };
        self.client.act(
            Method::Delete,
            &format!("/api2/repos/{repo}/{kind}/?p={}", q(path)),
            &[],
            "item",
            &[],
        )?;
        Ok(json!({"ok": true}))
    }

    fn put(&self, args: &Args<'_>, bytes: Vec<u8>) -> Result<Value, HubError> {
        let repo = repo_arg(args)?;
        let dir = path_arg(args, "dir")?;
        let name = name_arg(args, "name")?;
        let link = self.client.link(
            &format!("/api2/repos/{repo}/upload-link/?p={}", q(dir)),
            "upload link",
        )?;
        let (content_type, body) =
            multipart::upload_body(&[("parent_dir", dir), ("replace", "1")], name, &bytes);
        let url = format!("{link}?ret-json=1");
        // The upload link carries its own one-time token: no Authorization header.
        let response = self.client.send_unauthenticated(
            HttpRequest::post(url)
                .header("Content-Type", content_type)
                .body(body),
        )?;
        let uploaded: Vec<model::Uploaded> = response.ok(SERVICE, "upload")?.json(SERVICE)?;
        to_json(uploaded)
    }
}

impl Module for Files {
    fn call(&self, op: &str, args: &Value) -> Result<Value, HubError> {
        let args = Args::new(op, args);
        match op {
            "overview" => self.overview(),
            "ls" => self.ls(&args),
            "mkdir" => self.mkdir(&args),
            "rename" => self.rename(&args),
            "delete" => self.delete(&args),
            _ => Err(HubError::UnknownOp {
                module: "files".into(),
                op: op.to_owned(),
            }),
        }
    }

    fn upload(&self, op: &str, args: &Value, body: Vec<u8>) -> Result<Value, HubError> {
        match op {
            "put" => self.put(&Args::new(op, args), body),
            _ => Err(HubError::UnknownOp {
                module: "files".into(),
                op: op.to_owned(),
            }),
        }
    }

    fn media(&self, request: &MediaRequest) -> Result<MediaResponse, HubError> {
        let params = &request.params;
        let get = |key: &str| {
            params
                .get(key)
                .map(String::as_str)
                .ok_or_else(|| HubError::BadArgs {
                    op: request.op.clone(),
                    reason: format!("'{key}' is required"),
                })
        };
        let args_value = json!({"repo": get("repo")?, "path": get("path")?});
        let args = Args::new(&request.op, &args_value);
        let repo = repo_arg(&args)?;
        let path = path_arg(&args, "path")?;
        let response = match request.op.as_str() {
            "thumb" => {
                let size = params.get("size").map_or(DEFAULT_THUMB, String::as_str);
                let url = self.client.url(&format!(
                    "/api2/repos/{repo}/thumbnail/?p={}&size={}",
                    q(path),
                    q(size)
                ));
                self.client
                    .send(&|| HttpRequest::get(url.clone()))?
                    .ok(SERVICE, "thumbnail")?
            }
            "download" => {
                let link = self.client.link(
                    &format!("/api2/repos/{repo}/file/?p={}&reuse=1", q(path)),
                    "file",
                )?;
                let mut download = HttpRequest::get(link);
                if let Some(range) = &request.range {
                    download = download.header("Range", range.clone());
                }
                self.client
                    .send_unauthenticated(download)?
                    .ok(SERVICE, "file")?
            }
            _ => {
                return Err(HubError::UnknownOp {
                    module: "files".into(),
                    op: request.op.clone(),
                });
            }
        };
        let file_name = path.rsplit('/').next().map(str::to_owned);
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
            file_name,
            body: response.body,
        })
    }
}

#[cfg(test)]
mod tests;
