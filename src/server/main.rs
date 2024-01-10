use std::io::Read;
use std::net::TcpStream;
use std::sync::Arc;
use image::{ImageBuffer, Rgb};
use rustls::server::Acceptor;
use rustls::{ServerConfig, ServerConnection};
use rustls::qkd_config::QkdServerConfig;
use rustls_pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use show_image::{create_window, ImageInfo, ImageView};


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

        let mut conn = accepted.into_connection(server_config.clone()).unwrap();
        conn.complete_io(&mut stream).unwrap();

        manage_stream(conn, stream);
    }
}

fn manage_stream(mut conn: ServerConnection, mut stream: TcpStream) {

    let window = create_window("image", Default::default()).unwrap();

    loop {

        while let Ok(size_read) = conn.read_tls(&mut stream) {
            if size_read == 0 {
                println!("EOF");
                break;
            }
            println!("Read {} bytes", size_read);
        }

        conn.process_new_packets().unwrap();
        let mut read_vec = Vec::new();
        let _ = conn.reader().read_to_end(&mut read_vec);
        println!("Vec read {} bytes", read_vec.len());
        if read_vec.len() == 0 {
            println!("Client disconnected");
            break;
        }

        conn.complete_io(&mut stream).unwrap();

        let decompressed_image: ImageBuffer<Rgb<u8>, Vec<u8>> = match turbojpeg::decompress_image(&read_vec) {
            Ok(image) => image,
            Err(e) => {
                println!("Error decompressing image: {}", e);
                continue;
            }
        };
        let (width, height) = decompressed_image.dimensions();
        let image = ImageView::new(ImageInfo::rgb8(width, height), decompressed_image.as_raw());
        window.set_image("image-001", image).unwrap();
    }

    let _ = window.run_function_wait(|window_handle| {
        window_handle.destroy();
    });
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

    fn server_config(self) -> Arc<ServerConfig> {
        let mut server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_qkd_and_single_cert(vec![self.server_cert_der], self.server_key_der, &QkdServerConfig::new (
            "localhost:3000",
            "data/sae2.pfx",
            "",
            )).unwrap();

        server_config.key_log = Arc::new(rustls::KeyLogFile::new());

        Arc::new(server_config)
    }
}