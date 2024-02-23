use serde::Deserialize;

pub(crate) const DEFAULT_CAMERA_DEVICE_NAME: &'static str = "/dev/video0";
pub(crate) const DEFAULT_CAMERA_FPS: u32 = 30;
/// How many audio frames og length 512 to accumulate before sending them to the server
pub(crate) const DEFAULT_AUDIO_FRAME_ACCUMULATOR_LENGTH: usize = 2;

#[derive(Debug, Deserialize)]
pub(crate) struct JsonClientConfig {
    pub(crate) kme_address: String,
    pub(crate) kme_authentication_certificate_path: String,
    pub(crate) kme_authentication_certificate_password: String,
    pub(crate) target_sae_host: String,
    pub(crate) target_sae_port: u16,
    pub(crate) target_sae_id: i64,
    pub(crate) danger_accept_invalid_kme_cert: bool, // TODO audio frame length too for lag ?
    pub(crate) override_default_format: Option<JsonCameraFormatConfig>,
    pub(crate) override_default_camera_fps: Option<u32>,
    pub(crate) override_default_video_jpeg_quality: Option<i32>,
    pub(crate) override_default_camera_device: Option<String>,
    pub(crate) override_default_audio_frame_accumulator_length: Option<usize>
}

#[derive(Debug, Deserialize)]
pub(crate) struct JsonCameraFormatConfig {
    pub(crate) width: u32,
    pub(crate) height: u32
}