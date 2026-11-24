//! The app's IPA, got in step 1, before NexSideloader needs LocalDevVPN.
//!
//! iOS runs one VPN at a time, and from step 1's dial of the iPhone on it has
//! to be LocalDevVPN, which carries nothing but 10.7.0.1. Where GitHub is
//! blocked (release downloads, in mainland China and Iran), it's reached only
//! through another VPN, which can't be on then. So NexSideloader gets the IPA
//! first, over whatever VPN is on, and step 1 then asks for LocalDevVPN when
//! nothing answers at 10.7.0.1 (preflight.rs).
//!
//! The nightly release is kept in ~/.siboot/downloads, with what it is in
//! SideStore.json or LiveContainer.json. A later run asks GitHub only for the
//! download's ETag and Last-Modified (one HEAD, no download) and downloads
//! only when the nightly has changed. When GitHub can't be reached,
//! NexSideloader installs the kept copy and says which it is.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::header::{ETAG, LAST_MODIFIED};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::fail::{Fail, Result};
use crate::sign::AppTarget;
use crate::{ui, Args, VERSION};

/// What step 1 does about the IPA.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Have it ready for step 5: downloaded now, or kept from before.
    Get,
    /// `NexSideloader check`: only see that it could be, downloading nothing.
    Look,
}

/// The IPA step 5 signs.
#[derive(Debug)]
pub struct Ipa {
    pub path: PathBuf,
    /// Which app it installs.
    pub target: AppTarget,
    /// GitHub answered in this run. Where it's blocked, that means a VPN
    /// other than LocalDevVPN is on now.
    pub github: bool,
    /// Set for an IPA from `--ipa`: its name in the run's lines.
    pub custom: bool,
}

impl Ipa {
    /// "SideStore nightly", for step 1's summary.
    pub fn name(&self) -> String {
        if self.custom {
            "your IPA".into()
        } else {
            match self.target {
                AppTarget::SideStore => "SideStore nightly".into(),
                AppTarget::LiveContainer => "SideStore+LiveContainer nightly".into(),
            }
        }
    }
}

/// How to get the IPA where GitHub is blocked. Apple's sign-in server
/// answered before this is said, so the internet itself works.
const WHERE_BLOCKED: &str =
    "Where GitHub is blocked, turn on a VPN that reaches it and run NexSideloader again: it gets \
     the app first, then asks you to switch to LocalDevVPN, and keeps it for the runs after. \
     Or put the IPA in a folder you mount in Minis (Settings › Mount External Folders) and run \
     NexSideloader --ipa /var/minis/mounts/<folder>/<app>.ipa.";

/// Where GitHub is blocked, a connection to it hangs rather than fails.
const CONNECT: Duration = Duration::from_secs(20);
/// Asking after the download is one small answer.
const LOOKUP: Duration = Duration::from_secs(20);
/// A download that sends nothing for this long has stalled.
const STALL: Duration = Duration::from_secs(30);

/// `--ipa` as a path, or the URL it names, or the chosen app's nightly release.
pub async fn get(args: &Args, mode: Mode) -> Result<Ipa> {
    get_from(args.ipa.as_deref(), &args.home, mode).await
}

async fn get_from(ipa: Option<&str>, home: &Path, mode: Mode) -> Result<Ipa> {
    match ipa {
        Some(path) if !is_url(path) => {
            let path = PathBuf::from(path);
            check_ipa(&path)?;
            ui::ok(&format!("using {}", path.display()));
            Ok(Ipa {
                path,
                target: AppTarget::SideStore,
                github: false,
                custom: true,
            })
        }
        Some(url) => {
            let target = if url.contains("LiveContainer") {
                AppTarget::LiveContainer
            } else {
                AppTarget::SideStore
            };
            from_url(url, target, &downloads(home)?, mode).await
        }
        None => match mode {
            // `check` installs nothing, so it asks nothing: it only sees that
            // GitHub would give the nightly IPAs.
            Mode::Look => look_nightly(&downloads(home)?).await,
            Mode::Get => {
                let target = ask_app_target()?;
                nightly(&downloads(home)?, target, target.download_url(), mode).await
            }
        },
    }
}

fn is_url(source: &str) -> bool {
    source.starts_with("https://") || source.starts_with("http://")
}

fn downloads(home: &Path) -> Result<PathBuf> {
    let dir = home.join("downloads");
    std::fs::create_dir_all(&dir)
        .map_err(|e| Fail::new(format!("Couldn't create {}.", dir.display()), e.to_string()))?;
    Ok(dir)
}

/// Ask the user whether to install SideStore or SideStore+LiveContainer.
fn ask_app_target() -> Result<AppTarget> {
    let answer = ui::ask_menu(
        "Which app would you like to install?",
        &[
            ("SideStore", "SideStore only"),
            ("LiveContainer", "SideStore with LiveContainer"),
        ],
    )
    .ok_or_else(|| {
        Fail::new(
            "No app selected. Please run NexSideloader again.",
            "user cancelled",
        )
    })?;
    match answer.as_str() {
        "SideStore" => Ok(AppTarget::SideStore),
        "LiveContainer" => Ok(AppTarget::LiveContainer),
        _ => Err(Fail::new(
            "Invalid choice. Please run NexSideloader again.",
            format!("unexpected answer: {answer}"),
        )),
    }
}

/// The kept nightly for `target`: its file, and what it is.
fn kept_names(target: AppTarget) -> (&'static str, &'static str) {
    match target {
        AppTarget::SideStore => ("SideStore.ipa", "SideStore.json"),
        AppTarget::LiveContainer => ("LiveContainer.ipa", "LiveContainer.json"),
    }
}

/// The chosen app's nightly: kept already, downloaded now, or, when GitHub
/// can't give it, the one kept from an earlier run. `url` is the release's
/// download address, given so tests can stand in for GitHub.
async fn nightly(dir: &Path, target: AppTarget, url: &str, mode: Mode) -> Result<Ipa> {
    let (kept_ipa, kept_info) = kept_names(target);
    let path = dir.join(kept_ipa);
    let ipa = |github: bool| Ipa {
        path: path.clone(),
        target,
        github,
        custom: false,
    };
    let kept = kept(dir, kept_ipa, kept_info);
    if let Some(k) = &kept {
        ui::detail(&format!(
            "kept from an earlier run: {} ({} bytes, SHA-256 {})",
            k.name(),
            k.size,
            k.sha256
        ));
    }

    // What becomes of the kept copy when GitHub can't give a newer one.
    let installs = match mode {
        Mode::Get => "NexSideloader installs",
        Mode::Look => "a run would install",
    };
    let app = target.name();

    ui::activity(&format!("asking GitHub about {app}'s nightly"));
    let head = match head_release(url).await {
        Ok(found) => found,
        Err(raw) => {
            let Some(k) = kept else {
                return Err(Fail::new(
                    format!(
                        "Couldn't reach GitHub, where NexSideloader gets {app}. {WHERE_BLOCKED}"
                    ),
                    raw,
                ));
            };
            ui::warn(&format!(
                "Couldn't reach GitHub, so {installs} {app} {}, downloaded on an earlier run.",
                k.name()
            ));
            ui::detail(&raw);
            return Ok(ipa(false));
        }
    };
    if kept.as_ref().is_some_and(|k| k.same_release(&head)) {
        ui::ok(&format!(
            "{app} nightly, the latest, is downloaded already"
        ));
        return Ok(ipa(true));
    }

    let got = match mode {
        Mode::Look => {
            ui::activity("reaching GitHub's download");
            reachable(url)
                .await
                .map(|()| ui::ok(&format!("GitHub answers, and {app} nightly is there to download")))
        }
        Mode::Get => {
            ui::status(&format!("downloading {app} nightly"));
            download(url, &path).await.map(|got| {
                let kept = Kept {
                    size: got.size,
                    sha256: got.sha256,
                    etag: head.etag.clone(),
                    last_modified: head.last_modified.clone(),
                };
                if let Err(e) = keep(dir, kept_info, &kept) {
                    ui::detail(&format!("couldn't write {kept_info}: {e}"));
                }
                ui::ok(&format!("downloaded {app} nightly ({})", megabytes(got.size)));
            })
        }
    };
    let raw = match got {
        Ok(()) => return Ok(ipa(true)),
        Err(raw) => raw,
    };
    let missed = match mode {
        Mode::Get => format!("download {app} nightly from GitHub"),
        Mode::Look => format!("reach {app} nightly's download on GitHub"),
    };
    let Some(k) = kept else {
        return Err(Fail::new(
            format!("Couldn't {missed}. Try again in a minute. {WHERE_BLOCKED}"),
            raw,
        ));
    };
    ui::warn(&format!(
        "Couldn't {missed}, so {installs} {}, downloaded on an earlier run.",
        k.name()
    ));
    ui::detail(&raw);
    Ok(ipa(true))
}

/// `check` with no `--ipa`: both nightly IPAs are asked after, nothing is
/// picked or downloaded. Answers as the SideStore one would.
async fn look_nightly(dir: &Path) -> Result<Ipa> {
    ui::activity("asking GitHub about the nightly releases");
    let mut answered = false;
    let mut last_err = String::new();
    for target in [AppTarget::SideStore, AppTarget::LiveContainer] {
        match head_release(target.download_url()).await {
            Ok(_) => answered = true,
            Err(e) => last_err = e,
        }
    }
    if !answered {
        return Err(Fail::new(
            format!("Couldn't reach GitHub, where NexSideloader gets the app. {WHERE_BLOCKED}"),
            last_err,
        ));
    }
    ui::ok("GitHub answers, and the nightly releases are there to download");
    Ok(Ipa {
        path: dir.join("SideStore.ipa"),
        target: AppTarget::SideStore,
        github: true,
        custom: false,
    })
}

/// An IPA from `--ipa URL`, downloaded again on every run.
async fn from_url(url: &str, target: AppTarget, dir: &Path, mode: Mode) -> Result<Ipa> {
    let host = host_of(url);
    let got = match mode {
        Mode::Look => {
            ui::activity(&format!("reaching {host}"));
            reachable(url)
                .await
                .map(|()| ui::ok(&format!("{host} answers, and the IPA is there")))
        }
        Mode::Get => {
            ui::status(&format!("downloading the IPA from {host}"));
            download(url, &dir.join("other.ipa"))
                .await
                .map(|got| ui::ok(&format!("downloaded the IPA ({})", megabytes(got.size))))
        }
    };
    got.map_err(|raw| {
        Fail::new(
            format!(
                "Couldn't download the IPA from {host}. Check the address and the internet \
                 connection."
            ),
            raw,
        )
    })?;
    Ok(Ipa {
        path: dir.join("other.ipa"),
        target,
        github: false,
        custom: true,
    })
}

/// What a HEAD on the download URL says about it now.
struct Head {
    etag: Option<String>,
    last_modified: Option<String>,
}

/// The kept nightly, while its file is still the size its description says
/// and an IPA.
fn kept(dir: &Path, ipa_name: &str, info_name: &str) -> Option<Kept> {
    let text = std::fs::read_to_string(dir.join(info_name)).ok()?;
    let kept: Kept = serde_json::from_str(&text).ok()?;
    let ipa = dir.join(ipa_name);
    let size = std::fs::metadata(&ipa).ok()?.len();
    (size == kept.size && is_zip(&ipa).unwrap_or(false)).then_some(kept)
}

/// What the kept nightly is. Written once its download is whole.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Kept {
    size: u64,
    /// For the log: it can be checked against the release's own digest.
    sha256: String,
    /// The download's ETag and Last-Modified when it was fetched: a HEAD that
    /// answers the same means the nightly hasn't changed.
    etag: Option<String>,
    last_modified: Option<String>,
}

impl Kept {
    /// "nightly", for the run's lines: the tag the URL names.
    fn name(&self) -> &'static str {
        "nightly"
    }

    /// Whether a HEAD now describes the kept download.
    fn same_release(&self, head: &Head) -> bool {
        self.etag.is_some() && self.etag == head.etag
            || self.last_modified.is_some() && self.last_modified == head.last_modified
    }
}

/// Describe the kept nightly. Through a temporary file, so a description is
/// always whole.
fn keep(dir: &Path, info_name: &str, kept: &Kept) -> std::io::Result<()> {
    let path = dir.join(info_name);
    let part = dir.join(format!("{info_name}.part"));
    let text = serde_json::to_vec_pretty(kept).map_err(std::io::Error::other)?;
    std::fs::write(&part, text)?;
    std::fs::rename(&part, &path)
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_048_576.0)
}

fn host_of(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| url.to_string())
}

fn download_client() -> std::result::Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(format!("NexSideloader/{VERSION}"))
        .connect_timeout(CONNECT)
        .read_timeout(STALL)
        .build()
        .map_err(|e| crate::error_text::error_text(&e))
}

/// The download answers, without downloading it.
async fn reachable(url: &str) -> std::result::Result<(), String> {
    let response = download_client()?
        .head(url)
        .send()
        .await
        .map_err(|e| crate::error_text::error_text(&e))?;
    match response.status() {
        status if status.is_success() => Ok(()),
        status => Err(format!(
            "{} answered {status}",
            host_of(response.url().as_str())
        )),
    }
}

/// What the download's headers say it is now, without downloading it.
async fn head_release(url: &str) -> std::result::Result<Head, String> {
    let response = download_client()?
        .head(url)
        .send()
        .await
        .map_err(|e| crate::error_text::error_text(&e))?;
    match response.status() {
        status if status.is_success() => Ok(Head {
            etag: response
                .headers()
                .get(ETAG)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string),
            last_modified: response
                .headers()
                .get(LAST_MODIFIED)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string),
        }),
        status => Err(format!(
            "{} answered {status}",
            host_of(response.url().as_str())
        )),
    }
}

/// What a whole download came to.
struct Got {
    size: u64,
    sha256: String,
}

/// `url` into `dest`, through `dest.part`, so `dest` only ever holds a whole
/// IPA and an earlier one stays until a new one has arrived. The error is the
/// raw cause, for the caller's advice.
async fn download(url: &str, dest: &Path) -> std::result::Result<Got, String> {
    ui::detail(url);
    let mut response = download_client()?
        .get(url)
        .send()
        .await
        .map_err(|e| crate::error_text::error_text(&e))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "{} answered {status}",
            host_of(response.url().as_str())
        ));
    }
    let total = response.content_length().filter(|t| *t > 0);
    let partial = dest.with_extension("ipa.part");
    let mut file = std::fs::File::create(&partial)
        .map_err(|e| format!("couldn't write {}: {e}", partial.display()))?;
    let mut hasher = Sha256::new();
    let mut received: u64 = 0;
    let arrived = loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if let Err(e) = file.write_all(&chunk) {
                    break Err(format!("couldn't write {}: {e}", partial.display()));
                }
                hasher.update(&chunk);
                received += chunk.len() as u64;
                if let Some(total) = total {
                    ui::progress("downloaded", received, total);
                }
            }
            Ok(None) => break Ok(()),
            Err(e) => {
                break Err(format!(
                    "the download broke off: {}",
                    crate::error_text::error_text(&e)
                ))
            }
        }
    };
    ui::progress_end();
    drop(file);
    let whole = arrived
        .and_then(|()| match total {
            Some(total) if total != received => Err(format!(
                "the download stopped at {received} of {total} bytes"
            )),
            _ => Ok(()),
        })
        .and_then(|()| match is_zip(&partial) {
            Ok(true) => Ok(()),
            Ok(false) => Err(format!(
                "what arrived from {} isn't an IPA (an error page, say)",
                host_of(response.url().as_str())
            )),
            Err(e) => Err(format!("couldn't read {}: {e}", partial.display())),
        })
        .and_then(|()| {
            std::fs::rename(&partial, dest)
                .map_err(|e| format!("couldn't write {}: {e}", dest.display()))
        });
    if whole.is_err() {
        let _ = std::fs::remove_file(&partial);
    }
    whole.map(|()| Got {
        size: received,
        sha256: hex::encode(hasher.finalize()),
    })
}

/// An IPA is a zip: an error page or a truncated download isn't, and would
/// only fail later, inside the signer.
fn is_zip(path: &Path) -> std::io::Result<bool> {
    let mut magic = [0u8; 4];
    let mut file = std::fs::File::open(path)?;
    match std::io::Read::read_exact(&mut file, &mut magic) {
        Ok(()) => Ok(&magic == b"PK\x03\x04"),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
        Err(e) => Err(e),
    }
}

/// `--ipa PATH` names an IPA.
fn check_ipa(path: &Path) -> Result<()> {
    match is_zip(path) {
        Ok(true) => Ok(()),
        Ok(false) => Err(Fail::new(
            format!("{} isn't an IPA.", path.display()),
            "it doesn't start as a zip does",
        )),
        Err(e) => Err(Fail::new(
            format!("Couldn't read {}.", path.display()),
            e.to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// An IPA as far as NexSideloader looks: a zip's first bytes, then `tag`
    /// so two nightlies differ.
    fn ipa_bytes(tag: &str) -> Vec<u8> {
        let mut bytes = b"PK\x03\x04".to_vec();
        bytes.extend(format!("SideStore {tag} ").repeat(500).into_bytes());
        bytes
    }

    fn home(name: &str) -> PathBuf {
        let home = std::env::temp_dir().join(format!(
            "nexsideloader-test-download-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        home
    }

    /// One answer: status line and headers, then the body. `short` sends that
    /// many bytes fewer than Content-Length says, then hangs up.
    struct Answer {
        head: String,
        body: Vec<u8>,
        short: usize,
    }

    fn file(body: Vec<u8>) -> Answer {
        Answer {
            head: format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nETag: \"tag-1\"\r\nLast-Modified: Wed, 18 Nov 2026 00:00:00 GMT\r\n",
                body.len()
            ),
            body,
            short: 0,
        }
    }

    fn status(line: &str) -> Answer {
        Answer {
            head: format!("HTTP/1.1 {line}\r\nContent-Length: 0\r\n"),
            body: Vec::new(),
            short: 0,
        }
    }

    /// A stand-in for GitHub on the loopback: `route` answers each request
    /// from its method and path. Counts the GETs, which are downloads.
    async fn github(
        route: impl Fn(&str, &str) -> Answer + Send + Sync + 'static,
    ) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let gets = Arc::new(AtomicUsize::new(0));
        let counted = gets.clone();
        let route = Arc::new(route);
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let route = route.clone();
                let counted = counted.clone();
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    let mut buf = [0u8; 1024];
                    while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                        match stream.read(&mut buf).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => request.extend_from_slice(&buf[..n]),
                        }
                    }
                    let request = String::from_utf8_lossy(&request).to_string();
                    let mut first = request.split_whitespace();
                    let (method, path) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
                    if method == "GET" {
                        counted.fetch_add(1, Ordering::SeqCst);
                    }
                    let answer = route(method, path);
                    let mut out = format!("{}Connection: close\r\n\r\n", answer.head).into_bytes();
                    if method != "HEAD" {
                        out.extend_from_slice(&answer.body[..answer.body.len() - answer.short]);
                    }
                    let _ = stream.write_all(&out).await;
                    let _ = stream.shutdown().await;
                });
            }
        });
        (format!("http://127.0.0.1:{port}"), gets)
    }

    /// GitHub serving `tag`'s nightly at `/nightly/SideStore.ipa`.
    async fn nightly(tag: &'static str) -> (String, Arc<AtomicUsize>) {
        github(move |_, path| {
            if path == "/nightly/SideStore.ipa" {
                file(ipa_bytes(tag))
            } else {
                status("404 Not Found")
            }
        })
        .await
    }

    #[tokio::test]
    async fn the_nightly_is_downloaded_once_and_kept() {
        let home = home("kept");
        let dir = home.join("downloads");
        std::fs::create_dir_all(&dir).unwrap();
        let (base, gets) = nightly("first").await;
        let url = format!("{base}/nightly/SideStore.ipa");

        let ipa = nightly(&dir, AppTarget::SideStore, &url, Mode::Get)
            .await
            .unwrap();
        assert!(ipa.github);
        assert!(!ipa.custom);
        assert_eq!(ipa.name(), "SideStore nightly");
        assert_eq!(ipa.path, dir.join("SideStore.ipa"));
        assert_eq!(std::fs::read(&ipa.path).unwrap(), ipa_bytes("first"));
        let (_, info_name) = kept_names(AppTarget::SideStore);
        assert!(kept(&dir, "SideStore.ipa", info_name).is_some());
        assert!(!dir.join("SideStore.ipa.part").exists());
        assert_eq!(gets.load(Ordering::SeqCst), 1);

        // The HEAD answers the same ETag: asked about, not downloaded again.
        let again = nightly(&dir, AppTarget::SideStore, &url, Mode::Get)
            .await
            .unwrap();
        assert!(again.github);
        assert_eq!(gets.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_changed_nightly_is_downloaded_again() {
        let home = home("changed");
        let dir = home.join("downloads");
        std::fs::create_dir_all(&dir).unwrap();
        let (base, gets) = github(|_, path| {
            if path == "/nightly/SideStore.ipa" {
                file(ipa_bytes("second"))
            } else {
                status("404 Not Found")
            }
        })
        .await;
        let url = format!("{base}/nightly/SideStore.ipa");

        // A kept copy whose description names another ETag.
        let described = Kept {
            size: ipa_bytes("first").len() as u64,
            sha256: "ab".into(),
            etag: Some("\"older\"".into()),
            last_modified: None,
        };
        std::fs::write(dir.join("SideStore.ipa"), ipa_bytes("first")).unwrap();
        keep(&dir, "SideStore.json", &described).unwrap();

        let ipa = nightly(&dir, AppTarget::SideStore, &url, Mode::Get)
            .await
            .unwrap();
        assert_eq!(std::fs::read(&ipa.path).unwrap(), ipa_bytes("second"));
        assert_eq!(gets.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn blocked_github_installs_the_kept_nightly() {
        let home = home("blocked");
        let dir = home.join("downloads");
        std::fs::create_dir_all(&dir).unwrap();
        let (base, _) = nightly("first").await;
        let url = format!("{base}/nightly/SideStore.ipa");
        nightly(&dir, AppTarget::SideStore, &url, Mode::Get)
            .await
            .unwrap();

        let dead = blocked().await;
        let ipa = nightly(&dir, AppTarget::SideStore, &dead, Mode::Get)
            .await
            .unwrap();
        assert!(!ipa.github);
        assert_eq!(std::fs::read(&ipa.path).unwrap(), ipa_bytes("first"));
    }

    #[tokio::test]
    async fn blocked_github_with_nothing_kept_says_how_to_get_it() {
        let home = home("nothing");
        let dir = home.join("downloads");
        std::fs::create_dir_all(&dir).unwrap();
        let fail = nightly(&dir, AppTarget::SideStore, &blocked().await, Mode::Get)
            .await
            .unwrap_err();
        assert!(
            fail.advice.starts_with("Couldn't reach GitHub"),
            "{}",
            fail.advice
        );
        assert!(fail.advice.contains("turn on a VPN that reaches it"));
        assert!(fail.advice.contains("--ipa /var/minis/mounts/"));
        assert!(!fail.raw.is_empty());
    }

    #[tokio::test]
    async fn a_failed_download_falls_back_to_the_kept_nightly() {
        let home = home("fallback");
        let dir = home.join("downloads");
        std::fs::create_dir_all(&dir).unwrap();

        // GitHub answers the HEAD, but the download itself is refused.
        let (base, gets) = github(|_, path| match path {
            "/nightly/SideStore.ipa" => status("403 Forbidden"),
            _ => status("404 Not Found"),
        })
        .await;
        // HEAD gets 403 too here, so first keep a copy by hand.
        let described = Kept {
            size: ipa_bytes("first").len() as u64,
            sha256: "ab".into(),
            etag: None,
            last_modified: None,
        };
        std::fs::write(dir.join("SideStore.ipa"), ipa_bytes("first")).unwrap();
        keep(&dir, "SideStore.json", &described).unwrap();

        let url = format!("{base}/nightly/SideStore.ipa");
        let ipa = nightly(&dir, AppTarget::SideStore, &url, Mode::Get)
            .await
            .unwrap();
        assert_eq!(std::fs::read(&ipa.path).unwrap(), ipa_bytes("first"));
        assert_eq!(gets.load(Ordering::SeqCst), 0, "the HEAD answered 403 too");
    }

    #[tokio::test]
    async fn check_downloads_nothing() {
        let home = home("look");
        let dir = home.join("downloads");
        std::fs::create_dir_all(&dir).unwrap();
        let (base, gets) = nightly("first").await;
        let url = format!("{base}/nightly/SideStore.ipa");

        let ipa = nightly(&dir, AppTarget::SideStore, &url, Mode::Look)
            .await
            .unwrap();
        assert!(ipa.github);
        assert_eq!(gets.load(Ordering::SeqCst), 0);
        assert!(kept(&dir, "SideStore.ipa", "SideStore.json").is_none());
    }

    #[tokio::test]
    async fn an_ipa_by_url_is_downloaded_apart_from_the_kept_one() {
        let home = home("url");
        let dir = home.join("downloads");
        std::fs::create_dir_all(&dir).unwrap();
        let (other, _) = github(|_, _| file(ipa_bytes("custom"))).await;
        let ipa_url = format!("{other}/Custom.ipa");

        let ipa = from_url(&ipa_url, AppTarget::SideStore, &dir, Mode::Get)
            .await
            .unwrap();
        assert!(ipa.custom);
        assert!(!ipa.github);
        assert!(ipa.path.ends_with("downloads/other.ipa"));
        assert_eq!(std::fs::read(&ipa.path).unwrap(), ipa_bytes("custom"));

        let fail = from_url(&blocked().await, AppTarget::SideStore, &dir, Mode::Get)
            .await
            .unwrap_err();
        assert!(fail
            .advice
            .starts_with("Couldn't download the IPA from 127.0.0.1"));
    }

    #[tokio::test]
    async fn an_ipa_by_path_is_used_as_it_is() {
        let home = home("path");
        let own = home.join("Mine.ipa");
        std::fs::write(&own, ipa_bytes("mine")).unwrap();
        let ipa = get_from(Some(own.to_str().unwrap()), &home, Mode::Get)
            .await
            .unwrap();
        assert_eq!(ipa.path, own);
        assert!(ipa.custom);
        assert_eq!(ipa.name(), "your IPA");
        let missing = home.join("missing.ipa");
        assert!(get_from(Some(missing.to_str().unwrap()), &home, Mode::Get)
            .await
            .is_err());
    }

    #[test]
    fn only_a_zip_passes_as_an_ipa() {
        let dir = home("zip");
        let good = dir.join("good.ipa");
        std::fs::write(&good, b"PK\x03\x04rest").unwrap();
        assert!(check_ipa(&good).is_ok());
        let page = dir.join("page.ipa");
        std::fs::write(&page, b"<!DOCTYPE html>").unwrap();
        assert!(check_ipa(&page)
            .unwrap_err()
            .advice
            .contains("isn't an IPA"));
        let tiny = dir.join("tiny.ipa");
        std::fs::write(&tiny, b"PK").unwrap();
        assert!(check_ipa(&tiny).is_err());
        assert!(check_ipa(&dir.join("missing.ipa")).is_err());
    }

    #[test]
    fn a_kept_nightly_must_still_be_what_it_says() {
        let dir = home("described");
        let (ipa_name, info_name) = kept_names(AppTarget::SideStore);
        assert_eq!(kept(&dir, ipa_name, info_name), None);
        let bytes = ipa_bytes("nightly");
        std::fs::write(dir.join(ipa_name), &bytes).unwrap();
        assert_eq!(kept(&dir, ipa_name, info_name), None, "no description");
        let described = Kept {
            size: bytes.len() as u64,
            sha256: "ab".into(),
            etag: Some("\"tag-1\"".into()),
            last_modified: Some("Wed, 18 Nov 2026 00:00:00 GMT".into()),
        };
        keep(&dir, info_name, &described).unwrap();
        assert_eq!(
            kept(&dir, ipa_name, info_name),
            Some(described.clone())
        );
        assert!(!dir.join(format!("{info_name}.part")).exists());

        std::fs::write(dir.join(ipa_name), &bytes[..100]).unwrap();
        assert_eq!(kept(&dir, ipa_name, info_name), None, "another size");
        let mut page = b"<html>".to_vec();
        page.resize(bytes.len(), b' ');
        std::fs::write(dir.join(ipa_name), &page).unwrap();
        assert_eq!(kept(&dir, ipa_name, info_name), None, "not an IPA");
        std::fs::write(dir.join(ipa_name), &bytes).unwrap();
        std::fs::write(dir.join(info_name), "{\"size\":").unwrap();
        assert_eq!(
            kept(&dir, ipa_name, info_name),
            None,
            "an unreadable description"
        );
    }

    #[test]
    fn an_unchanged_nightly_is_the_kept_one() {
        let kept = Kept {
            size: 1,
            sha256: "ab".into(),
            etag: Some("\"tag-1\"".into()),
            last_modified: None,
        };
        let same = Head {
            etag: Some("\"tag-1\"".into()),
            last_modified: Some("anything".into()),
        };
        assert!(kept.same_release(&same));
        let changed = Head {
            etag: Some("\"tag-2\"".into()),
            last_modified: None,
        };
        assert!(!kept.same_release(&changed));
        let bare = Kept {
            etag: None,
            last_modified: None,
            ..kept.clone()
        };
        assert!(!bare.same_release(&same), "no headers kept: download again");
    }
}
