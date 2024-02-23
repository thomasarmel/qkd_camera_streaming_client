use image::{ImageBuffer, Rgb};
use crate::json_client_config::JsonClientConfig;

pub trait Camera {
    fn new(client_config: &JsonClientConfig) -> Self;
    fn get_frame(&mut self) -> ImageBuffer<Rgb<u8>, Vec<u8>>;
}