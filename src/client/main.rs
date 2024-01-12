use std::fmt::{Debug, Formatter};
use std::io::Write;
use std::net::TcpStream;
use std::sync::Arc;
//use std::thread;
use image::{ImageBuffer, Rgb};
use rustls::{ClientConnection, DigitallySignedStruct, Error, RootCertStore, SignatureScheme};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::qkd_config::QkdClientConfig;
use rustls_pki_types::{CertificateDer, ServerName, UnixTime};

use simple_image_interface::simple_image_interface::SimpleImageInterface;
use v4l::Device;
use v4l::video::Capture;

const DEVICE_NAME: &'static str = "/dev/video0";
const FPS: u32 = 30;
const JPEG_COMPRESS_QUALITY: i32 = 25;

fn main() {
    let mut interface: SimpleImageInterface;

    let (webcam_width, webcam_height) = get_webcam_format(DEVICE_NAME);

    interface = SimpleImageInterface::new_camera(DEVICE_NAME, webcam_width, webcam_height, FPS);


    const HOST: &'static str = "localhost";
    let mut root_store = RootCertStore::empty();
    root_store.extend(
        webpki_roots::TLS_SERVER_ROOTS
            .iter()
            .cloned(),
    );
    let mut config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_qkd(
            &QkdClientConfig::new(
                "localhost:3000",
                "data/sae1.pfx",
                "",
                2
            )).unwrap();
        /*.dangerous()
        .with_custom_certificate_verifier(Arc::new(NoVerifier {}))
        .with_no_client_auth();*/

    // Allow using SSLKEYLOGFILE.
    config.key_log = Arc::new(rustls::KeyLogFile::new());

    let server_name = HOST.try_into().unwrap();
    let mut conn = ClientConnection::new(Arc::new(config), server_name).unwrap();
    let mut sock = TcpStream::connect(format!("{}:4443", HOST)).unwrap();
    let mut tls = rustls::Stream::new(&mut conn, &mut sock);


    loop {
        let input_image = interface.get_frame();
        if input_image.is_none() {
            break;
        }
        let input_image = input_image.unwrap();
        let input_image: ImageBuffer<Rgb<u8>, Vec<u8>> = image::ImageBuffer::from_raw(webcam_width, webcam_height, input_image.as_raw().as_slice().to_vec()).unwrap();
        let compressed_image = turbojpeg::compress_image(&input_image, JPEG_COMPRESS_QUALITY, turbojpeg::Subsamp::Sub2x2).unwrap();
        println!("Compressed size: {}", compressed_image.len());
        tls.write_all(&compressed_image).unwrap();
        //thread::sleep(std::time::Duration::from_millis(1000 / FPS as u64));

    }

    conn.send_close_notify();
    conn.complete_io(&mut sock).unwrap();
}

fn get_webcam_format(device_name: &str) -> (u32, u32) {
    let dev = Device::with_path(device_name);
    let format = dev.unwrap().format().unwrap();
    (format.width, format.height)
}

struct NoVerifier {}

impl Debug for NoVerifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("NoVerifier").map_err(|_| std::fmt::Error::default())?;
        Ok(())
    }
}

impl ServerCertVerifier for NoVerifier {
    fn verify_server_cert(&self, _end_entity: &CertificateDer<'_>, _intermediates: &[CertificateDer<'_>], _server_name: &ServerName<'_>, _ocsp_response: &[u8], _now: UnixTime) -> Result<ServerCertVerified, Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(&self, _message: &[u8], _cert: &CertificateDer<'_>, _dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(&self, _message: &[u8], _cert: &CertificateDer<'_>, _dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ECDSA_NISTP256_SHA256]
    }
}