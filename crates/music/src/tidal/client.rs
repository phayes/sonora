//! TIDAL's side of [`MusicApi`]. Only the signed-in profile is answered so far; everything else
//! reports that it is not supported yet.

use std::collections::HashMap;

use anyhow::{Result, bail};
use async_trait::async_trait;

use crate::{
    Album, AlbumDetail, Artist, ArtistProfile, MediaKind, MusicApi, Playlist, PlaylistDetail,
    SavedArtist, Track, UserProfile, escape,
};

/// A signed-in TIDAL account.
#[derive(Clone)]
pub struct TidalApi {
    profile: UserProfile,
}

impl TidalApi {
    pub(crate) fn new(profile: UserProfile) -> Self {
        Self { profile }
    }
}

#[async_trait]
impl MusicApi for TidalApi {
    fn share_url(&self, kind: MediaKind, id: &str) -> Option<String> {
        let kind = match kind {
            MediaKind::Track => "track",
            MediaKind::Album => "album",
            MediaKind::Artist => "artist",
            MediaKind::Playlist => "playlist",
        };
        Some(format!(
            "https://tidal.com/browse/{kind}/{}",
            escape::component(id)
        ))
    }

    async fn profile(&self) -> Result<UserProfile> {
        Ok(self.profile.clone())
    }

    async fn artist(&self, _artist_id: &str) -> Result<Artist> {
        tidal_todo("artists")
    }

    async fn artist_profile(&self, _artist_id: &str) -> Result<ArtistProfile> {
        tidal_todo("artists")
    }

    async fn artist_images(&self, _ids: Vec<String>) -> Result<HashMap<String, String>> {
        tidal_todo("artist images")
    }

    async fn saved_tracks(&self) -> Result<Vec<Track>> {
        tidal_todo("saved tracks")
    }

    async fn set_track_saved(&self, _track_id: &str, _saved: bool) -> Result<()> {
        tidal_todo("saved tracks")
    }

    async fn track(&self, _track_id: &str) -> Result<Track> {
        tidal_todo("tracks")
    }

    async fn track_playcount(&self, _track_id: &str) -> Result<Option<u64>> {
        tidal_todo("playcounts")
    }

    async fn playlists(&self) -> Result<Vec<Playlist>> {
        tidal_todo("playlists")
    }

    async fn create_playlist(&self, _name: &str) -> Result<String> {
        tidal_todo("playlists")
    }

    async fn rename_playlist(&self, _playlist_id: &str, _name: &str) -> Result<()> {
        tidal_todo("playlists")
    }

    async fn delete_playlist(&self, _playlist_id: &str) -> Result<()> {
        tidal_todo("playlists")
    }

    async fn remove_playlist_from_library(&self, _playlist_id: &str) -> Result<()> {
        tidal_todo("playlists")
    }

    async fn add_playlist_to_library(&self, _playlist_id: &str) -> Result<()> {
        tidal_todo("playlists")
    }

    async fn set_playlist_public(&self, _playlist_id: &str, _public: bool) -> Result<()> {
        tidal_todo("playlists")
    }

    async fn add_track_to_playlist(&self, _playlist_id: &str, _track_id: &str) -> Result<()> {
        tidal_todo("playlists")
    }

    async fn remove_track_from_playlist(&self, _playlist_id: &str, _track_id: &str) -> Result<()> {
        tidal_todo("playlists")
    }

    async fn saved_albums(&self) -> Result<Vec<Album>> {
        tidal_todo("saved albums")
    }

    async fn set_album_saved(&self, _album_id: &str, _saved: bool) -> Result<()> {
        tidal_todo("saved albums")
    }

    async fn saved_artists(&self) -> Result<Vec<SavedArtist>> {
        tidal_todo("saved artists")
    }

    async fn set_artist_saved(&self, _artist_id: &str, _saved: bool) -> Result<()> {
        tidal_todo("saved artists")
    }

    async fn album(&self, _album_id: &str) -> Result<AlbumDetail> {
        tidal_todo("albums")
    }

    async fn album_tracks(&self, _album_id: &str) -> Result<Vec<Track>> {
        tidal_todo("albums")
    }

    async fn playlist(&self, _playlist_id: &str) -> Result<PlaylistDetail> {
        tidal_todo("playlists")
    }

    async fn playlist_tracks(&self, _playlist_id: &str) -> Result<Vec<Track>> {
        tidal_todo("playlists")
    }

    async fn playlist_covers(&self, _playlist_id: &str, _wanted: usize) -> Result<Vec<String>> {
        tidal_todo("playlists")
    }

    async fn track_radio(
        &self,
        _track_id: &str,
        _from: Option<&str>,
    ) -> Result<(Vec<Track>, Option<String>)> {
        tidal_todo("radio")
    }

    async fn search(&self, _query: &str) -> Result<Vec<Track>> {
        tidal_todo("search")
    }
}

/// The error every part of the api that is not built yet answers with.
fn tidal_todo<T>(what: &str) -> Result<T> {
    bail!("This is a TODO. Tidal does not support {what} yet")
}
