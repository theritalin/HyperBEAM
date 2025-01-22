// SPDX-License-Identifier: Apache-2.0

use super::*;

use crate::error::CertFormatError;
use openssl::pkey::{PKey, Public};
use openssl::x509::X509;

/// Structures/interfaces for SEV-SNP certificates.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Certificate(X509);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CertFormat {
    Pem,
    Der,
}

impl std::fmt::Display for CertFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pem => write!(f, "pem"),
            Self::Der => write!(f, "der"),
        }
    }
}

impl std::str::FromStr for CertFormat {
    type Err = CertFormatError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "pem" => Ok(Self::Pem),
            "der" => Ok(Self::Der),
            _ => Err(CertFormatError::UnknownFormat),
        }
    }
}

/// Wrap an X509 struct into a Certificate.
impl From<X509> for Certificate {
    fn from(x509: X509) -> Self {
        Self(x509)
    }
}

/// Unwrap the underlying X509 struct from a Certificate.
impl From<Certificate> for X509 {
    fn from(cert: Certificate) -> Self {
        cert.0
    }
}

/// Clone the underlying X509 structure from a reference to a Certificate.
impl From<&Certificate> for X509 {
    fn from(cert: &Certificate) -> Self {
        cert.0.clone()
    }
}

impl From<&X509> for Certificate {
    fn from(value: &X509) -> Self {
        Self(value.clone())
    }
}

impl From<&[X509]> for Certificate {
    /// Retrieves only the first value from the hash, ignoring all other values.
    fn from(value: &[X509]) -> Self {
        value[0].clone().into()
    }
}

impl<'a: 'b, 'b> From<&'a Certificate> for &'b X509 {
    fn from(value: &'a Certificate) -> Self {
        &value.0
    }
}

/// Verify if the public key of one Certificate signs another Certificate.
impl Verifiable for (&Certificate, &Certificate) {
    type Output = ();

    fn verify(self) -> Result<Self::Output> {
        let signer: X509 = self.0.into();
        let signee: X509 = self.1.into();

        let key: PKey<Public> = signer.public_key()?;
        let signed = signee.verify(&key)?;

        match signed {
            true => Ok(()),
            false => Err(Error::new(
                ErrorKind::Other,
                "Signer certificate does not sign signee certificate",
            )),
        }
    }
}

impl Certificate {
    /// Create a Certificate from a PEM-encoded X509 structure.
    pub fn from_pem(pem: &[u8]) -> Result<Self> {
        Ok(Self(X509::from_pem(pem)?))
    }

    /// Serialize a Certificate struct to PEM.
    pub fn to_pem(&self) -> Result<Vec<u8>> {
        Ok(self.0.to_pem()?)
    }

    /// Create a Certificate from a DER-encoded X509 structure.
    pub fn from_der(der: &[u8]) -> Result<Self> {
        Ok(Self(X509::from_der(der)?))
    }

    /// Serialize a Certificate struct to DER.
    pub fn to_der(&self) -> Result<Vec<u8>> {
        Ok(self.0.to_der()?)
    }

    /// Retrieve the underlying X509 public key for a Certificate.
    pub fn public_key(&self) -> Result<PKey<Public>> {
        Ok(self.0.public_key()?)
    }

    /// Identifies the format of a certificate based upon the first twenty-seven
    /// bytes of a byte stream. A non-PEM format assumes DER format.
    pub fn identify_format(bytes: &[u8]) -> CertFormat {
        const PEM_START: &[u8] = b"-----BEGIN CERTIFICATE-----";
        match &bytes[0..27] {
            PEM_START => CertFormat::Pem,
            _ => CertFormat::Der,
        }
    }

    /// An façade method for constructing a Certificate from raw bytes.
    pub fn from_bytes(raw_bytes: &[u8]) -> Result<Self> {
        match Self::identify_format(raw_bytes) {
            CertFormat::Pem => Self::from_pem(raw_bytes),
            CertFormat::Der => Self::from_der(raw_bytes),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_identify_format_pem() {
        let dummy_pem: &[u8] = br#"-----BEGIN CERTIFICATE-----
MIIGYzCCBBKgAwIBAgIDAQAAMEYGCSqGSIb3DQEBCjA5oA8wDQYJYIZIAWUDBAIC
BQChHDAaBgkqhkiG9w0BAQgwDQYJYIZIAWUDBAICBQCiAwIBMKMDAgEBMHsxFDAS
BgNVBAsMC0VuZ2luZWVyaW5nMQswCQYDVQQGEwJVUzEUMBIGA1UEBwwLU2FudGEg
Q2xhcmExCzAJBgNVBAgMAkNBMR8wHQYDVQQKDBZBZHZhbmNlZCBNaWNybyBEZXZp
Y2VzMRIwEAYDVQQDDAlBUkstTWlsYW4wHhcNMjAxMDIyMTcyMzA1WhcNNDUxMDIy
MTcyMzA1WjB7MRQwEgYDVQQLDAtFbmdpbmVlcmluZzELMAkGA1UEBhMCVVMxFDAS
BgNVBAcMC1NhbnRhIENsYXJhMQswCQYDVQQIDAJDQTEfMB0GA1UECgwWQWR2YW5j
ZWQgTWljcm8gRGV2aWNlczESMBAGA1UEAwwJQVJLLU1pbGFuMIICIjANBgkqhkiG
9w0BAQEFAAOCAg8AMIICCgKCAgEA0Ld52RJOdeiJlqK2JdsVmD7FktuotWwX1fNg
W41XY9Xz1HEhSUmhLz9Cu9DHRlvgJSNxbeYYsnJfvyjx1MfU0V5tkKiU1EesNFta
1kTA0szNisdYc9isqk7mXT5+KfGRbfc4V/9zRIcE8jlHN61S1ju8X93+6dxDUrG2
SzxqJ4BhqyYmUDruPXJSX4vUc01P7j98MpqOS95rORdGHeI52Naz5m2B+O+vjsC0
60d37jY9LFeuOP4Meri8qgfi2S5kKqg/aF6aPtuAZQVR7u3KFYXP59XmJgtcog05
gmI0T/OitLhuzVvpZcLph0odh/1IPXqx3+MnjD97A7fXpqGd/y8KxX7jksTEzAOg
bKAeam3lm+3yKIcTYMlsRMXPcjNbIvmsBykD//xSniusuHBkgnlENEWx1UcbQQrs
+gVDkuVPhsnzIRNgYvM48Y+7LGiJYnrmE8xcrexekBxrva2V9TJQqnN3Q53kt5vi
Qi3+gCfmkwC0F0tirIZbLkXPrPwzZ0M9eNxhIySb2npJfgnqz55I0u33wh4r0ZNQ
eTGfw03MBUtyuzGesGkcw+loqMaq1qR4tjGbPYxCvpCq7+OgpCCoMNit2uLo9M18
fHz10lOMT8nWAUvRZFzteXCm+7PHdYPlmQwUw3LvenJ/ILXoQPHfbkH0CyPfhl1j
WhJFZasCAwEAAaN+MHwwDgYDVR0PAQH/BAQDAgEGMB0GA1UdDgQWBBSFrBrRQ/fI
rFXUxR1BSKvVeErUUzAPBgNVHRMBAf8EBTADAQH/MDoGA1UdHwQzMDEwL6AtoCuG
KWh0dHBzOi8va2RzaW50Zi5hbWQuY29tL3ZjZWsvdjEvTWlsYW4vY3JsMEYGCSqG
SIb3DQEBCjA5oA8wDQYJYIZIAWUDBAICBQChHDAaBgkqhkiG9w0BAQgwDQYJYIZI
AWUDBAICBQCiAwIBMKMDAgEBA4ICAQC6m0kDp6zv4Ojfgy+zleehsx6ol0ocgVel
ETobpx+EuCsqVFRPK1jZ1sp/lyd9+0fQ0r66n7kagRk4Ca39g66WGTJMeJdqYriw
STjjDCKVPSesWXYPVAyDhmP5n2v+BYipZWhpvqpaiO+EGK5IBP+578QeW/sSokrK
dHaLAxG2LhZxj9aF73fqC7OAJZ5aPonw4RE299FVarh1Tx2eT3wSgkDgutCTB1Yq
zT5DuwvAe+co2CIVIzMDamYuSFjPN0BCgojl7V+bTou7dMsqIu/TW/rPCX9/EUcp
KGKqPQ3P+N9r1hjEFY1plBg93t53OOo49GNI+V1zvXPLI6xIFVsh+mto2RtgEX/e
pmMKTNN6psW88qg7c1hTWtN6MbRuQ0vm+O+/2tKBF2h8THb94OvvHHoFDpbCELlq
HnIYhxy0YKXGyaW1NjfULxrrmxVW4wcn5E8GddmvNa6yYm8scJagEi13mhGu4Jqh
3QU3sf8iUSUr09xQDwHtOQUVIqx4maBZPBtSMf+qUDtjXSSq8lfWcd8bLr9mdsUn
JZJ0+tuPMKmBnSH860llKk+VpVQsgqbzDIvOLvD6W1Umq25boxCYJ+TuBoa4s+HH
CViAvgT9kf/rBq1d+ivj6skkHxuzcxbk1xv6ZGxrteJxVH7KlX7YRdZ6eARKwLe4
AFZEAwoKCQ==
-----END CERTIFICATE-----"#;

        assert_eq!(Certificate::identify_format(dummy_pem), CertFormat::Pem)
    }

    #[test]
    #[should_panic]
    fn test_identify_format_panic_pem() {
        let dummy_pem: &[u8] = b"-----BEGIN CERTIFICATE---";

        assert_eq!(Certificate::identify_format(dummy_pem), CertFormat::Pem)
    }

    #[test]
    #[should_panic]
    fn test_identify_format_panic_der() {
        let dummy_der: &[u8] = &[
            0x30, 0x82, 0x06, 0x63, 0x30, 0x82, 0x04, 0x12, 0xa0, 0x03, 0x02, 0x01, 0x02, 0x02,
        ];

        assert_eq!(Certificate::identify_format(dummy_der), CertFormat::Der)
    }

    #[test]
    fn test_identify_format_der() {
        let dummy_der: &[u8] = &[
            0x30, 0x82, 0x06, 0x63, 0x30, 0x82, 0x04, 0x12, 0xa0, 0x03, 0x02, 0x01, 0x02, 0x02,
            0x03, 0x01, 0x00, 0x00, 0x30, 0x46, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d,
            0x01, 0x01, 0x0a, 0x30, 0x39, 0xa0, 0x0f, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48,
            0x01, 0x65, 0x03, 0x04, 0x02, 0x02, 0x05, 0x00, 0xa1, 0x1c, 0x30, 0x1a, 0x06, 0x09,
            0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x08, 0x30, 0x0d, 0x06, 0x09, 0x60,
            0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x02, 0x05, 0x00, 0xa2, 0x03, 0x02, 0x01,
            0x30, 0xa3, 0x03, 0x02, 0x01, 0x01, 0x30, 0x7b, 0x31, 0x14, 0x30, 0x12, 0x06, 0x03,
            0x55, 0x04, 0x0b, 0x0c, 0x0b, 0x45, 0x6e, 0x67, 0x69, 0x6e, 0x65, 0x65, 0x72, 0x69,
            0x6e, 0x67, 0x31, 0x0b, 0x30, 0x09, 0x06, 0x03, 0x55, 0x04, 0x06, 0x13, 0x02, 0x55,
            0x53, 0x31, 0x14, 0x30, 0x12, 0x06, 0x03, 0x55, 0x04, 0x07, 0x0c, 0x0b, 0x53, 0x61,
            0x6e, 0x74, 0x61, 0x20, 0x43, 0x6c, 0x61, 0x72, 0x61, 0x31, 0x0b, 0x30, 0x09, 0x06,
            0x03, 0x55, 0x04, 0x08, 0x0c, 0x02, 0x43, 0x41, 0x31, 0x1f, 0x30, 0x1d, 0x06, 0x03,
            0x55, 0x04, 0x0a, 0x0c, 0x16, 0x41, 0x64, 0x76, 0x61, 0x6e, 0x63, 0x65, 0x64, 0x20,
            0x4d, 0x69, 0x63, 0x72, 0x6f, 0x20, 0x44, 0x65, 0x76, 0x69, 0x63, 0x65, 0x73, 0x31,
            0x12, 0x30, 0x10, 0x06, 0x03, 0x55, 0x04, 0x03, 0x0c, 0x09, 0x41, 0x52, 0x4b, 0x2d,
            0x4d, 0x69, 0x6c, 0x61, 0x6e, 0x30, 0x1e, 0x17, 0x0d, 0x32, 0x30, 0x31, 0x30, 0x32,
            0x32, 0x31, 0x37, 0x32, 0x33, 0x30, 0x35, 0x5a, 0x17, 0x0d, 0x34, 0x35, 0x31, 0x30,
            0x32, 0x32, 0x31, 0x37, 0x32, 0x33, 0x30, 0x35, 0x5a, 0x30, 0x7b, 0x31, 0x14, 0x30,
            0x12, 0x06, 0x03, 0x55, 0x04, 0x0b, 0x0c, 0x0b, 0x45, 0x6e, 0x67, 0x69, 0x6e, 0x65,
            0x65, 0x72, 0x69, 0x6e, 0x67, 0x31, 0x0b, 0x30, 0x09, 0x06, 0x03, 0x55, 0x04, 0x06,
            0x13, 0x02, 0x55, 0x53, 0x31, 0x14, 0x30, 0x12, 0x06, 0x03, 0x55, 0x04, 0x07, 0x0c,
            0x0b, 0x53, 0x61, 0x6e, 0x74, 0x61, 0x20, 0x43, 0x6c, 0x61, 0x72, 0x61, 0x31, 0x0b,
            0x30, 0x09, 0x06, 0x03, 0x55, 0x04, 0x08, 0x0c, 0x02, 0x43, 0x41, 0x31, 0x1f, 0x30,
            0x1d, 0x06, 0x03, 0x55, 0x04, 0x0a, 0x0c, 0x16, 0x41, 0x64, 0x76, 0x61, 0x6e, 0x63,
            0x65, 0x64, 0x20, 0x4d, 0x69, 0x63, 0x72, 0x6f, 0x20, 0x44, 0x65, 0x76, 0x69, 0x63,
            0x65, 0x73, 0x31, 0x12, 0x30, 0x10, 0x06, 0x03, 0x55, 0x04, 0x03, 0x0c, 0x09, 0x41,
            0x52, 0x4b, 0x2d, 0x4d, 0x69, 0x6c, 0x61, 0x6e, 0x30, 0x82, 0x02, 0x22, 0x30, 0x0d,
            0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01, 0x05, 0x00, 0x03,
            0x82, 0x02, 0x0f, 0x00, 0x30, 0x82, 0x02, 0x0a, 0x02, 0x82, 0x02, 0x01, 0x00, 0xd0,
            0xb7, 0x79, 0xd9, 0x12, 0x4e, 0x75, 0xe8, 0x89, 0x96, 0xa2, 0xb6, 0x25, 0xdb, 0x15,
            0x98, 0x3e, 0xc5, 0x92, 0xdb, 0xa8, 0xb5, 0x6c, 0x17, 0xd5, 0xf3, 0x60, 0x5b, 0x8d,
            0x57, 0x63, 0xd5, 0xf3, 0xd4, 0x71, 0x21, 0x49, 0x49, 0xa1, 0x2f, 0x3f, 0x42, 0xbb,
            0xd0, 0xc7, 0x46, 0x5b, 0xe0, 0x25, 0x23, 0x71, 0x6d, 0xe6, 0x18, 0xb2, 0x72, 0x5f,
            0xbf, 0x28, 0xf1, 0xd4, 0xc7, 0xd4, 0xd1, 0x5e, 0x6d, 0x90, 0xa8, 0x94, 0xd4, 0x47,
            0xac, 0x34, 0x5b, 0x5a, 0xd6, 0x44, 0xc0, 0xd2, 0xcc, 0xcd, 0x8a, 0xc7, 0x58, 0x73,
            0xd8, 0xac, 0xaa, 0x4e, 0xe6, 0x5d, 0x3e, 0x7e, 0x29, 0xf1, 0x91, 0x6d, 0xf7, 0x38,
            0x57, 0xff, 0x73, 0x44, 0x87, 0x04, 0xf2, 0x39, 0x47, 0x37, 0xad, 0x52, 0xd6, 0x3b,
            0xbc, 0x5f, 0xdd, 0xfe, 0xe9, 0xdc, 0x43, 0x52, 0xb1, 0xb6, 0x4b, 0x3c, 0x6a, 0x27,
            0x80, 0x61, 0xab, 0x26, 0x26, 0x50, 0x3a, 0xee, 0x3d, 0x72, 0x52, 0x5f, 0x8b, 0xd4,
            0x73, 0x4d, 0x4f, 0xee, 0x3f, 0x7c, 0x32, 0x9a, 0x8e, 0x4b, 0xde, 0x6b, 0x39, 0x17,
            0x46, 0x1d, 0xe2, 0x39, 0xd8, 0xd6, 0xb3, 0xe6, 0x6d, 0x81, 0xf8, 0xef, 0xaf, 0x8e,
            0xc0, 0xb4, 0xeb, 0x47, 0x77, 0xee, 0x36, 0x3d, 0x2c, 0x57, 0xae, 0x38, 0xfe, 0x0c,
            0x7a, 0xb8, 0xbc, 0xaa, 0x07, 0xe2, 0xd9, 0x2e, 0x64, 0x2a, 0xa8, 0x3f, 0x68, 0x5e,
            0x9a, 0x3e, 0xdb, 0x80, 0x65, 0x05, 0x51, 0xee, 0xed, 0xca, 0x15, 0x85, 0xcf, 0xe7,
            0xd5, 0xe6, 0x26, 0x0b, 0x5c, 0xa2, 0x0d, 0x39, 0x82, 0x62, 0x34, 0x4f, 0xf3, 0xa2,
            0xb4, 0xb8, 0x6e, 0xcd, 0x5b, 0xe9, 0x65, 0xc2, 0xe9, 0x87, 0x4a, 0x1d, 0x87, 0xfd,
            0x48, 0x3d, 0x7a, 0xb1, 0xdf, 0xe3, 0x27, 0x8c, 0x3f, 0x7b, 0x03, 0xb7, 0xd7, 0xa6,
            0xa1, 0x9d, 0xff, 0x2f, 0x0a, 0xc5, 0x7e, 0xe3, 0x92, 0xc4, 0xc4, 0xcc, 0x03, 0xa0,
            0x6c, 0xa0, 0x1e, 0x6a, 0x6d, 0xe5, 0x9b, 0xed, 0xf2, 0x28, 0x87, 0x13, 0x60, 0xc9,
            0x6c, 0x44, 0xc5, 0xcf, 0x72, 0x33, 0x5b, 0x22, 0xf9, 0xac, 0x07, 0x29, 0x03, 0xff,
            0xfc, 0x52, 0x9e, 0x2b, 0xac, 0xb8, 0x70, 0x64, 0x82, 0x79, 0x44, 0x34, 0x45, 0xb1,
            0xd5, 0x47, 0x1b, 0x41, 0x0a, 0xec, 0xfa, 0x05, 0x43, 0x92, 0xe5, 0x4f, 0x86, 0xc9,
            0xf3, 0x21, 0x13, 0x60, 0x62, 0xf3, 0x38, 0xf1, 0x8f, 0xbb, 0x2c, 0x68, 0x89, 0x62,
            0x7a, 0xe6, 0x13, 0xcc, 0x5c, 0xad, 0xec, 0x5e, 0x90, 0x1c, 0x6b, 0xbd, 0xad, 0x95,
            0xf5, 0x32, 0x50, 0xaa, 0x73, 0x77, 0x43, 0x9d, 0xe4, 0xb7, 0x9b, 0xe2, 0x42, 0x2d,
            0xfe, 0x80, 0x27, 0xe6, 0x93, 0x00, 0xb4, 0x17, 0x4b, 0x62, 0xac, 0x86, 0x5b, 0x2e,
            0x45, 0xcf, 0xac, 0xfc, 0x33, 0x67, 0x43, 0x3d, 0x78, 0xdc, 0x61, 0x23, 0x24, 0x9b,
            0xda, 0x7a, 0x49, 0x7e, 0x09, 0xea, 0xcf, 0x9e, 0x48, 0xd2, 0xed, 0xf7, 0xc2, 0x1e,
            0x2b, 0xd1, 0x93, 0x50, 0x79, 0x31, 0x9f, 0xc3, 0x4d, 0xcc, 0x05, 0x4b, 0x72, 0xbb,
            0x31, 0x9e, 0xb0, 0x69, 0x1c, 0xc3, 0xe9, 0x68, 0xa8, 0xc6, 0xaa, 0xd6, 0xa4, 0x78,
            0xb6, 0x31, 0x9b, 0x3d, 0x8c, 0x42, 0xbe, 0x90, 0xaa, 0xef, 0xe3, 0xa0, 0xa4, 0x20,
            0xa8, 0x30, 0xd8, 0xad, 0xda, 0xe2, 0xe8, 0xf4, 0xcd, 0x7c, 0x7c, 0x7c, 0xf5, 0xd2,
            0x53, 0x8c, 0x4f, 0xc9, 0xd6, 0x01, 0x4b, 0xd1, 0x64, 0x5c, 0xed, 0x79, 0x70, 0xa6,
            0xfb, 0xb3, 0xc7, 0x75, 0x83, 0xe5, 0x99, 0x0c, 0x14, 0xc3, 0x72, 0xef, 0x7a, 0x72,
            0x7f, 0x20, 0xb5, 0xe8, 0x40, 0xf1, 0xdf, 0x6e, 0x41, 0xf4, 0x0b, 0x23, 0xdf, 0x86,
            0x5d, 0x63, 0x5a, 0x12, 0x45, 0x65, 0xab, 0x02, 0x03, 0x01, 0x00, 0x01, 0xa3, 0x7e,
            0x30, 0x7c, 0x30, 0x0e, 0x06, 0x03, 0x55, 0x1d, 0x0f, 0x01, 0x01, 0xff, 0x04, 0x04,
            0x03, 0x02, 0x01, 0x06, 0x30, 0x1d, 0x06, 0x03, 0x55, 0x1d, 0x0e, 0x04, 0x16, 0x04,
            0x14, 0x85, 0xac, 0x1a, 0xd1, 0x43, 0xf7, 0xc8, 0xac, 0x55, 0xd4, 0xc5, 0x1d, 0x41,
            0x48, 0xab, 0xd5, 0x78, 0x4a, 0xd4, 0x53, 0x30, 0x0f, 0x06, 0x03, 0x55, 0x1d, 0x13,
            0x01, 0x01, 0xff, 0x04, 0x05, 0x30, 0x03, 0x01, 0x01, 0xff, 0x30, 0x3a, 0x06, 0x03,
            0x55, 0x1d, 0x1f, 0x04, 0x33, 0x30, 0x31, 0x30, 0x2f, 0xa0, 0x2d, 0xa0, 0x2b, 0x86,
            0x29, 0x68, 0x74, 0x74, 0x70, 0x73, 0x3a, 0x2f, 0x2f, 0x6b, 0x64, 0x73, 0x69, 0x6e,
            0x74, 0x66, 0x2e, 0x61, 0x6d, 0x64, 0x2e, 0x63, 0x6f, 0x6d, 0x2f, 0x76, 0x63, 0x65,
            0x6b, 0x2f, 0x76, 0x31, 0x2f, 0x4d, 0x69, 0x6c, 0x61, 0x6e, 0x2f, 0x63, 0x72, 0x6c,
            0x30, 0x46, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0a, 0x30,
            0x39, 0xa0, 0x0f, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04,
            0x02, 0x02, 0x05, 0x00, 0xa1, 0x1c, 0x30, 0x1a, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86,
            0xf7, 0x0d, 0x01, 0x01, 0x08, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65,
            0x03, 0x04, 0x02, 0x02, 0x05, 0x00, 0xa2, 0x03, 0x02, 0x01, 0x30, 0xa3, 0x03, 0x02,
            0x01, 0x01, 0x03, 0x82, 0x02, 0x01, 0x00, 0xba, 0x9b, 0x49, 0x03, 0xa7, 0xac, 0xef,
            0xe0, 0xe8, 0xdf, 0x83, 0x2f, 0xb3, 0x95, 0xe7, 0xa1, 0xb3, 0x1e, 0xa8, 0x97, 0x4a,
            0x1c, 0x81, 0x57, 0xa5, 0x11, 0x3a, 0x1b, 0xa7, 0x1f, 0x84, 0xb8, 0x2b, 0x2a, 0x54,
            0x54, 0x4f, 0x2b, 0x58, 0xd9, 0xd6, 0xca, 0x7f, 0x97, 0x27, 0x7d, 0xfb, 0x47, 0xd0,
            0xd2, 0xbe, 0xba, 0x9f, 0xb9, 0x1a, 0x81, 0x19, 0x38, 0x09, 0xad, 0xfd, 0x83, 0xae,
            0x96, 0x19, 0x32, 0x4c, 0x78, 0x97, 0x6a, 0x62, 0xb8, 0xb0, 0x49, 0x38, 0xe3, 0x0c,
            0x22, 0x95, 0x3d, 0x27, 0xac, 0x59, 0x76, 0x0f, 0x54, 0x0c, 0x83, 0x86, 0x63, 0xf9,
            0x9f, 0x6b, 0xfe, 0x05, 0x88, 0xa9, 0x65, 0x68, 0x69, 0xbe, 0xaa, 0x5a, 0x88, 0xef,
            0x84, 0x18, 0xae, 0x48, 0x04, 0xff, 0xb9, 0xef, 0xc4, 0x1e, 0x5b, 0xfb, 0x12, 0xa2,
            0x4a, 0xca, 0x74, 0x76, 0x8b, 0x03, 0x11, 0xb6, 0x2e, 0x16, 0x71, 0x8f, 0xd6, 0x85,
            0xef, 0x77, 0xea, 0x0b, 0xb3, 0x80, 0x25, 0x9e, 0x5a, 0x3e, 0x89, 0xf0, 0xe1, 0x11,
            0x36, 0xf7, 0xd1, 0x55, 0x6a, 0xb8, 0x75, 0x4f, 0x1d, 0x9e, 0x4f, 0x7c, 0x12, 0x82,
            0x40, 0xe0, 0xba, 0xd0, 0x93, 0x07, 0x56, 0x2a, 0xcd, 0x3e, 0x43, 0xbb, 0x0b, 0xc0,
            0x7b, 0xe7, 0x28, 0xd8, 0x22, 0x15, 0x23, 0x33, 0x03, 0x6a, 0x66, 0x2e, 0x48, 0x58,
            0xcf, 0x37, 0x40, 0x42, 0x82, 0x88, 0xe5, 0xed, 0x5f, 0x9b, 0x4e, 0x8b, 0xbb, 0x74,
            0xcb, 0x2a, 0x22, 0xef, 0xd3, 0x5b, 0xfa, 0xcf, 0x09, 0x7f, 0x7f, 0x11, 0x47, 0x29,
            0x28, 0x62, 0xaa, 0x3d, 0x0d, 0xcf, 0xf8, 0xdf, 0x6b, 0xd6, 0x18, 0xc4, 0x15, 0x8d,
            0x69, 0x94, 0x18, 0x3d, 0xde, 0xde, 0x77, 0x38, 0xea, 0x38, 0xf4, 0x63, 0x48, 0xf9,
            0x5d, 0x73, 0xbd, 0x73, 0xcb, 0x23, 0xac, 0x48, 0x15, 0x5b, 0x21, 0xfa, 0x6b, 0x68,
            0xd9, 0x1b, 0x60, 0x11, 0x7f, 0xde, 0xa6, 0x63, 0x0a, 0x4c, 0xd3, 0x7a, 0xa6, 0xc5,
            0xbc, 0xf2, 0xa8, 0x3b, 0x73, 0x58, 0x53, 0x5a, 0xd3, 0x7a, 0x31, 0xb4, 0x6e, 0x43,
            0x4b, 0xe6, 0xf8, 0xef, 0xbf, 0xda, 0xd2, 0x81, 0x17, 0x68, 0x7c, 0x4c, 0x76, 0xfd,
            0xe0, 0xeb, 0xef, 0x1c, 0x7a, 0x05, 0x0e, 0x96, 0xc2, 0x10, 0xb9, 0x6a, 0x1e, 0x72,
            0x18, 0x87, 0x1c, 0xb4, 0x60, 0xa5, 0xc6, 0xc9, 0xa5, 0xb5, 0x36, 0x37, 0xd4, 0x2f,
            0x1a, 0xeb, 0x9b, 0x15, 0x56, 0xe3, 0x07, 0x27, 0xe4, 0x4f, 0x06, 0x75, 0xd9, 0xaf,
            0x35, 0xae, 0xb2, 0x62, 0x6f, 0x2c, 0x70, 0x96, 0xa0, 0x12, 0x2d, 0x77, 0x9a, 0x11,
            0xae, 0xe0, 0x9a, 0xa1, 0xdd, 0x05, 0x37, 0xb1, 0xff, 0x22, 0x51, 0x25, 0x2b, 0xd3,
            0xdc, 0x50, 0x0f, 0x01, 0xed, 0x39, 0x05, 0x15, 0x22, 0xac, 0x78, 0x99, 0xa0, 0x59,
            0x3c, 0x1b, 0x52, 0x31, 0xff, 0xaa, 0x50, 0x3b, 0x63, 0x5d, 0x24, 0xaa, 0xf2, 0x57,
            0xd6, 0x71, 0xdf, 0x1b, 0x2e, 0xbf, 0x66, 0x76, 0xc5, 0x27, 0x25, 0x92, 0x74, 0xfa,
            0xdb, 0x8f, 0x30, 0xa9, 0x81, 0x9d, 0x21, 0xfc, 0xeb, 0x49, 0x65, 0x2a, 0x4f, 0x95,
            0xa5, 0x54, 0x2c, 0x82, 0xa6, 0xf3, 0x0c, 0x8b, 0xce, 0x2e, 0xf0, 0xfa, 0x5b, 0x55,
            0x26, 0xab, 0x6e, 0x5b, 0xa3, 0x10, 0x98, 0x27, 0xe4, 0xee, 0x06, 0x86, 0xb8, 0xb3,
            0xe1, 0xc7, 0x09, 0x58, 0x80, 0xbe, 0x04, 0xfd, 0x91, 0xff, 0xeb, 0x06, 0xad, 0x5d,
            0xfa, 0x2b, 0xe3, 0xea, 0xc9, 0x24, 0x1f, 0x1b, 0xb3, 0x73, 0x16, 0xe4, 0xd7, 0x1b,
            0xfa, 0x64, 0x6c, 0x6b, 0xb5, 0xe2, 0x71, 0x54, 0x7e, 0xca, 0x95, 0x7e, 0xd8, 0x45,
            0xd6, 0x7a, 0x78, 0x04, 0x4a, 0xc0, 0xb7, 0xb8, 0x00, 0x56, 0x44, 0x03, 0x0a, 0x0a,
            0x09,
        ];

        assert_eq!(Certificate::identify_format(dummy_der), CertFormat::Der)
    }
}
