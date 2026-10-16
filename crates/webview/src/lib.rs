//! A native browser window with a throwaway session, for the two jobs that need a real engine.
//!
//! The window loads a page in a data store that lives only as long as the window. Once the proof
//! cookies appear, their header is handed back and the window closes. No browser profile ever
//! holds that session, so nothing rotates the cookies behind the app's back the way a shared
//! browser session does.
//!
//! The first job is signing in: the user works through the provider's pages and the proof cookies
//! are the session. The second is running a script on a page the app cannot reach any other way,
//! which is how YouTube's proof-of-origin token is minted; that window is hidden and its script
//! leaves the answer in a cookie the same poll reads. An OAuth sign-in is done instead once the
//! page lands on the redirect address, and that address, code and all, is what comes back.
//!
//! An account provider can finish the sign-in on an interstitial of its own — Google's security
//! check-up, say — that jumps straight to the return url and skips the hop that hands the account
//! to the provider's domain. The page then comes up signed out. When that happens, the sign-in url
//! is loaded once more: with the account already in, it only runs the hop that was skipped, which
//! is exactly what the page's own Sign in button would do.
//!
//! macOS, Windows and Linux have native backends. Every other platform reports
//! `supported() == false` and `Page::open` fails, so a caller falls back to pasting a header or
//! goes without a token.

use anyhow::Result;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as platform;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
mod native;

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod unsupported;
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
use unsupported as platform;

/// What a window is asked to do. `url` opens first, and `finish` says when the window is done.
///
/// Two kinds of window fit this. A sign-in window is shown and carries no script: the user works
/// through the provider's pages and the proof cookies, or the redirect address, are the session.
/// A scripted window is hidden and runs `script` on every page it loads, which does its work and
/// leaves the answer in a cookie named in `proof`; nothing about it is a sign-in, and the retry
/// that recovers a skipped hand-off is left out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub url: String,
    pub finish: Finish,
    pub title: String,
    /// A user agent the window presents instead of the backend's default, when the provider
    /// needs one that matches the engine the backend drives.
    pub agent: Option<String>,
    /// Javascript to run in every page the window loads, before the page's own scripts. A window
    /// with one is a scripted window and is never shown.
    pub script: Option<String>,
}

/// When a window is done.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Finish {
    /// Once the cookies for `domain` carry at least one of the `proof` names with a value.
    /// `landing` scopes cookie reads on platforms whose cookie store asks for a URL.
    Cookies {
        landing: String,
        domain: String,
        proof: Vec<String>,
    },
    /// Once the page is at an address starting with `prefix`.
    Redirect { prefix: String },
}

impl Target {
    /// Whether this target is a background page rather than a window the user works in.
    pub(crate) fn scripted(&self) -> bool {
        self.script.is_some()
    }
}

/// One cookie as the page holds it. `domain` keeps the leading dot when the browser stored one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
}

/// Where a sign-in window is between two polls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Poll {
    /// Still open, the user is not through yet.
    Pending,
    /// The user closed the window before signing in.
    Closed,
    /// The `Cookie` header for the target's domain. The window has closed itself.
    Cookies(String),
    /// The address a redirect window landed on. The window has closed itself.
    Redirect(String),
}

/// A browser window driving one [`Target`]. Open it on the main thread and poll it from there;
/// dropping it closes it.
pub struct Page {
    target: Target,
    window: platform::Window,
    /// Whether the sign-in url has already been loaded a second time. Once is the limit: a page
    /// that comes up signed out again is left to the user.
    retried: bool,
}

/// The argument that starts this executable as a throwaway check for webkit2gtk on Linux.
pub const PROBE: &str = "--webview-probe";

/// Whether this platform can open a browser window at all. On Linux the answer is only known once
/// a child process has looked for webkit2gtk, so ask it where a pause would not be felt.
pub fn supported() -> bool {
    platform::supported()
}

/// The exit code for a [`PROBE`] launch, zero when webkit2gtk loads, or none when this process was
/// started for anything else. Check it first thing in `main`.
pub fn probed() -> Option<i32> {
    #[cfg(target_os = "linux")]
    return platform::probed().map(|loads| i32::from(!loads));
    #[cfg(not(target_os = "linux"))]
    None
}

impl Page {
    /// Opens the window and starts loading the target url. Must run on the main thread.
    pub fn open(target: Target) -> Result<Self> {
        let window = platform::Window::open(&target)?;
        Ok(Self {
            target,
            window,
            retried: false,
        })
    }

    /// Checks where the window is. Call it every few hundred milliseconds until it stops answering
    /// `Pending`; the cookies or the redirect come back once, and the window closes on that poll.
    pub fn poll(&mut self) -> Poll {
        if self.window.closed() {
            return Poll::Closed;
        }
        let (domain, proof) = match &self.target.finish {
            Finish::Cookies { domain, proof, .. } => (domain.clone(), proof.clone()),
            Finish::Redirect { prefix } => {
                let Some(url) = self.window.url().filter(|url| url.starts_with(prefix)) else {
                    return Poll::Pending;
                };
                self.window.close();
                return Poll::Redirect(url);
            }
        };
        let Some(cookies) = self.window.fetch() else {
            return Poll::Pending;
        };
        let header = header(&cookies, &domain);
        match proven(&header, &proof) {
            true => {
                self.window.close();
                Poll::Cookies(header)
            }
            false => {
                self.retry(&domain);
                Poll::Pending
            }
        }
    }

    /// Loads the sign-in url again when the page is on the provider's `domain` without a session,
    /// once. Any page of the domain counts, not only the landing: a skipped hand-off can end on
    /// an error page of the provider's just as well. A scripted window has no hand-off to
    /// recover, and reloading would only throw away the work its script is in the middle of.
    fn retry(&mut self, domain: &str) {
        if self.retried || self.target.scripted() {
            return;
        }
        let Some(host) = self.window.url().as_deref().and_then(host) else {
            return;
        };
        if !matches(&host, domain) {
            return;
        }
        log::debug!("webview: {host} came up signed out, loading the sign-in url again");
        self.retried = true;
        self.window.load(&self.target.url);
    }
}

/// Joins the cookies a browser would send to `domain` into a `Cookie` header value.
fn header(cookies: &[Cookie], domain: &str) -> String {
    cookies
        .iter()
        .filter(|cookie| matches(&cookie.domain, domain))
        .map(|cookie| format!("{}={}", cookie.name, cookie.value))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Extracts the host from the absolute HTTP(S) url a webview reports for its page.
fn host(url: &str) -> Option<String> {
    let (_, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = authority
        .strip_prefix('[')
        .and_then(|host| host.split_once(']').map(|(host, _)| host))
        .unwrap_or_else(|| authority.split(':').next().unwrap_or(authority));
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// Whether a cookie stored for `stored` is sent to `domain` and its subdomains.
fn matches(stored: &str, domain: &str) -> bool {
    let stored = stored.trim_start_matches('.');
    stored == domain || stored.ends_with(&format!(".{domain}"))
}

/// Whether the header carries at least one of the proof cookies with a value. The name alone is
/// not enough: a signed-out Apple Music page already sets an empty `media-user-token`, and
/// taking that for a session closes the window before the user has typed anything.
fn proven(header: &str, proof: &[String]) -> bool {
    header
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .any(|(name, value)| !value.trim().is_empty() && proof.iter().any(|wanted| wanted == name))
}

#[cfg(test)]
mod tests {
    use super::{Cookie, header, matches, proven};

    fn cookie(name: &str, domain: &str) -> Cookie {
        Cookie {
            name: name.to_string(),
            value: format!("{name}-value"),
            domain: domain.to_string(),
        }
    }

    #[test]
    fn keeps_the_domain_and_its_subdomains() {
        assert!(matches(".youtube.com", "youtube.com"));
        assert!(matches("youtube.com", "youtube.com"));
        assert!(matches("music.youtube.com", "youtube.com"));
        assert!(!matches(".google.com", "youtube.com"));
        assert!(!matches("notyoutube.com", "youtube.com"));
    }

    #[test]
    fn joins_only_the_matching_cookies() {
        let cookies = [
            cookie("SAPISID", ".youtube.com"),
            cookie("NID", ".google.com"),
            cookie("PREF", "music.youtube.com"),
        ];
        assert_eq!(
            header(&cookies, "youtube.com"),
            "SAPISID=SAPISID-value; PREF=PREF-value"
        );
    }

    #[test]
    fn proof_needs_one_of_the_names() {
        let proof = vec!["SAPISID".to_string(), "__Secure-3PAPISID".to_string()];
        assert!(proven("VISITOR=1; __Secure-3PAPISID=x", &proof));
        assert!(!proven("VISITOR=1; PREF=x", &proof));
        assert!(!proven("", &proof));
    }

    /// Apple Music sets the cookie it delivers the account in before anyone has signed in, with
    /// nothing in it.
    #[test]
    fn an_empty_proof_cookie_is_not_a_session() {
        let proof = vec!["media-user-token".to_string()];
        assert!(!proven("geo=PL; media-user-token=", &proof));
        assert!(!proven("media-user-token=   ", &proof));
        assert!(proven("media-user-token=AbCd", &proof));
    }
}
