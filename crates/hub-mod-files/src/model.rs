//! UI-facing shapes and their conversion from Seafile answers.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub(crate) struct AccountInfo {
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) usage: i64,
    /// Bytes; `-2` means "no limit" in Seafile.
    #[serde(default)]
    pub(crate) total: i64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Account {
    name: String,
    used: u64,
    /// `null` when the account has no quota.
    quota: Option<u64>,
}

impl From<AccountInfo> for Account {
    fn from(info: AccountInfo) -> Self {
        Self {
            name: info.name,
            used: u64::try_from(info.usage).unwrap_or(0),
            quota: u64::try_from(info.total).ok().filter(|&t| t > 0),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReposAnswer {
    #[serde(default)]
    repos: Vec<RawRepo>,
}

#[derive(Debug, Deserialize)]
struct RawRepo {
    repo_id: String,
    repo_name: String,
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    owner_name: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    last_modified: String,
    #[serde(default)]
    encrypted: bool,
    #[serde(default)]
    permission: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Library {
    id: String,
    name: String,
    /// `mine`, `shared`, `group` or `public`.
    kind: String,
    owner: String,
    size: u64,
    modified: String,
    encrypted: bool,
    writable: bool,
}

pub(crate) fn libraries(answer: ReposAnswer) -> Vec<Library> {
    answer
        .repos
        .into_iter()
        .map(|r| Library {
            id: r.repo_id,
            name: r.repo_name,
            kind: r.kind,
            owner: r.owner_name,
            size: r.size,
            modified: r.last_modified,
            encrypted: r.encrypted,
            writable: r.permission == "rw",
        })
        .collect()
}

/// One item of Seafile's `ret-json` upload answer.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct Uploaded {
    name: String,
    #[serde(default)]
    size: u64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DirAnswer {
    #[serde(default)]
    dirent_list: Vec<RawEntry>,
}

#[derive(Debug, Deserialize)]
struct RawEntry {
    #[serde(rename = "type")]
    kind: String,
    name: String,
    #[serde(default)]
    mtime: u64,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    parent_dir: String,
    #[serde(default)]
    encoded_thumbnail_src: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Entry {
    name: String,
    /// Absolute path inside the library.
    path: String,
    is_dir: bool,
    size: u64,
    /// Unix seconds.
    modified: u64,
    has_thumbnail: bool,
}

/// Folders first, then files, each A-Z ignoring case.
pub(crate) fn entries(answer: DirAnswer, dir: &str) -> Vec<Entry> {
    let mut entries: Vec<Entry> = answer
        .dirent_list
        .into_iter()
        .map(|raw| {
            let parent = if raw.parent_dir.is_empty() {
                dir
            } else {
                raw.parent_dir.as_str()
            };
            Entry {
                path: join(parent, &raw.name),
                is_dir: raw.kind == "dir",
                size: raw.size,
                modified: raw.mtime,
                has_thumbnail: raw.encoded_thumbnail_src.is_some(),
                name: raw.name,
            }
        })
        .collect();
    entries.sort_by_cached_key(|e| (!e.is_dir, e.name.to_lowercase()));
    entries
}

/// `join("/", "a") == "/a"`, `join("/a/", "b") == "/a/b"`.
pub(crate) fn join(parent: &str, name: &str) -> String {
    format!("{}/{name}", parent.trim_end_matches('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_limit_account_has_no_quota() {
        let account = Account::from(AccountInfo {
            name: "a".into(),
            usage: 10,
            total: -2,
        });
        assert_eq!(account.quota, None);
        assert_eq!(account.used, 10);
    }

    #[test]
    fn quota_is_reported_when_set() {
        let account = Account::from(AccountInfo {
            name: "a".into(),
            usage: 0,
            total: 500_000_000,
        });
        assert_eq!(account.quota, Some(500_000_000));
    }

    #[test]
    fn folders_sort_before_files_and_names_ignore_case() {
        let answer: DirAnswer = serde_json::from_str(
            r#"{"dirent_list":[
              {"type":"file","name":"b.txt","size":3,"mtime":5,"parent_dir":"/"},
              {"type":"dir","name":"Zed","mtime":1,"parent_dir":"/"},
              {"type":"file","name":"A.png","size":9,"mtime":7,"parent_dir":"/","encoded_thumbnail_src":"/x"},
              {"type":"dir","name":"alpha","mtime":2,"parent_dir":"/"}]}"#,
        )
        .unwrap();
        let names: Vec<_> = entries(answer, "/")
            .iter()
            .map(|e| (e.name.clone(), e.is_dir))
            .collect();
        assert_eq!(
            names,
            [
                ("alpha".to_owned(), true),
                ("Zed".to_owned(), true),
                ("A.png".to_owned(), false),
                ("b.txt".to_owned(), false)
            ]
        );
    }

    #[test]
    fn entry_paths_are_absolute_and_thumbnail_flag_follows_the_server() {
        let answer: DirAnswer = serde_json::from_str(
            r#"{"dirent_list":[{"type":"file","name":"p.png","size":1,"mtime":1,"parent_dir":"/Docs/","encoded_thumbnail_src":"/t"}]}"#,
        )
        .unwrap();
        let e = &entries(answer, "/Docs")[0];
        assert_eq!(e.path, "/Docs/p.png");
        assert!(e.has_thumbnail);
    }

    #[test]
    fn join_handles_root_and_trailing_slashes() {
        assert_eq!(join("/", "a"), "/a");
        assert_eq!(join("/a/", "b"), "/a/b");
    }

    #[test]
    fn libraries_map_permission_to_writable() {
        let answer: ReposAnswer = serde_json::from_str(
            r#"{"repos":[{"repo_id":"r1","repo_name":"My Library","type":"mine","permission":"rw","size":5},
                         {"repo_id":"r2","repo_name":"Team","type":"shared","permission":"r","encrypted":true}]}"#,
        )
        .unwrap();
        let libs = libraries(answer);
        assert!(libs[0].writable && !libs[1].writable);
        assert!(libs[1].encrypted);
    }
}
