use crate::PeerPin;
use rustls::{
    DigitallySignedStruct, DistinguishedName, Error, SignatureScheme,
    client::{
        WebPkiServerVerifier,
        danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    },
    pki_types::{CertificateDer, ServerName, UnixTime},
    server::danger::{ClientCertVerified, ClientCertVerifier},
};
use std::sync::Arc;

#[derive(Debug)]
pub(crate) struct PinnedServer {
    pub webpki: Arc<WebPkiServerVerifier>,
    pub pin: PeerPin,
}

impl ServerCertVerifier for PinnedServer {
    fn verify_server_cert(
        &self,
        leaf: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        name: &ServerName<'_>,
        ocsp: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let verified = self
            .webpki
            .verify_server_cert(leaf, intermediates, name, ocsp, now)?;
        if PeerPin::of_der(leaf.as_ref()) != self.pin {
            return Err(Error::General("paired_server_fingerprint_mismatch".into()));
        }
        Ok(verified)
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.webpki.verify_tls12_signature(message, cert, dss)
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.webpki.verify_tls13_signature(message, cert, dss)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.webpki.supported_verify_schemes()
    }
    fn root_hint_subjects(&self) -> Option<&[DistinguishedName]> {
        self.webpki.root_hint_subjects()
    }
}

#[derive(Debug)]
pub(crate) struct PinnedClient {
    pub webpki: Arc<dyn ClientCertVerifier>,
    pub pins: Vec<PeerPin>,
}

impl ClientCertVerifier for PinnedClient {
    fn offer_client_auth(&self) -> bool {
        true
    }
    fn client_auth_mandatory(&self) -> bool {
        true
    }
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        self.webpki.root_hint_subjects()
    }
    fn verify_client_cert(
        &self,
        leaf: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        now: UnixTime,
    ) -> Result<ClientCertVerified, Error> {
        let verified = self.webpki.verify_client_cert(leaf, intermediates, now)?;
        if !self.pins.contains(&PeerPin::of_der(leaf.as_ref())) {
            return Err(Error::General("paired_client_fingerprint_mismatch".into()));
        }
        Ok(verified)
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.webpki.verify_tls12_signature(message, cert, dss)
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.webpki.verify_tls13_signature(message, cert, dss)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.webpki.supported_verify_schemes()
    }
}
