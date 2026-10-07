//! hub-api shell: HTTP listener, clock, and the verifier that runs `kvasir accounts login`.

use anyhow::{Context, Result};
use clap::Parser;
use hub_api::{
    CredentialCache, Provisioner, Services, Verdict, Verifier, VerifierError, handle,
    parse_cli_login,
};
use hub_wire::Me;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

/// Server companion of the Kvasir Hub app.
#[derive(Parser)]
struct Cli {
    /// Address to listen on (loopback only: nginx is the public face).
    #[arg(long, default_value = "127.0.0.1:4550")]
    listen: String,
    /// The `kvasir` binary.
    #[arg(long, env = "ACCOUNTS_BIN")]
    kvasir_bin: PathBuf,
    /// accounts.db
    #[arg(long, env = "ACCOUNTS_DB")]
    accounts_db: PathBuf,
    /// accounts.key
    #[arg(long, env = "ACCOUNTS_KEY")]
    accounts_key: PathBuf,
    /// Navidrome web page used to create a user on first sign-in.
    #[arg(long, default_value = "http://127.0.0.1:4533/music/app/")]
    navidrome_url: String,
}

/// Runs `kvasir accounts login` with the password on stdin (never argv).
struct CliVerifier {
    bin: PathBuf,
    db: PathBuf,
    key: PathBuf,
}

impl Verifier for CliVerifier {
    fn verify(&self, username: &str, password: &str) -> Result<Verdict, VerifierError> {
        let mut child = Command::new(&self.bin)
            .arg("--db")
            .arg(&self.db)
            .arg("accounts")
            .arg("--secret-file")
            .arg(&self.key)
            .arg("login")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| VerifierError::Unavailable(e.to_string()))?;
        let input = serde_json::json!({"username": username, "password": password}).to_string();
        child
            .stdin
            .take()
            .ok_or_else(|| VerifierError::Unavailable("no stdin".into()))?
            .write_all(input.as_bytes())
            .map_err(|e| VerifierError::Unavailable(e.to_string()))?;
        let output = child
            .wait_with_output()
            .map_err(|e| VerifierError::Unavailable(e.to_string()))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(VerifierError::Unavailable(
                stderr.trim().chars().take(200).collect(),
            ));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("");
        parse_cli_login(line)
    }
}

/// Navidrome creates a user when `Remote-User` first opens its web page; the Subsonic
/// endpoints refuse a user that has never done so. This performs that first visit.
struct NavidromeProvisioner {
    url: String,
}

impl Provisioner for NavidromeProvisioner {
    fn ensure(&self, me: &Me) -> Result<(), String> {
        if !me.apps.iter().any(|app| app == "music") {
            return Ok(());
        }
        // Navidrome trusts Remote-User only from the docker gateway (172.19.0.1); a call to
        // 127.0.0.1 through docker-proxy arrives from that address.
        let response = ureq::get(&self.url)
            .header("Remote-User", &me.username)
            .call()
            .map_err(|e| e.to_string())?;
        match response.status().as_u16() {
            200..=299 => Ok(()),
            status => Err(format!("HTTP {status}")),
        }
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let verifier = CliVerifier {
        bin: cli.kvasir_bin,
        db: cli.accounts_db,
        key: cli.accounts_key,
    };
    let cache = CredentialCache::default();
    let provisioner = NavidromeProvisioner {
        url: cli.navidrome_url,
    };
    let server = tiny_http::Server::http(&cli.listen)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .with_context(|| format!("listening on {}", cli.listen))?;
    eprintln!("[hub-api] listening on {}", cli.listen);
    for request in server.incoming_requests() {
        let authorization = request
            .headers()
            .iter()
            .find(|h| h.field.equiv("Authorization"))
            .map(|h| h.value.as_str().to_owned());
        let path = request.url().split('?').next().unwrap_or("").to_owned();
        let reply = handle(
            request.method().as_str(),
            &path,
            authorization.as_deref(),
            Services {
                verifier: &verifier,
                cache: &cache,
                provisioner: &provisioner,
            },
            now(),
        );
        let mut response =
            tiny_http::Response::from_string(reply.body).with_status_code(reply.status);
        response.add_header(header("Content-Type", "application/json"));
        response.add_header(header("Cache-Control", "no-store"));
        if reply.www_authenticate {
            response.add_header(header(
                "WWW-Authenticate",
                "Basic realm=\"Kvasir\", charset=\"UTF-8\"",
            ));
        }
        if let Err(e) = request.respond(response) {
            eprintln!("[hub-api] respond failed: {e}");
        }
    }
    Ok(())
}

fn header(name: &str, value: &str) -> tiny_http::Header {
    // Names and values here are ASCII literals chosen in this file.
    tiny_http::Header::from_bytes(name, value).expect("static header is valid")
}
