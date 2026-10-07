//! Kvasir Hub core: the module contract, the session facade and the ports.
//!
//! Pure logic. No network library, no file system, no UI toolkit and no app names: those
//! live in adapter crates (`hub-http`, `hub-store-file`), module crates (`hub-mod-*`) and
//! shells. See `docs/decisions/0013-hub-core-with-pluggable-modules.md`.

mod credentials;
mod error;
mod hub;
mod module;
mod ports;

pub use credentials::Credentials;
pub use error::{ErrorView, HubError, Refusal};
pub use hub::{Hub, Ports};
pub use module::{Args, Factory, Manifest, MediaRequest, MediaResponse, Module, ModuleDef};
pub use ports::{
    Body, Directory, HttpPort, HttpRequest, HttpResponse, Method, SecretStore, percent_encode,
};
