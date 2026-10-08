//! The one place that knows this deployment: server addresses and the module list.
//!
//! Adding an app to the Hub = one line in [`modules`] (plus its crate and its UI page).

use hub_core::{Hub, ModuleDef, Ports};
use hub_directory::ApiDirectory;
use hub_http::UreqHttp;
use hub_store_file::FileSecretStore;
use std::path::Path;
use std::sync::Arc;

/// Where Kvasir's services live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    /// `hub-api` base, without trailing slash.
    pub hub_api: String,
    /// Seafile base.
    pub files: String,
    /// Navidrome Subsonic REST base.
    pub music_rest: String,
    /// Anki sync server URL shown to the user.
    pub anki_sync: String,
}

impl Endpoints {
    /// The production servers.
    pub fn production() -> Self {
        Self {
            hub_api: "https://example.org/hub".to_owned(),
            files: "https://files.example.org".to_owned(),
            music_rest: "https://music.example.org/music/rest".to_owned(),
            anki_sync: "https://anki.example.org/".to_owned(),
        }
    }
}

/// Every module of the Hub, in the order the UI shows them. Add new apps here.
pub fn modules(endpoints: &Endpoints) -> Vec<ModuleDef> {
    vec![
        hub_mod_files::definition(&endpoints.files),
        hub_mod_music::definition(&endpoints.music_rest),
        hub_mod_anki::definition(&endpoints.anki_sync),
    ]
}

/// A fully wired Hub: real HTTP, credentials in `secret_file`, sign-in through `hub-api`.
pub fn build(endpoints: &Endpoints, secret_file: &Path) -> Hub {
    let http: Arc<UreqHttp> = Arc::new(UreqHttp::new());
    let ports = Ports {
        directory: Arc::new(ApiDirectory::new(http.clone(), endpoints.hub_api.clone())),
        http,
        secrets: Arc::new(FileSecretStore::new(secret_file)),
    };
    Hub::new(modules(endpoints), ports)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn module_ids_are_unique_and_match_kvasir_app_ids() {
        let ids: Vec<_> = modules(&Endpoints::production())
            .iter()
            .map(|d| d.manifest.id)
            .collect();
        assert_eq!(ids.iter().collect::<HashSet<_>>().len(), ids.len());
        // Known grant ids from mod-identity (`App::as_str`).
        let known = [
            "kvasir", "usage", "music", "files", "anki", "hermes", "proxy",
        ];
        assert!(ids.iter().all(|id| known.contains(id)), "{ids:?}");
    }
}
