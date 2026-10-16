//! Playback for TIDAL. Not built yet: every track reports that it cannot load, which the engine
//! shows as unavailable.

use std::time::Duration;

use anyhow::{Result, bail};
use async_trait::async_trait;

use crate::engine::{self, Fetch};
use crate::{PlaybackConfig, PlaybackEvents, PlaybackFactory, Player};

pub struct Factory;

impl PlaybackFactory for Factory {
    fn start(&self, config: PlaybackConfig) -> (Box<dyn Player>, Box<dyn PlaybackEvents>) {
        engine::start(Tidal, config)
    }
}

struct Tidal;

#[async_trait]
impl Fetch for Tidal {
    type Loaded = ();
    type Source = rodio::source::Empty;

    fn name(&self) -> &'static str {
        "tidal"
    }

    async fn load(&self, _id: &str) -> Result<()> {
        bail!("tidal does not support playback yet")
    }

    fn open(&self, _id: &str, _loaded: &(), _at: Duration) -> Option<Self::Source> {
        None
    }
}
