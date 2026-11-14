// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/14 12:09:01.791321
//+ Revised: 	2026/11/14 21:46:04.297237
// -----------------------------------------------------------------------------

//! Proving a release was published by Orin's maintainer, not only by whoever controls a forge.
//!
//! A `.b3` checksum comes from the same release as the binary, so it only catches a damaged
//! download. Each asset also has a `.sig`: an Ed25519 signature, made with a key that never
//! leaves the maintainer's machine, over a statement naming the version, the asset and its
//! BLAKE3 hash. The version and name are signed too, so a genuine older binary, or another
//! system's, can't be offered in place of the one the release names.

use super::release::UpdateError;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use semver::Version;

/// The public halves of the release keys, in hex; a release signed by either is trusted.
/// Releases are signed with the first. The second is a backup whose secret half is kept apart
/// and never used day to day: if the first is lost or stolen, a release signed with the backup
/// still reaches every user, and can carry a new pair of keys that leaves the old one out.
pub const RELEASE_KEYS: [&str; 2] = [
    "43bc53fac2b823845a381106efe8ecf8ef37746d48b4ef648111e29e3950fbd8",
    "e8c96b1eb91d7f466bebbafa8c3e7eece4dd2d786a9c9445ab8a9e5fceae447a",
];

/// The keys a download may be signed with. Debug builds honour `ORIN_UPDATE_KEY`, so tests can
/// sign their own releases; a release build only trusts the keys it was compiled with.
pub fn release_keys() -> Vec<String> {
    if cfg!(debug_assertions)
        && let Ok(key) = std::env::var("ORIN_UPDATE_KEY")
    {
        return vec![key];
    }
    RELEASE_KEYS.iter().map(|key| (*key).to_owned()).collect()
}

/// Whether `signature` is over `statement` by any one of `keys`. A signature none of them
/// made is refused as such; text that isn't a signature at all is named as that.
pub fn verify_any<K: AsRef<str>>(
    keys: &[K],
    statement: &str,
    signature: &str,
) -> Result<(), UpdateError> {
    let mut refusal = None;
    for key in keys {
        match verify(key.as_ref(), statement, signature) {
            Ok(()) => return Ok(()),
            Err(error @ UpdateError::Parse("the signature")) => return Err(error),
            Err(UpdateError::Signature) => refusal = Some(UpdateError::Signature),
            // An unreadable key is reported only when no readable key had its say.
            Err(error) => {
                refusal.get_or_insert(error);
            }
        }
    }
    Err(refusal.unwrap_or(UpdateError::Signature))
}

/// What a signature covers: exactly which file is which version's build for which system.
pub fn statement(version: &Version, asset: &str, hash: &str) -> String {
    format!(
        "orin-release-v1\n{version}\n{asset}\n{}\n",
        hash.to_ascii_lowercase()
    )
}

/// Whether `signature` (the text of a `.sig` file: 128 hex digits) is `key`'s over `statement`.
pub fn verify(key: &str, statement: &str, signature: &str) -> Result<(), UpdateError> {
    let key = decode::<32>(key)
        .and_then(|bytes| VerifyingKey::from_bytes(&bytes).ok())
        .ok_or(UpdateError::Parse("the release key"))?;
    let signature = decode::<64>(signature.lines().next().unwrap_or_default())
        .map(|bytes| Signature::from_bytes(&bytes))
        .ok_or(UpdateError::Parse("the signature"))?;
    key.verify_strict(statement.as_bytes(), &signature)
        .map_err(|_| UpdateError::Signature)
}

/// A new secret key, in hex, from the system's random source.
pub fn generate() -> Result<String, UpdateError> {
    let mut seed = [0_u8; 32];
    getrandom::fill(&mut seed)
        .map_err(|error| UpdateError::Io(format!("the system gave no random bytes ({error})")))?;
    Ok(encode(&seed))
}

/// The public key, in hex, that verifies what `secret` signs.
pub fn public_key(secret: &str) -> Result<String, UpdateError> {
    Ok(encode(signing_key(secret)?.verifying_key().as_bytes()))
}

/// The text of a `.sig` file: `secret`'s signature over `statement`, in hex, and a line break.
pub fn sign(secret: &str, statement: &str) -> Result<String, UpdateError> {
    let signature = signing_key(secret)?.sign(statement.as_bytes());
    Ok(format!("{}\n", encode(&signature.to_bytes())))
}

fn signing_key(secret: &str) -> Result<SigningKey, UpdateError> {
    decode::<32>(secret)
        .map(|seed| SigningKey::from_bytes(&seed))
        .ok_or(UpdateError::Parse("the secret key"))
}

fn encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// `text` (surrounding white space ignored) as exactly `N` bytes of hex.
fn decode<const N: usize>(text: &str) -> Option<[u8; N]> {
    let text = text.trim();
    if text.len() != 2 * N || !text.is_ascii() {
        return None;
    }
    let mut bytes = [0_u8; N];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * index..2 * index + 2], 16).ok()?;
    }
    Some(bytes)
}

/// A fixed key pair for tests: `(secret, public)`.
#[cfg(test)]
pub(crate) fn test_keys() -> (String, String) {
    let secret = encode(&[7_u8; 32]);
    let public = public_key(&secret).unwrap();
    (secret, public)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262";

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn unit_a_signature_verifies_only_its_own_statement_and_key() {
        let (secret, public) = test_keys();
        let signed = statement(&v("0.3.0"), "orin-0.3.0-linux-x86_64", HASH);
        let signature = sign(&secret, &signed).unwrap();
        assert_eq!(verify(&public, &signed, &signature), Ok(()));
        // An older genuine build, another system's build, or other bytes: all refused.
        for other in [
            statement(&v("0.2.0"), "orin-0.3.0-linux-x86_64", HASH),
            statement(&v("0.3.0"), "orin-0.3.0-windows-x86_64.exe", HASH),
            statement(
                &v("0.3.0"),
                "orin-0.3.0-linux-x86_64",
                &HASH.replace('a', "b"),
            ),
        ] {
            assert_eq!(
                verify(&public, &other, &signature),
                Err(UpdateError::Signature)
            );
        }
        let stranger = public_key(&encode(&[8_u8; 32])).unwrap();
        assert_eq!(
            verify(&stranger, &signed, &signature),
            Err(UpdateError::Signature)
        );
    }

    #[test]
    fn unit_the_hash_is_signed_whatever_its_case() {
        let (secret, public) = test_keys();
        let lower = statement(&v("0.3.0"), "orin", HASH);
        let upper = statement(&v("0.3.0"), "orin", &HASH.to_uppercase());
        assert_eq!(lower, upper);
        let signature = sign(&secret, &lower).unwrap().to_uppercase();
        assert_eq!(verify(&public, &upper, &signature), Ok(()));
    }

    #[test]
    fn unit_malformed_keys_and_signatures_are_named() {
        let (secret, public) = test_keys();
        let signed = statement(&v("0.3.0"), "orin", HASH);
        let signature = sign(&secret, &signed).unwrap();
        for bad in [
            "",
            "not hex",
            &signature[..100],
            &format!("{}00", signature.trim()),
            "é".repeat(64).as_str(),
        ] {
            assert_eq!(
                verify(&public, &signed, bad),
                Err(UpdateError::Parse("the signature")),
                "{bad:?}"
            );
        }
        assert_eq!(
            verify("abc", &signed, &signature),
            Err(UpdateError::Parse("the release key"))
        );
        assert_eq!(
            sign("abc", &signed),
            Err(UpdateError::Parse("the secret key"))
        );
        assert_eq!(public_key("zz"), Err(UpdateError::Parse("the secret key")));
    }

    proptest::proptest! {
        /// Whatever is signed verifies, and changing any one part of it does not.
        #[test]
        fn property_only_the_signed_statement_verifies(
            seed in proptest::array::uniform32(proptest::num::u8::ANY),
            (major, minor, patch) in (0_u64..50, 0_u64..50, 0_u64..50),
            asset in "[a-z0-9._-]{1,40}",
            hash in "[0-9a-f]{64}",
            other_hash in "[0-9a-f]{64}",
        ) {
            let secret = encode(&seed);
            let public = public_key(&secret).unwrap();
            let version = Version::new(major, minor, patch);
            let signed = statement(&version, &asset, &hash);
            let signature = sign(&secret, &signed).unwrap();
            proptest::prop_assert_eq!(verify(&public, &signed, &signature), Ok(()));
            let newer = Version::new(major, minor, patch + 1);
            let renamed = format!("{asset}x");
            for changed in [
                statement(&newer, &asset, &hash),
                statement(&version, &renamed, &hash),
            ] {
                proptest::prop_assert_eq!(
                    verify(&public, &changed, &signature),
                    Err(UpdateError::Signature)
                );
            }
            if other_hash != hash {
                proptest::prop_assert_eq!(
                    verify(&public, &statement(&version, &asset, &other_hash), &signature),
                    Err(UpdateError::Signature)
                );
            }
        }
    }

    #[test]
    fn unit_generated_keys_differ_and_sign() {
        let first = generate().unwrap();
        let second = generate().unwrap();
        assert_ne!(first, second);
        let signed = statement(&v("1.0.0"), "orin", HASH);
        let signature = sign(&first, &signed).unwrap();
        assert_eq!(
            verify(&public_key(&first).unwrap(), &signed, &signature),
            Ok(())
        );
    }

    #[test]
    fn unit_the_embedded_release_keys_are_valid_and_distinct() {
        let signed = statement(&v("1.0.0"), "orin", HASH);
        let (secret, _) = test_keys();
        let stranger = sign(&secret, &signed).unwrap();
        // A stranger's signature fails as a signature, not as an unreadable key.
        for key in RELEASE_KEYS {
            assert_eq!(verify(key, &signed, &stranger), Err(UpdateError::Signature));
        }
        assert_eq!(
            verify_any(&RELEASE_KEYS, &signed, &stranger),
            Err(UpdateError::Signature)
        );
        assert_ne!(
            RELEASE_KEYS[0], RELEASE_KEYS[1],
            "the backup is its own key"
        );
    }

    #[test]
    fn unit_either_trusted_key_is_enough_and_no_key_trusts_nothing() {
        let signed = statement(&v("1.0.0"), "orin", HASH);
        let (primary, primary_public) = test_keys();
        let backup = encode(&[9_u8; 32]);
        let backup_public = public_key(&backup).unwrap();
        let trusted = [primary_public.clone(), backup_public.clone()];
        // The everyday key, and the backup that takes over when it is lost.
        for secret in [&primary, &backup] {
            let signature = sign(secret, &signed).unwrap();
            assert_eq!(verify_any(&trusted, &signed, &signature), Ok(()));
        }
        // Once a build leaves a key out, that key's signatures stop counting.
        let rotated = [backup_public];
        let old = sign(&primary, &signed).unwrap();
        assert_eq!(
            verify_any(&rotated, &signed, &old),
            Err(UpdateError::Signature)
        );
        let none: [&str; 0] = [];
        assert_eq!(
            verify_any(&none, &signed, &old),
            Err(UpdateError::Signature)
        );
        // Text that isn't a signature is named as that, whichever keys are trusted.
        assert_eq!(
            verify_any(&trusted, &signed, "not a signature"),
            Err(UpdateError::Parse("the signature"))
        );
        // One unreadable key among good ones doesn't hide a valid signature or a refusal.
        let mixed = ["abc".to_owned(), primary_public];
        assert_eq!(verify_any(&mixed, &signed, &old), Ok(()));
        let stranger = sign(&encode(&[3_u8; 32]), &signed).unwrap();
        assert_eq!(
            verify_any(&mixed, &signed, &stranger),
            Err(UpdateError::Signature)
        );
        assert_eq!(
            verify_any(&["abc"], &signed, &old),
            Err(UpdateError::Parse("the release key"))
        );
    }
}
