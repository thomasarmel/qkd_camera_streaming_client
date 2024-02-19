use image::{ImageBuffer, Rgb};
use v4l::Device;
use crate::camera::Camera;
use simple_image_interface::simple_image_interface::SimpleImageInterface;
use v4l::video::Capture;

pub(crate) struct LinuxCamera {
    interface: SimpleImageInterface,
    webcam_width: u32,
    webcam_height: u32
}

impl LinuxCamera {

    const DEVICE_NAME: &'static str = "/dev/video0";
    const FPS: u32 = 30;
    fn get_webcam_format(device_name: &str) -> (u32, u32) {
        let dev = Device::with_path(device_name);
        let format = dev.unwrap().format().unwrap();
        (format.width, format.height)
    }
}

impl Camera for LinuxCamera {

    fn new() -> Self {
        let interface: SimpleImageInterface;
        let (webcam_width, webcam_height) = Self::get_webcam_format(Self::DEVICE_NAME);
        interface = SimpleImageInterface::new_camera(Self::DEVICE_NAME, webcam_width, webcam_height, Self::FPS);
        Self{
            interface,
            webcam_width,
            webcam_height
        }
    }

    fn get_frame(&mut self) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
        let input_image = self.interface.get_frame().unwrap();

        let input_image: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_raw(self.webcam_width, self.webcam_height, input_image.to_vec()).unwrap();
        input_image
    }
}