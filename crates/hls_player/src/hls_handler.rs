use hls_m3u8::{MasterPlaylist, MediaPlaylist,};
use mpeg2ts::ts::payload::Bytes;
use std::collections::VecDeque;
use url;
use anyhow::{Result, Context, Error};

struct HlsHandler<'a> {
    masterPlayList: MasterPlaylist<'a>,
    mediaPlayList: MediaPlaylist<'a>,
    streams: VecDeque<Bytes>,
}