mod linux_camera;
mod camera;

use std::fmt::{Debug, Formatter};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::vec;
use rustls::{ClientConnection, DigitallySignedStruct, Error, RootCertStore, SignatureScheme};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::qkd_config::QkdClientConfig;
use rustls_pki_types::{CertificateDer, ServerName, UnixTime};

use pv_recorder::{PvRecorder, PvRecorderBuilder};
use serde::Deserialize;
use qkd_camera_common_lib::PACKET_CHUNK_SIZE;
use crate::camera::Camera;

const FPS: u32 = 30;
const JPEG_COMPRESS_QUALITY: i32 = 25;
const PV_RECORDER_FRAME_LENGTH: i32 = 512;

#[derive(Debug, Deserialize)]
struct JsonClientConfig {
    kme_address: String,
    kme_authentication_certificate_path: String,
    kme_authentication_certificate_password: String,
    target_sae_host: String,
    target_sae_port: u16,
    target_sae_id: i64,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <client_config.json>", args[0]);
        std::process::exit(1);
    }

    let client_config_str = std::fs::read_to_string(&args[1]).unwrap();
    let client_config: JsonClientConfig = serde_json::from_str(&client_config_str).unwrap();

    #[cfg(target_os = "linux")]
    let mut camera = linux_camera::LinuxCamera::new();
    #[cfg(target_os = "windows")]
    compile_error!("Windows is not yet supported");

    let sound_recorder = PvRecorderBuilder::new(PV_RECORDER_FRAME_LENGTH).init().unwrap();

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
                client_config.kme_address.as_str(),
                client_config.kme_authentication_certificate_path.as_str(),
                client_config.kme_authentication_certificate_password.as_str(),
                client_config.target_sae_id,
            )).unwrap();
        /*.dangerous()
        .with_custom_certificate_verifier(Arc::new(NoVerifier {}))
        .with_no_client_auth();*/

    // Allow using SSLKEYLOGFILE.
    config.key_log = Arc::new(rustls::KeyLogFile::new());

    let server_sae_host = client_config.target_sae_host;
    let server_sae_port = client_config.target_sae_port;
    let server_name: ServerName = server_sae_host.clone().try_into().unwrap();

    let mut conn = ClientConnection::new(Arc::new(config), server_name).unwrap();
    let mut sock = match TcpStream::connect(format!("{}:{}", server_sae_host, server_sae_port)) {
        Ok(sock) => sock,
        Err(e) => {
            eprintln!("Error connecting to server: {}", e);
            return;
        }
    };
    let mut tls = rustls::Stream::new(&mut conn, &mut sock);
    tls.conn.complete_io(&mut tls.sock).unwrap();

    sound_recorder.start().unwrap();

    match init_audio_capture_sync(&sound_recorder, 100) {
        Ok(sync_duration) => {
            println!("Audio capture synchronized in {} ms", sync_duration.as_millis());
        },
        Err(_) => {
            println!("Warning: audio capture could be not well synchronized...");
        }
    }


    loop {
        if /*input_image.is_none() ||*/ !sound_recorder.is_recording() {
            eprintln!("Error getting frame or sound recorder not recording, disconnecting client...");
            return;
        }
        let sound_frame = (0..2).fold(Vec::new(), |mut acc, _| {
            acc.append(&mut sound_recorder.read().unwrap());
            acc
        });
        let input_image = camera.get_frame();
        //println!("Sound frame time: {}", start.elapsed().as_millis());
        //println!("Sound frame size: {}", sound_frame.len());
        let compressed_image = turbojpeg::compress_image(&input_image, JPEG_COMPRESS_QUALITY, turbojpeg::Subsamp::Sub2x2).unwrap();
        //println!("Compressed size: {}", compressed_image.len());
        let audio_video_packet = qkd_camera_common_lib::VideoAudioPacket {
            compressed_image: compressed_image.to_vec(),
            sound_frame,
            sound_sample_rate: sound_recorder.sample_rate() as u32,
        };
        let packet_to_send = postcard::to_allocvec(&audio_video_packet).unwrap();
        let packet_size: usize = packet_to_send.len();
        let nb_chunk: usize = packet_size / PACKET_CHUNK_SIZE + 1;
        //println!("Packet size: {}: {} chunks", packet_size, nb_chunk);
        if tls.write_all(&[packet_size.to_be_bytes(), nb_chunk.to_be_bytes(), usize::MAX.to_be_bytes()].concat()).is_err() {
            eprintln!("Error writing packet size, disconnecting client...");
            break;
        }
        //println!("{}", tls.conn.wants_read());
        //println!("{:?}", tls.conn.read_tls(&mut tls.sock));
        tls.conn.write_tls(&mut tls.sock).unwrap();
        if tls.flush().is_err() {
            eprintln!("Error flushing data, disconnecting client...");
            break;
        }

        for packet_chunk in packet_to_send.chunks(PACKET_CHUNK_SIZE) {
            if tls.write_all(packet_chunk).is_err() {
                eprintln!("Error writing packet chunk, disconnecting client...");
                break;
            }
            if tls.flush().is_err() {
                eprintln!("Error flushing data, disconnecting client...");
                break;
            }
            if tls.conn.write_tls(&mut tls.sock).is_err() {
                eprintln!("Error writing TLS, disconnecting client...");
                break;
            }
        }
        //tls.conn.complete_io(&mut tls.sock).unwrap();
        //std::thread::sleep(std::time::Duration::from_millis(1000 / FPS as u64));
        if tls.conn.read_tls(&mut tls.sock).is_err() {
            eprintln!("Error reading TLS for ACK, disconnecting client...");
            break;
        }
        tls.conn.process_new_packets().unwrap();
        let mut buf = [0u8; 3];
        tls.conn.reader().read(&mut buf).unwrap();
        if &buf != b"ACK" {
            eprintln!("Warning: Invalid ACK, disconnecting client...");
        }

        //std::thread::sleep(std::time::Duration::from_millis(1000 / FPS as u64));
        /*println!("{:?}", tls.conn.read_tls(&mut tls.sock));
        tls.conn.process_new_packets().unwrap();
        let mut buf = [0u8; 25];
        tls.conn.reader().read(&mut buf).unwrap();
        println!("{:?}", buf);
        println!("Wants read: {}", tls.conn.wants_read());
        println!("Wants write: {}", tls.conn.wants_write());
        println!("{:?}", tls.conn.read_tls(&mut tls.sock));*/
        //tls.conn.complete_io(&mut tls.sock).unwrap();
    }

    sound_recorder.stop().unwrap();
    conn.send_close_notify();
    let _ = conn.complete_io(&mut sock);
}

/// Ensure that audio is synchronized with video by reading audio chunks until capture is initialized
fn init_audio_capture_sync(sound_recorder: &PvRecorder, max_read_loops: usize) -> Result<std::time::Duration, ()> {
    // Read ellasped time factor meaning that audio capture is initialized
    const READ_TIME_THRESHOLD: usize = 100;

    let mut previous_time: u128 = 0;
    let mut correctly_initialized = false;
    let whole_sync_start = std::time::Instant::now();
    for _ in 0..max_read_loops {
        let read_start = std::time::Instant::now();
        sound_recorder.read().unwrap();
        let read_time =  read_start.elapsed().as_micros();
        if read_time * (READ_TIME_THRESHOLD as u128) < previous_time {
            correctly_initialized = true;
            break;
        }
        previous_time = read_time;
    }
    let whole_sync_duration = whole_sync_start.elapsed();
    if correctly_initialized {
        Ok(whole_sync_duration)
    } else {
        Err(())
    }
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