use serde::{Serializer, Deserializer};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

pub fn serialize<S: Serializer, const N: usize>(binary: &[u8; N], serializer: S)-> Result<S::Ok, S::Error> {
    let mut tmp = String::with_capacity(base64::encoded_len(N, false).unwrap_or(0));
    encode(binary, &mut tmp);
    serializer.serialize_str(&tmp)
}

pub fn encode<const N: usize>(binary: &[u8; N], out: &mut String) {
    URL_SAFE_NO_PAD.encode_string(binary, out)
}

pub fn decode<const N: usize>(base64: &str) -> Option<[u8; N]> {
    let mut buf = [0u8; N];
    if base64.len() != base64::encoded_len(N, false)? {
        return None;
    }
    // this is a safe function. the checked version checks against a pessimistic estimate,
    // which can't roundtrip any specific length!
    URL_SAFE_NO_PAD.decode_slice_unchecked(base64, &mut buf).ok()?;
    Some(buf)
}

pub fn deserialize<'de, D: Deserializer<'de>, const N: usize>(deserializer: D) -> Result<[u8; N], D::Error> {
    struct TokenVisitor<const N: usize>;
    use serde::de::{Error, Visitor, Unexpected};
    use std::fmt;

    impl<'de, const N: usize> Visitor<'de> for TokenVisitor<N> {
        type Value = [u8; N];

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{} base64 chars for {} bytes", base64::encoded_len(N, false).unwrap_or(0), N)
        }

        fn visit_str<E: Error>(self, s: &str) -> Result<Self::Value, E> {
            decode(s).ok_or(Error::invalid_value(Unexpected::Str(s), &self))
        }

        fn visit_borrowed_str<E: Error>(self, s: &'de str) -> Result<Self::Value, E> {
            decode(s).ok_or(Error::invalid_value(Unexpected::Str(s), &self))
        }
    }

    deserializer.deserialize_str(TokenVisitor::<N>)
}

#[test]
fn bin_base64() {
    #[track_caller]
    fn test_case<const N: usize>() {
        let mut out = String::new();
        let input = [123; N];
        encode(&input, &mut out);
        assert_eq!(out.len(), base64::encoded_len(N, false).unwrap(), "actual");
        assert_eq!(input, decode(&out).expect("decode"), "decoded");
        out.push('x');
        assert!(decode::<N>(&out).is_none());
        out.truncate(out.len()-2);
        assert!(decode::<N>(&out).is_none());
    }
    test_case::<40>();
    test_case::<32>();
    test_case::<21>();
    test_case::<20>();
    test_case::<16>();
    test_case::<8>();
}
