# ETSI GS QKD 014 video call demo

*A video conference software that relies on an ETSI GS QKD 014 TLS implementation*

---

## Quantum Key Distribution (QKD)

We offer demonstration software for making video conference calls encrypted by Quantum Key Distribution (QKD). It is a method of sharing cryptographic keys based on the exchange of qubits between two participants.

Traditionally, key exchange is done through public key cryptography. Unfortunately, attacks are regularly found in these algorithms, and there is concern that an attacker will record communications with the aim of decrypting them once the algorithm has been broken, for example with Shor's quantum algorithm.

On the other hand, the security of QKD is based on the quantum theorem of non-cloning, that is to say that an attacker cannot copy the state of a qubit without modifying it. Participants will therefore be able to detect an attacker on the fly, and intercept communications. The QKD therefore allows perfect forward secrecy.

## ETSI GS QKD 014

ETSI (European Telecommunications Standards Institute) aims to standardize telecommunications. The organization has proposed a standard for key exchange via QKD, the ETSI GS QKD 014 standard.
It defines a comprehensive framework to ensure interoperability, security, and reliability in QKD systems. The specification outlines the architecture, interfaces, key management processes, and security measures necessary for QKD deployment.

The standard is intended for cross-datacenter key sharing.
In each data center there is an entity, the **KME** (Key Management Entity), responsible for carrying out QKD with its counterparts in remote data centers. Applications that use these keys are called **SAE** (Secure Application Entity), like the software in this repository.

SAEs make requests, using classical cryptography, to KMEs in their datacenter to obtain cryptographic keys.

## QKD adaptation of Rustls

Rustls is a cryptographic library written in native Rust, which interfaces with the TLS protocol.

We have adapted this library so that it can use QKD keys retrieved from a KME rather than exchanging keys with public key cryptography. Our implementation was intended to be backwards compatible in both directions, that is to say that a classic client can connect to a TLS-QKD server, and a TLS-QKD client can connect to a classic server. The code for our adaptation is available in [our repository](https://github.com/thomasarmel/rustls/tree/qkd).

## Usage

Start by installing, in each of the data centers, our KME software which you will find in [this repository](https://github.com/thomasarmel/qkd_kme_server).

For each participant in the videoconference, you must launch the "server" (which will broadcast the sound and the remote image) then the "client", which will record the image and the sound.

### Server JSON configuration

```json
{
  "kme_address": address of the KME's' SAE interface, eg "localhost:14000",
  "kme_authentication_certificate_path": PFX certificate path used to authenticate to the KME,
  "kme_authentication_certificate_password": PFX certificate password,
  "binding_address": Visioconference server binding adress, eg "0.0.0.0:14443",
  "danger_accept_invalid_kme_cert": Boolean, should the server accept invalid KME certificates
}
```

Then launch the server with the following command:
```bash
./visio_server path_to_server_config.json
```

### Client JSON configuration

```json
{
  "kme_address": address of the KME's' SAE interface, eg "localhost:13000",
  "kme_authentication_certificate_path": PFX certificate path used to authenticate to the KME,
  "kme_authentication_certificate_password": PFX certificate password,
  "target_sae_host": hostname of the visioconference server, eg "localhost",
  "target_sae_port": port of the visioconference server, eg 14443,
  "target_sae_id": SAE id of the videioconference server, eg 12,
  "danger_accept_invalid_kme_cert": Boolean, should the server accept invalid KME certificates,
  "override_default_format": { optional
    "width": image width,
    "height": image height
  },
  "override_default_camera_fps": optional boolean, should the client override the default camera fps,
  "override_default_video_jpeg_quality": optional, JPEG compression quality (defualt 25),
  "override_default_camera_device": optional, camera device to use (default "/dev/video0"),
  "override_default_audio_frame_accumulator_length": optional how many audio frames to accumulate
          in each packet (default 2) change if you experience audio lag
}
```

Then launch the client with the following command:
```bash
./visio_client path_to_client_config.json
```