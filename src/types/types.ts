export interface Song {
  id: string;
  title: string;
  artist: string;
  artistCover: string;
  album: string;
  cover: string;
  duration: number;
}

export interface Album {
  id: string;
  name: string;
  artist: string;
  artistCover: string;
  cover: string;
  songs: Song[];
}

export interface Playlist {
  id: string;
  name: string;
  artist: string;
  artistCover: string;
  cover: string;
  songs: Song[];
}