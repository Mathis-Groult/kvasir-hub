//! The facade every shell talks to: sign in, list granted modules, call a module.
//!
//! Hides: session state, grant enforcement and module lifetime. Shells see five verbs and
//! never a module type.

use crate::{
    Credentials, Directory, HttpPort, HubError, Manifest, MediaRequest, MediaResponse, Module,
    ModuleDef, SecretStore,
};
use hub_wire::Me;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// The outside world, injected once at start-up.
pub struct Ports {
    pub http: Arc<dyn HttpPort>,
    pub secrets: Arc<dyn SecretStore>,
    pub directory: Arc<dyn Directory>,
}

/// State of one signed-in account.
struct Session {
    me: Me,
    modules: HashMap<&'static str, Arc<dyn Module>>,
}

/// The Hub: a catalog of module definitions plus at most one signed-in session.
pub struct Hub {
    defs: Vec<ModuleDef>,
    ports: Ports,
    session: RwLock<Option<Session>>,
}

impl Hub {
    /// `defs` is the catalog, in the order the UI should show them.
    pub fn new(defs: Vec<ModuleDef>, ports: Ports) -> Self {
        Self {
            defs,
            ports,
            session: RwLock::new(None),
        }
    }

    /// Signs in with credentials saved by a previous launch.
    ///
    /// Returns `Ok(None)` when nothing is saved. Credentials the server now refuses are
    /// deleted; a network failure keeps them so the next launch can retry.
    pub fn restore(&self) -> Result<Option<Me>, HubError> {
        let Some(saved) = self.ports.secrets.load()? else {
            return Ok(None);
        };
        match self.open_session(saved) {
            Ok(me) => Ok(Some(me)),
            Err(HubError::Refused(_)) => {
                self.ports.secrets.clear()?;
                Ok(None)
            }
            Err(other) => Err(other),
        }
    }

    /// Verifies the credentials, saves them and opens the session.
    pub fn sign_in(&self, credentials: Credentials) -> Result<Me, HubError> {
        let me = self.open_session(credentials.clone())?;
        self.ports.secrets.save(&credentials)?;
        Ok(me)
    }

    /// Forgets the session and the saved credentials.
    pub fn sign_out(&self) -> Result<(), HubError> {
        *self.write() = None;
        self.ports.secrets.clear()
    }

    /// The signed-in account, if any.
    pub fn me(&self) -> Option<Me> {
        self.read().as_ref().map(|s| s.me.clone())
    }

    /// Manifests of the modules this account may open, in catalog order.
    pub fn manifests(&self) -> Result<Vec<Manifest>, HubError> {
        let guard = self.read();
        let session = guard.as_ref().ok_or(HubError::NotSignedIn)?;
        Ok(self
            .defs
            .iter()
            .filter(|d| session.modules.contains_key(d.manifest.id))
            .map(|d| d.manifest.clone())
            .collect())
    }

    /// Runs `op` of `module`.
    pub fn call(&self, module: &str, op: &str, args: &Value) -> Result<Value, HubError> {
        self.module(module)?.call(op, args)
    }

    /// Runs `op` of `module` with a binary body (an upload).
    pub fn upload(
        &self,
        module: &str,
        op: &str,
        args: &Value,
        body: Vec<u8>,
    ) -> Result<Value, HubError> {
        self.module(module)?.upload(op, args, body)
    }

    /// Streams the bytes of a media operation of `module`.
    pub fn media(&self, module: &str, request: &MediaRequest) -> Result<MediaResponse, HubError> {
        self.module(module)?.media(request)
    }

    fn open_session(&self, credentials: Credentials) -> Result<Me, HubError> {
        let me = self.ports.directory.identify(&credentials)?;
        let modules = self
            .defs
            .iter()
            .filter(|d| me.apps.iter().any(|app| app == d.manifest.id))
            .map(|d| {
                let module: Arc<dyn Module> = Arc::from((d.factory)(
                    Arc::clone(&self.ports.http),
                    credentials.clone(),
                ));
                (d.manifest.id, module)
            })
            .collect();
        *self.write() = Some(Session {
            me: me.clone(),
            modules,
        });
        Ok(me)
    }

    /// Finds a granted module; the lock is released before the module does any IO.
    fn module(&self, id: &str) -> Result<Arc<dyn Module>, HubError> {
        let guard = self.read();
        let session = guard.as_ref().ok_or(HubError::NotSignedIn)?;
        if let Some(module) = session.modules.get(id) {
            return Ok(Arc::clone(module));
        }
        if self.defs.iter().any(|d| d.manifest.id == id) {
            Err(HubError::Forbidden {
                module: id.to_owned(),
            })
        } else {
            Err(HubError::UnknownModule(id.to_owned()))
        }
    }

    // A poisoned lock means another thread panicked mid-update of a plain Option; the
    // state is still a valid Option, so continuing is safe.
    fn read(&self) -> std::sync::RwLockReadGuard<'_, Option<Session>> {
        self.session.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, Option<Session>> {
        self.session.write().unwrap_or_else(|e| e.into_inner())
    }
}
