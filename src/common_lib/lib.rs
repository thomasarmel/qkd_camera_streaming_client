use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct VideoAudioPacket {
    pub compressed_image: Vec<u8>,
    pub sound_frame: Vec<i16>,
    pub sound_sample_rate: u32,
}