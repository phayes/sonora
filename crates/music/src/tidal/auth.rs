//! Sign-in credentials for TIDAL: the OAuth tokens a PKCE sign-in hands back, kept in the
//! provider's own cache folder and rewritten whenever tidalrs refreshes them.

use std::path::PathBuf;

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use tidalrs::{Authz, TidalClient, User};

use crate::{UserProfile, credentials};

/// The client TIDAL's Android app signs in as, with PKCE.
const CLIENT_ID: &str = "6BDSRdpK9hqEBTgU";

/// Sent only when a token is refreshed; the PKCE exchange itself needs no secret.
const CLIENT_SECRET: &str = "xeuPmY7nbpZ9IIbLAcQ93shka1VNheUAqN6IcszjTG8=";

/// Where TIDAL's login page sends the browser once the user is through.
/// We check the login URL for the code and state parameters, then close the window.
pub(crate) const REDIRECT: &str = "https://tidal.com/android/login/auth";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Credentials {
    pub authz: Authz,
    /// The account's name as TIDAL gave it at sign-in, since there is no cheap way to ask again.
    pub name: String,
}

impl Credentials {
    /// The profile these credentials stand for, or `None` when they carry no user.
    pub fn profile(&self) -> Option<UserProfile> {
        let id = self.authz.user_id?;
        Some(UserProfile {
            id: id.to_string(),
            display_name: self.name.clone(),
            avatar: None,
        })
    }
}

/// Builds a client for TIDAL's API. Every token it refreshes is written back to the stored
/// credentials.
pub(crate) fn client() -> TidalClient {
    TidalClient::new(CLIENT_ID.to_owned())
        .with_client_secret(CLIENT_SECRET)
        .with_authz_refresh_callback(refreshed)
}

/// The name to show for a TIDAL account: the first of its names that is filled in.
pub(crate) fn name(user: &User) -> String {
    [&user.full_name, &user.nickname, &user.first_name]
        .into_iter()
        .flatten()
        .map(|name| name.trim())
        .find(|name| !name.is_empty())
        .unwrap_or(&user.username)
        .to_owned()
}

pub(crate) fn load() -> Option<Credentials> {
    let bytes = std::fs::read(path()).ok()?;
    let credentials: Credentials = serde_json::from_slice(&bytes).ok()?;
    credentials.authz.user_id.is_some().then_some(credentials)
}

pub(crate) fn store(credentials: &Credentials) -> Result<()> {
    let bytes =
        serde_json::to_vec_pretty(credentials).context("cannot serialize tidal credentials")?;
    credentials::write(&path(), &bytes).context("cannot store tidal credentials")
}

pub(crate) fn forget() {
    credentials::remove(&path());
}

fn path() -> PathBuf {
    credentials::dir("tidal").join(credentials::FILE)
}

/// Writes refreshed tokens over the stored ones. A refresh that lands after the account was
/// signed out, or after another account signed in, has nothing of its own to update.
fn refreshed(authz: Authz) {
    let Some(mut stored) = load() else {
        return;
    };
    if stored.authz.user_id != authz.user_id {
        return;
    }
    stored.authz = authz;
    if let Err(error) = store(&stored) {
        log::warn!("tidal: cannot keep the refreshed tokens: {error:#}");
    }
}
