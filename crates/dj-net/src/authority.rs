//! §119: the certificate a phone's browser wants before it lends a page its
//! camera.
//!
//! A phone gives a page its camera only on a secure origin, and a page served
//! by a laptop at `192.168.1.20` is secure only over HTTPS with a certificate
//! for that address. No public authority will vouch for a private address, so
//! djmanzo is its own: an [`Authority`] made once and kept, which signs a
//! certificate for whichever address a phone reached ([`Authority::issue`]).
//!
//! A phone that has not installed the authority warns once and can be tapped
//! past; one that has installed it opens the page as it would any other site.
//!
//! # Why the authority can vouch for nothing real
//!
//! An authority installed on a phone is trusted for everything it signs, and
//! its key sits in a file on the DJ's laptop. So it is made with **name
//! constraints**: it may sign for private and link-local addresses, `.local`
//! names and `localhost`, and nothing else. Whoever took the key could vouch
//! for a laptop on a home network, and not for a bank. A verifier that keeps
//! to the standard refuses anything else it signs, and a test here holds
//! that.

use rcgen::{
    BasicConstraints, CertificateParams, CidrSubnet, DistinguishedName, DnType,
    ExtendedKeyUsagePurpose, GeneralSubtree, IsCa, Issuer, KeyPair, KeyUsagePurpose,
    NameConstraints, SanType,
};
use std::net::IpAddr;

/// How long the authority lasts: ten years. It is installed on a phone once,
/// and a phone that has to install it again every year would stop bothering.
const AUTHORITY_DAYS: i64 = 3650;

/// How long a certificate for one address lasts.
///
/// A year. A phone refuses a server certificate valid for longer than 825
/// days whoever signed it, and a new one is made every run anyway.
pub const LEAF_DAYS: i64 = 365;

/// How far back a certificate's validity starts: a phone whose clock is a day
/// behind must not be told the certificate is not valid yet.
const SKEW_DAYS: i64 = 1;

#[derive(Debug, thiserror::Error)]
pub enum AuthorityError {
    #[error("could not make a certificate: {0}")]
    Make(String),
    #[error("the kept certificate and its key do not belong together")]
    Mismatched,
}

impl From<rcgen::Error> for AuthorityError {
    fn from(why: rcgen::Error) -> Self {
        Self::Make(why.to_string())
    }
}

/// djmanzo's own certificate authority for the phone camera page.
pub struct Authority {
    name: String,
    key: KeyPair,
    der: Vec<u8>,
}

impl std::fmt::Debug for Authority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the key.
        f.debug_struct("Authority")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// A certificate for one address, with what a TLS server needs to send it.
#[derive(Debug)]
pub struct Leaf {
    /// The certificate, then the authority's.
    pub chain: Vec<Vec<u8>>,
    /// Its key, PKCS#8.
    pub key: Vec<u8>,
}

impl Authority {
    /// A new authority called `name` — the name a phone shows in its list of
    /// installed certificates, so it should say whose it is.
    ///
    /// # Errors
    /// When a key cannot be made.
    pub fn new(name: &str) -> Result<Self, AuthorityError> {
        let key = KeyPair::generate()?;
        let mut params = authority_params(name);
        let now = time::OffsetDateTime::now_utc();
        params.not_before = now - time::Duration::days(SKEW_DAYS);
        params.not_after = now + time::Duration::days(AUTHORITY_DAYS);
        let der = params.self_signed(&key)?.der().to_vec();
        Ok(Self {
            name: name.to_owned(),
            key,
            der,
        })
    }

    /// The authority kept from an earlier run: its name, its certificate and
    /// its key (PKCS#8), as [`Self::name`], [`Self::certificate`] and
    /// [`Self::key`] gave them.
    ///
    /// # Errors
    /// When the key cannot be read, or is not the certificate's.
    pub fn restore(name: &str, der: Vec<u8>, key: &[u8]) -> Result<Self, AuthorityError> {
        let key = KeyPair::try_from(key)?;
        // The certificate carries its public key whole; a key that is not
        // found in it is another authority's, and what it signed would not
        // chain to the certificate a phone installed.
        let public = key.public_key_raw();
        if !der.windows(public.len()).any(|w| w == public) {
            return Err(AuthorityError::Mismatched);
        }
        Ok(Self {
            name: name.to_owned(),
            key,
            der,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The authority's own certificate, DER: what a phone installs.
    #[must_use]
    pub fn certificate(&self) -> &[u8] {
        &self.der
    }

    /// Its key, PKCS#8 DER: to be kept where only the DJ can read it.
    #[must_use]
    pub fn key(&self) -> Vec<u8> {
        self.key.serialize_der()
    }

    /// The SHA-256 of the certificate, as phones and browsers show it:
    /// `"AB:CD:…"`.
    #[must_use]
    pub fn fingerprint(&self) -> String {
        use sha2::Digest as _;
        sha2::Sha256::digest(&self.der)
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(":")
    }

    /// A certificate for `address`, signed by this authority, with a key of
    /// its own.
    ///
    /// # Errors
    /// When a key cannot be made or the certificate signed.
    pub fn issue(&self, address: IpAddr) -> Result<Leaf, AuthorityError> {
        let key = KeyPair::generate()?;
        let mut params = CertificateParams::default();
        // Not a host name, so that no verifier reads it as one.
        let mut name = DistinguishedName::new();
        name.push(DnType::CommonName, "djmanzo phone camera");
        params.distinguished_name = name;
        params.subject_alt_names = vec![SanType::IpAddress(address)];
        params.is_ca = IsCa::ExplicitNoCa;
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        params.use_authority_key_identifier_extension = true;
        let now = time::OffsetDateTime::now_utc();
        params.not_before = now - time::Duration::days(SKEW_DAYS);
        params.not_after = now + time::Duration::days(LEAF_DAYS);

        // The issuer is rebuilt from the same parameters the authority was
        // made with: its name, its key usages and how its key is identified
        // are all a signature needs, and all are fixed by `authority_params`.
        let issuer = Issuer::new(authority_params(&self.name), &self.key);
        let leaf = params.signed_by(&key, &issuer)?;
        Ok(Leaf {
            chain: vec![leaf.der().to_vec(), self.der.clone()],
            key: key.serialize_der(),
        })
    }
}

impl Leaf {
    /// What a TLS server needs to send it.
    ///
    /// # Errors
    /// When `rustls` will not take the key.
    pub fn server_config(self) -> Result<std::sync::Arc<rustls::ServerConfig>, AuthorityError> {
        use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
        let chain = self.chain.into_iter().map(CertificateDer::from).collect();
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key));
        let provider = std::sync::Arc::new(rustls::crypto::ring::default_provider());
        let config = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .and_then(|b| b.with_no_client_auth().with_single_cert(chain, key))
            .map_err(|why| AuthorityError::Make(why.to_string()))?;
        Ok(std::sync::Arc::new(config))
    }
}

/// Everything about the authority but its key and its dates.
fn authority_params(name: &str) -> CertificateParams {
    let mut params = CertificateParams::default();
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, name);
    dn.push(DnType::OrganizationName, "djmanzo");
    params.distinguished_name = dn;
    // It signs certificates for addresses, never another authority.
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    params.name_constraints = Some(NameConstraints {
        permitted_subtrees: permitted(),
        excluded_subtrees: Vec::new(),
    });
    params
}

/// Where the authority may vouch: the private and link-local ranges, the
/// shared range carrier-grade NAT and tailnets use, `.local` and `localhost`.
///
/// A DNS entry has to be here even though the certificates carry only
/// addresses: a constraint limits only the kinds of name it lists, and an
/// authority constrained by addresses alone could still sign any host name.
fn permitted() -> Vec<GeneralSubtree> {
    let v4 = |a: [u8; 4], prefix| GeneralSubtree::IpAddress(CidrSubnet::from_v4_prefix(a, prefix));
    let v6 = |first: u8, second: u8, prefix| {
        let mut a = [0u8; 16];
        a[0] = first;
        a[1] = second;
        GeneralSubtree::IpAddress(CidrSubnet::from_v6_prefix(a, prefix))
    };
    vec![
        v4([10, 0, 0, 0], 8),
        v4([172, 16, 0, 0], 12),
        v4([192, 168, 0, 0], 16),
        v4([169, 254, 0, 0], 16),
        v4([100, 64, 0, 0], 10),
        v4([127, 0, 0, 0], 8),
        v6(0xfc, 0x00, 7),
        v6(0xfe, 0x80, 10),
        GeneralSubtree::IpAddress(CidrSubnet::from_v6_prefix(IPV6_LOOPBACK, 128)),
        GeneralSubtree::DnsName("local".to_owned()),
        GeneralSubtree::DnsName("localhost".to_owned()),
    ]
}

const IPV6_LOOPBACK: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kept_authority_comes_back_only_with_its_own_key() {
        let made = Authority::new("djmanzo test").unwrap();
        let back = Authority::restore("djmanzo test", made.certificate().to_vec(), &made.key())
            .expect("its own key");
        assert_eq!(back.fingerprint(), made.fingerprint());

        let other = Authority::new("djmanzo test").unwrap();
        assert!(matches!(
            Authority::restore("djmanzo test", made.certificate().to_vec(), &other.key()),
            Err(AuthorityError::Mismatched)
        ));
        assert!(
            Authority::restore("djmanzo test", made.certificate().to_vec(), b"no key").is_err()
        );
    }

    #[test]
    fn the_fingerprint_is_as_a_phone_shows_it() {
        let made = Authority::new("djmanzo test").unwrap();
        let print = made.fingerprint();
        assert_eq!(print.len(), 32 * 3 - 1);
        assert!(print.split(':').all(|b| {
            b.len() == 2
                && b.chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_lowercase())
        }));
    }

    /// What `authority` would sign for `name` if someone had its key —
    /// anything at all, not only what [`Authority::issue`] makes.
    fn signed_for(authority: &Authority, name: SanType) -> Vec<u8> {
        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::default();
        params.subject_alt_names = vec![name];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        params.use_authority_key_identifier_extension = true;
        let issuer = Issuer::new(authority_params(authority.name()), &authority.key);
        params.signed_by(&key, &issuer).unwrap().der().to_vec()
    }

    /// Whether a phone that installed `authority` would accept `leaf` for
    /// `name`.
    fn accepted(authority: &Authority, leaf: Vec<u8>, name: &str) -> Result<(), String> {
        use rustls::client::danger::ServerCertVerifier as _;
        use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
        let mut roots = rustls::RootCertStore::empty();
        roots
            .add(CertificateDer::from(authority.certificate().to_vec()))
            .unwrap();
        let verifier = rustls::client::WebPkiServerVerifier::builder_with_provider(
            std::sync::Arc::new(roots),
            std::sync::Arc::new(rustls::crypto::ring::default_provider()),
        )
        .build()
        .unwrap();
        verifier
            .verify_server_cert(
                &CertificateDer::from(leaf),
                &[],
                &ServerName::try_from(name.to_owned()).unwrap(),
                &[],
                UnixTime::now(),
            )
            .map(|_| ())
            .map_err(|why| format!("{why:?}"))
    }

    /// **Whoever took the authority's key could vouch for a laptop on a home
    /// network and for nothing real**: what it signs for a website's name or
    /// a public address is refused by a verifier that trusts it, and what it
    /// signs for a private address is accepted.
    #[test]
    fn a_taken_key_vouches_for_nothing_real() {
        let authority = Authority::new("djmanzo test").unwrap();
        let bank = signed_for(
            &authority,
            SanType::DnsName("bank.example".try_into().unwrap()),
        );
        let refused = accepted(&authority, bank, "bank.example").unwrap_err();
        assert!(refused.contains("NameConstraintViolation"), "{refused}");
        let public = signed_for(&authority, SanType::IpAddress("8.8.8.8".parse().unwrap()));
        let refused = accepted(&authority, public, "8.8.8.8").unwrap_err();
        assert!(refused.contains("NameConstraintViolation"), "{refused}");

        // What it is for.
        let home = authority.issue("192.168.1.20".parse().unwrap()).unwrap();
        accepted(&authority, home.chain[0].clone(), "192.168.1.20").unwrap();
        let local = signed_for(
            &authority,
            SanType::DnsName("djmanzo.local".try_into().unwrap()),
        );
        accepted(&authority, local, "djmanzo.local").unwrap();
    }

    /// A certificate names only the address it was made for.
    #[test]
    fn a_certificate_is_for_its_own_address() {
        let authority = Authority::new("djmanzo test").unwrap();
        let leaf = authority.issue("192.168.1.20".parse().unwrap()).unwrap();
        let refused = accepted(&authority, leaf.chain[0].clone(), "192.168.1.21").unwrap_err();
        assert!(refused.contains("NotValidForName"), "{refused}");
    }

    #[test]
    fn every_certificate_has_a_key_and_a_serial_of_its_own() {
        // A browser that sees two different certificates with one issuer and
        // one serial refuses the second outright, with nothing to tap past.
        // `rcgen` takes the serial from the key, so a key of its own is a
        // serial of its own.
        let made = Authority::new("djmanzo test").unwrap();
        let a = made.issue("192.168.1.20".parse().unwrap()).unwrap();
        let b = made.issue("192.168.1.20".parse().unwrap()).unwrap();
        assert_ne!(a.key, b.key);
        assert_ne!(a.chain[0], b.chain[0]);
        assert_eq!(a.chain[1], made.certificate());
    }
}
