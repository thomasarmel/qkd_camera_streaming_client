use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct VideoAudioPacket {
    pub compressed_image: Vec<u8>,
    pub sound_frame: Vec<i16>,
    pub sound_sample_rate: u32,
}

/// Size of sent packet chunks, in order to avoid sending too big packets that could overflow the server's buffer
pub const PACKET_CHUNK_SIZE: usize = 8192;