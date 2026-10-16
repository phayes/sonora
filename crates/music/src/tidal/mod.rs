//! TIDAL: an OAuth PKCE sign-in in the app's own browser window, over the tidalrs client. Only
//! signing in is built so far; the catalog, library and playback answer that they are not
//! supported yet.

mod auth;
mod client;
mod playback;

use std::sync::Arc;

use anyhow::{Context as _, Result, anyhow};
use async_trait::async_trait;

use crate::tidal::playback::Factory;
use crate::{
    Capabilities, InputSource, MusicProvider, PromptSink, ProviderSession, Shape, SignIn,
    SignInFailure, SignInProblem, SignInPrompt, UserProfile,
};

pub use client::TidalApi;

pub struct TidalProvider;

impl TidalProvider {
    pub fn new() -> Self {
        Self
    }

    /// Runs the PKCE sign-in: the window opens TIDAL's login page, and the address it is sent
    /// back to carries the code that is traded for tokens.
    async fn authorize(prompt: PromptSink, mut input: InputSource) -> Result<ProviderSession> {
        let client = auth::client();
        let url = client
            .start_pkce(auth::REDIRECT)
            .context("cannot start the tidal sign-in")?;
        prompt(SignInPrompt::Browser {
            url,
            redirect: auth::REDIRECT.to_owned(),
        });
        let landed = input.recv().await.context("sign-in was cancelled")?;
        if let Some(error) = refusal(&landed) {
            log::warn!("tidal: the sign-in came back with {error}");
            return Err(SignInFailure(SignInProblem::Cancelled).into());
        }
        let token = client
            .finish_pkce(&landed)
            .await
            .context("cannot finish the tidal sign-in")?;
        let credentials = auth::Credentials {
            authz: token.authz(),
            name: auth::name(&token.user),
        };
        let profile = credentials
            .profile()
            .context("tidal signed in without a user")?;
        auth::store(&credentials)?;
        Ok(session(profile))
    }
}

impl Default for TidalProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MusicProvider for TidalProvider {
    fn name(&self) -> &'static str {
        "TIDAL"
    }

    fn slug(&self) -> &'static str {
        "tidal"
    }

    fn reach(&self) -> Option<String> {
        Some("api.tidal.com".to_owned())
    }

    fn sign_in_options(&self) -> Vec<SignIn> {
        vec![SignIn::Default]
    }

    fn stored(&self) -> bool {
        auth::load().is_some()
    }

    fn public_art(&self) -> bool {
        // covers live on resources.tidal.com, open to anyone with the url
        true
    }

    /// Brings the stored account back without a round trip. An expired access token is
    /// refreshed by the first request that runs into it.
    async fn restore(&self) -> Result<Option<ProviderSession>> {
        Ok(auth::load()
            .and_then(|credentials| credentials.profile())
            .map(session))
    }

    async fn sign_in(
        &self,
        method: SignIn,
        prompt: PromptSink,
        input: InputSource,
    ) -> Result<ProviderSession> {
        match method {
            SignIn::Default => Self::authorize(prompt, input).await,
            SignIn::Anonymous => Err(anyhow!("tidal has no anonymous sign-in")),
            SignIn::Secret => Err(anyhow!("tidal does not sign in with cookies")),
            SignIn::Path(_) => Err(anyhow!("tidal does not sign in with a folder path")),
            SignIn::Credentials { .. } => {
                Err(anyhow!("tidal does not sign in with a server address"))
            }
        }
    }

    fn sign_out(&self) {
        auth::forget();
    }

    fn browser_sign_in(&self) -> bool {
        true
    }
}

fn session(profile: UserProfile) -> ProviderSession {
    ProviderSession {
        profile: profile.clone(),
        api: Arc::new(TidalApi::new(profile)),
        playback: Arc::new(Factory),
        shape: Shape::Saved,
        authenticated: true,
        capabilities: Capabilities::NONE,
    }
}

/// The OAuth error a redirect carries instead of a code, as when the user declines.
fn refusal(landed: &str) -> Option<String> {
    let url = reqwest::Url::parse(landed).ok()?;
    url.query_pairs()
        .find(|(key, _)| key == "error")
        .map(|(_, value)| value.into_owned())
}
