//! The one place that knows this deployment: server addresses and the module list.
//!
//! Adding an app to the Hub = one line in [`modules`] (plus its crate and its UI page).

use hub_core::{Hub, ModuleDef, Ports, SiteDomain, SiteDomainError};
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
    /// The servers of one deployment: `hub-api` on the domain itself, each app on its sub-domain.
    pub fn for_site(site: &SiteDomain) -> Self {
        Self {
            hub_api: site.https_url(None, "/hub"),
            files: site.https_url(Some("files"), ""),
            music_rest: site.https_url(Some("music"), "/music/rest"),
            anki_sync: site.https_url(Some("anki"), "/"),
        }
    }
}

/// Why no site domain could be chosen.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SiteSettingError {
    #[error("no server domain is set: put it in the app's `site.txt` or build with KVASIR_DOMAIN")]
    Unset,
    #[error(transparent)]
    Invalid(#[from] SiteDomainError),
}

/// The domain to use: what the person saved in the app wins over the value built into it.
/// Empty text counts as not saved.
pub fn choose_site(
    saved: Option<&str>,
    built_in: Option<&str>,
) -> Result<SiteDomain, SiteSettingError> {
    let given = [saved, built_in]
        .into_iter()
        .flatten()
        .find(|text| !text.trim().is_empty())
        .ok_or(SiteSettingError::Unset)?;
    Ok(SiteDomain::parse(given)?)
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

    fn site() -> SiteDomain {
        SiteDomain::parse("example.org").unwrap()
    }

    #[test]
    fn every_server_address_hangs_on_the_site_domain() {
        assert_eq!(
            Endpoints::for_site(&site()),
            Endpoints {
                hub_api: "https://example.org/hub".to_owned(),
                files: "https://files.example.org".to_owned(),
                music_rest: "https://music.example.org/music/rest".to_owned(),
                anki_sync: "https://anki.example.org/".to_owned(),
            }
        );
    }

    #[test]
    fn the_saved_domain_wins_over_the_built_in_one() {
        let chosen = |saved, built_in| choose_site(saved, built_in).map(|d| d.to_string());
        assert_eq!(
            chosen(Some("mine.net\n"), Some("built.in")),
            Ok("mine.net".to_owned())
        );
        assert_eq!(chosen(None, Some("built.in")), Ok("built.in".to_owned()));
        assert_eq!(
            chosen(Some("  "), Some("built.in")),
            Ok("built.in".to_owned())
        );
    }

    #[test]
    fn without_any_domain_the_app_says_so_and_a_bad_one_is_reported() {
        assert_eq!(choose_site(None, None), Err(SiteSettingError::Unset));
        assert_eq!(
            choose_site(Some(""), Some(" ")),
            Err(SiteSettingError::Unset)
        );
        assert!(matches!(
            choose_site(Some("https://x.org"), Some("built.in")),
            Err(SiteSettingError::Invalid(SiteDomainError::NotABareHost(_)))
        ));
    }

    #[test]
    fn module_ids_are_unique_and_match_kvasir_app_ids() {
        let ids: Vec<_> = modules(&Endpoints::for_site(&site()))
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
