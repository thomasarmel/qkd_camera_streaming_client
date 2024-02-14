use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use image::{ImageBuffer, Rgb};
use rodio::Sink;
use rustls::server::Acceptor;
use rustls::{ServerConfig, ServerConnection};
use rustls::qkd_config::{QkdInitialServerConfig};
use rustls::server::qkd::QkdServerConfig;
use rustls_pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use show_image::{create_window, ImageInfo, ImageView};
use qkd_camera_common_lib::PACKET_CHUNK_SIZE;

const MAX_ACCEPTABLE_IMAGE_SIZE: usize = 10_000_000;

#[show_image::main]
fn main() {
    let server_config = TestPki::new().server_config();

    let listener = std::net::TcpListener::bind(format!("0.0.0.0:{}", 4443)).unwrap();
    for stream in listener.incoming() {
        let mut stream = stream.unwrap();
        let mut acceptor = Acceptor::default();

        let accepted = loop {
            acceptor.read_tls(&mut stream).unwrap();
            if let Some(accepted) = acceptor.accept().unwrap() {
                break accepted;
            }
        };

        let conn = accepted.into_qkd_connection(server_config.clone()).unwrap();
        let mut conn = conn.complete_qkd_ack(&mut stream.try_clone().unwrap(), &mut stream.try_clone().unwrap());
        //let mut conn = accepted.into_connection(server_config.clone()).unwrap();
        conn.complete_io(&mut stream).unwrap();

        manage_stream(conn, stream);
    }
}

fn manage_stream(mut conn: ServerConnection, mut stream: TcpStream) {

    let window = create_window("image", Default::default()).unwrap();
    let (_stream, audio_output_stream_handle) = rodio::OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&audio_output_stream_handle).unwrap();

    loop {
        const USIZE_SIZE: usize = std::mem::size_of::<usize>();
        const PACKET_ANNOUNCE_SIZE: usize = USIZE_SIZE * 3; // packet size + nb chunks

        let mut packet_size_and_nb_chunks_buf = [0u8; PACKET_ANNOUNCE_SIZE];
        let received_plaintext_size = match conn.read_tls(&mut stream) {
            Ok(size) => size,
            Err(e) => {
                eprintln!("Error reading TLS: {}", e);
                break;
            }
        };
        let packet_process_result = conn.process_new_packets().unwrap();

        if packet_process_result.plaintext_bytes_to_read() < PACKET_ANNOUNCE_SIZE {
            println!("Client disconnected");
            break;
        }

        let mut read_vec = vec![0u8; received_plaintext_size];
        conn.reader().read(&mut read_vec).unwrap();

        packet_size_and_nb_chunks_buf.clone_from_slice(&read_vec[..PACKET_ANNOUNCE_SIZE]);

        let packet_size = usize::from_be_bytes(packet_size_and_nb_chunks_buf[..USIZE_SIZE].try_into().unwrap());
        let nb_chunks = usize::from_be_bytes(packet_size_and_nb_chunks_buf[USIZE_SIZE..(USIZE_SIZE * 2)].try_into().unwrap());
        let control_bytes = usize::from_be_bytes(packet_size_and_nb_chunks_buf[(USIZE_SIZE * 2)..PACKET_ANNOUNCE_SIZE].try_into().unwrap());
        if control_bytes != usize::MAX {
            eprintln!("Invalid Control bytes not MAX: {}, disconnecting client...", control_bytes);
            break;
        }
        //println!("Expecting {} bytes: {} chunks", packet_size, nb_chunks);

        let mut read_vec = Vec::with_capacity(packet_size);
        let mut packet_size_remaining = packet_size;
        for _ in 0..nb_chunks {
            let expected_chunk_size = std::cmp::min(packet_size_remaining, PACKET_CHUNK_SIZE);
            let mut chunk_vec = match read_stream_data(&mut conn, &mut stream, expected_chunk_size) {
                Ok(vec) => vec,
                Err(_) => {
                    println!("Client disconnected");
                    break;
                }
            };
            packet_size_remaining -= expected_chunk_size;
            read_vec.append(&mut chunk_vec);
        }

        let video_audio_packet: qkd_camera_common_lib::VideoAudioPacket = match postcard::from_bytes(&read_vec) {
            Ok(packet) => packet,
            Err(e) => {
                eprintln!("Error deserializing packet: {}", e);
                continue;
            }
        };

        conn.writer().write(b"ACK").unwrap();
        conn.writer().flush().unwrap();
        if conn.write_tls(&mut stream).is_err() {
            eprintln!("Error writing TLS ACK, disconnecting client...");
            break;
        }
        //println!("{:?}", conn.complete_io(&mut stream));

        let compressed_image_data = video_audio_packet.compressed_image.as_slice();
        let image_header = match turbojpeg::read_header(compressed_image_data) {
            Ok(header) => header,
            Err(e) => {
                eprintln!("Error reading image header: {}", e);
                continue;
            }
        };
        let image_allocated_space = image_header.width * image_header.height * image_header.colorspace as usize;

        if image_allocated_space > MAX_ACCEPTABLE_IMAGE_SIZE {
            eprintln!("Image too big: {} bytes", image_allocated_space);
            continue;
        }

        let decompressed_image: ImageBuffer<Rgb<u8>, Vec<u8>> = match turbojpeg::decompress_image(compressed_image_data) {
            Ok(image) => image,
            Err(e) => {
                eprintln!("Error decompressing image: {}", e);
                continue;
            }
        };
        let (width, height) = decompressed_image.dimensions();
        let image = ImageView::new(ImageInfo::rgb8(width, height), decompressed_image.as_raw());
        window.set_image("image-001", image).unwrap();


        let audio_buffer = rodio::buffer::SamplesBuffer::new(1, video_audio_packet.sound_sample_rate, video_audio_packet.sound_frame);
        sink.append(audio_buffer);
    }
    sink.sleep_until_end();
    let _ = window.run_function_wait(|window_handle| {
        window_handle.destroy();
    });
}

fn read_stream_data(conn: &mut ServerConnection, stream: &mut TcpStream, size_to_read: usize) -> Result<Vec<u8>, ()> {
    let last_connection_state = conn.process_new_packets().unwrap();
    if last_connection_state.plaintext_bytes_to_read() < size_to_read {
        //println!("Trying to read {} bytes... {}", size_to_read, conn.wants_read());
        while let Ok(size_read) = conn.read_tls(stream) {
            let t = conn.process_new_packets().unwrap();
            //println!("process result: {:?}", t);
            if t.plaintext_bytes_to_read() >= size_to_read {
                //println!("Enough bytes read");
                break;
            }
            if size_read == 0 {
                println!("EOF");
                return Err(());
            }
            //println!("Read {} bytes", size_read);
        }
    }

    let mut read_vec = vec![0u8; size_to_read];
    //let _ = conn.reader().read_exact(&mut read_vec).unwrap();
    let _ = conn.reader().read(&mut read_vec).unwrap();
    //println!("Vec read {} bytes", read_vec.len());
    Ok(read_vec)
}

struct TestPki {
    server_cert_der: CertificateDer<'static>,
    server_key_der: PrivateKeyDer<'static>,
}

impl TestPki {
    fn new() -> Self {
        let alg = &rcgen::PKCS_ECDSA_P256_SHA256;
        let mut ca_params = rcgen::CertificateParams::new(Vec::new());
        ca_params
            .distinguished_name
            .push(rcgen::DnType::OrganizationName, "Provider Server Example");
        ca_params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "Example CA");
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        ca_params.key_usages = vec![
            rcgen::KeyUsagePurpose::KeyCertSign,
            rcgen::KeyUsagePurpose::DigitalSignature,
        ];
        ca_params.alg = alg;
        let ca_cert = rcgen::Certificate::from_params(ca_params).unwrap();

        // Create a server end entity cert issued by the CA.
        let mut server_ee_params = rcgen::CertificateParams::new(vec!["localhost".to_string()]);
        server_ee_params.is_ca = rcgen::IsCa::NoCa;
        server_ee_params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
        server_ee_params.alg = alg;
        let server_cert = rcgen::Certificate::from_params(server_ee_params).unwrap();
        let server_cert_der = CertificateDer::from(
            server_cert
                .serialize_der_with_signer(&ca_cert)
                .unwrap(),
        );
        let server_key_der =
            PrivatePkcs8KeyDer::from(server_cert.serialize_private_key_der()).into();
        Self {
            server_cert_der,
            server_key_der,
        }
    }

    fn server_config(self) -> Arc<QkdServerConfig> {
        let server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_qkd_and_single_cert(vec![self.server_cert_der], self.server_key_der, &QkdInitialServerConfig::new (
            "localhost:4000",
            "data/sae3.pfx",
            "",
            )).unwrap();
            //.with_single_cert(vec![self.server_cert_der], self.server_key_der).unwrap();

        //server_config.set_key_log(Arc::new(rustls::KeyLogFile::new()));

        Arc::new(server_config)
    }
}