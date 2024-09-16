use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub artist_cover: String,
    pub album: String,
    pub cover: String,
    pub duration: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    pub id: String,
    pub name: String,
    pub artist: String,
    pub artist_cover: String,
    pub cover: String,
    pub songs: Vec<Song>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub artist: String,
    pub artist_cover: String,
    pub cover: String,
    pub songs: Vec<Song>,
}