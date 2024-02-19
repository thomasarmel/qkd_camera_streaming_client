use image::{ImageBuffer, Rgb};

pub trait Camera {
    fn new() -> Self;
    fn get_frame(&mut self) -> ImageBuffer<Rgb<u8>, Vec<u8>>;
}