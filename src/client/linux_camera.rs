use image::{ImageBuffer, Rgb};
use v4l::Device;
use crate::camera::Camera;
use simple_image_interface::simple_image_interface::SimpleImageInterface;
use v4l::video::Capture;
use crate::json_client_config::{DEFAULT_CAMERA_DEVICE_NAME, DEFAULT_CAMERA_FPS, JsonClientConfig};

pub(crate) struct LinuxCamera {
    interface: SimpleImageInterface,
    webcam_width: u32,
    webcam_height: u32
}

impl LinuxCamera {
    fn get_webcam_format(device_name: &str) -> (u32, u32) {
        let dev = Device::with_path(device_name);
        let format = dev.unwrap().format().unwrap();
        (format.width, format.height)
    }
}

impl Camera for LinuxCamera {

    fn new(client_config: &JsonClientConfig) -> Self {
        let interface: SimpleImageInterface;
        let (webcam_width, webcam_height) = match client_config.override_default_format.as_ref() {
            Some(override_default_format) => {
                (override_default_format.width, override_default_format.height)
            },
            None => {
                Self::get_webcam_format(DEFAULT_CAMERA_DEVICE_NAME)
            }
        };
        let camera_fps = client_config.override_default_camera_fps.unwrap_or_else(|| DEFAULT_CAMERA_FPS);
        let camera_device = match client_config.override_default_camera_device.as_ref() {
            Some(override_default_camera_device) => override_default_camera_device.as_str(),
            None => DEFAULT_CAMERA_DEVICE_NAME
        };
        interface = SimpleImageInterface::new_camera(camera_device, webcam_width, webcam_height, camera_fps);
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