//! Implementation of [AWS V4 Signing][link]
//!
//! [link]: https://docs.aws.amazon.com/AmazonS3/latest/API/sig-v4-authenticating-requests.html

use std::collections::HashMap;
use std::str;

use hmac::{Hmac, Mac};
use hyper::http::HeaderMap;
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use sha2::{Digest, Sha256};
use time::{macros::format_description, OffsetDateTime};
use url::Url;

use crate::error::S3Error;
use crate::region::Region;
use crate::LONG_DATETIME;

use std::fmt::Write as _;

const SHORT_DATE: &[time::format_description::FormatItem<'static>] =
    format_description!("[year][month][day]");

pub type HmacSha256 = Hmac<Sha256>;

// https://perishablepress.com/stop-using-unsafe-characters-in-urls/
pub const FRAGMENT: &AsciiSet = &CONTROLS
    // URL_RESERVED
    .add(b':')
    .add(b'?')
    .add(b'#')
    .add(b'[')
    .add(b']')
    .add(b'@')
    .add(b'!')
    .add(b'$')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b'+')
    .add(b',')
    .add(b';')
    .add(b'=')
    // URL_UNSAFE
    .add(b'"')
    .add(b' ')
    .add(b'<')
    .add(b'>')
    .add(b'%')
    .add(b'{')
    .add(b'}')
    .add(b'|')
    .add(b'\\')
    .add(b'^')
    .add(b'`');

pub const FRAGMENT_SLASH: &AsciiSet = &FRAGMENT.add(b'/');

/// Encode a URI following the specific requirements of the AWS service.
pub fn uri_encode(string: &str, encode_slash: bool) -> String {
    if encode_slash {
        utf8_percent_encode(string, FRAGMENT_SLASH).to_string()
    } else {
        utf8_percent_encode(string, FRAGMENT).to_string()
    }
}

/// Generate a canonical URI string from the given URL.
pub fn canonical_uri_string(uri: &Url) -> String {
    // decode `Url`'s percent-encoding and then reencode it
    // according to AWS's rules
    let decoded = percent_encoding::percent_decode_str(uri.path()).decode_utf8_lossy();
    uri_encode(&decoded, false)
}

/// Generate a canonical query string from the query pairs in the given URL.
pub fn canonical_query_string(uri: &Url) -> String {
    let mut keyvalues: Vec<(String, String)> = uri
        .query_pairs()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    keyvalues.sort();
    let keyvalues: Vec<String> = keyvalues
        .iter()
        .map(|(k, v)| {
            format!(
                "{}={}",
                utf8_percent_encode(k, FRAGMENT_SLASH),
                utf8_percent_encode(v, FRAGMENT_SLASH)
            )
        })
        .collect();
    keyvalues.join("&")
}

/// Generate a canonical header string from the provided headers.
pub fn canonical_header_string(headers: &HeaderMap) -> Result<String, S3Error> {
    let mut keyvalues = vec![];
    for (key, value) in headers.iter() {
        keyvalues.push(format!(
            "{}:{}",
            key.as_str().to_lowercase(),
            value.to_str()?.trim()
        ))
    }
    keyvalues.sort();
    Ok(keyvalues.join("\n"))
}

/// Generate a signed header string from the provided headers.
pub fn signed_header_string(headers: &HeaderMap) -> String {
    let mut keys = headers
        .keys()
        .map(|key| key.as_str().to_lowercase())
        .collect::<Vec<String>>();
    keys.sort();
    keys.join(";")
}

/// Generate a canonical request.
pub fn canonical_request(
    method: &str,
    url: &Url,
    headers: &HeaderMap,
    sha256: &str,
) -> Result<String, S3Error> {
    Ok(format!(
        "{method}\n{uri}\n{query_string}\n{headers}\n\n{signed}\n{sha256}",
        method = method,
        uri = canonical_uri_string(url),
        query_string = canonical_query_string(url),
        headers = canonical_header_string(headers)?,
        signed = signed_header_string(headers),
        sha256 = sha256
    ))
}

/// Generate an AWS scope string.
pub fn scope_string(datetime: &OffsetDateTime, region: &Region) -> Result<String, S3Error> {
    Ok(format!(
        "{date}/{region}/s3/aws4_request",
        date = datetime.format(SHORT_DATE)?,
        region = region
    ))
}

/// Generate the "string to sign" - the value to which the HMAC signing is
/// applied to sign requests.
pub fn string_to_sign(
    datetime: &OffsetDateTime,
    region: &Region,
    canonical_req: &str,
) -> Result<String, S3Error> {
    let mut hasher = Sha256::default();
    hasher.update(canonical_req.as_bytes());
    let string_to = format!(
        "AWS4-HMAC-SHA256\n{timestamp}\n{scope}\n{hash}",
        timestamp = datetime.format(LONG_DATETIME)?,
        scope = scope_string(datetime, region)?,
        hash = hex::encode(hasher.finalize().as_slice())
    );
    Ok(string_to)
}

/// Generate the AWS signing key, derived from the secret key, date, region,
/// and service name.
pub fn signing_key(
    datetime: &OffsetDateTime,
    secret_key: &str,
    region: &Region,
    service: &str,
) -> Result<Vec<u8>, S3Error> {
    let secret = format!("AWS4{}", secret_key);
    let mut date_hmac = HmacSha256::new_from_slice(secret.as_bytes())?;
    date_hmac.update(datetime.format(SHORT_DATE)?.as_bytes());
    let mut region_hmac = HmacSha256::new_from_slice(&date_hmac.finalize().into_bytes())?;
    region_hmac.update(region.to_string().as_bytes());
    let mut service_hmac = HmacSha256::new_from_slice(&region_hmac.finalize().into_bytes())?;
    service_hmac.update(service.as_bytes());
    let mut signing_hmac = HmacSha256::new_from_slice(&service_hmac.finalize().into_bytes())?;
    signing_hmac.update(b"aws4_request");
    Ok(signing_hmac.finalize().into_bytes().to_vec())
}

/// Generate the AWS authorization header.
pub fn authorization_header(
    access_key: &str,
    datetime: &OffsetDateTime,
    region: &Region,
    signed_headers: &str,
    signature: &str,
) -> Result<String, S3Error> {
    Ok(format!(
        "AWS4-HMAC-SHA256 Credential={access_key}/{scope},\
            SignedHeaders={signed_headers},Signature={signature}",
        access_key = access_key,
        scope = scope_string(datetime, region)?,
        signed_headers = signed_headers,
        signature = signature
    ))
}

pub fn authorization_query_params_no_sig(
    access_key: &str,
    datetime: &OffsetDateTime,
    region: &Region,
    expires: u32,
    custom_headers: Option<&HeaderMap>,
    token: Option<&String>,
) -> Result<String, S3Error> {
    let credentials = format!("{}/{}", access_key, scope_string(datetime, region)?);
    let credentials = utf8_percent_encode(&credentials, FRAGMENT_SLASH);

    let mut signed_headers = vec!["host".to_string()];

    if let Some(custom_headers) = &custom_headers {
        for k in custom_headers.keys() {
            signed_headers.push(k.to_string())
        }
    }

    signed_headers.sort();
    let signed_headers = signed_headers.join(";");
    let signed_headers = utf8_percent_encode(&signed_headers, FRAGMENT_SLASH);

    let mut query_params = format!(
        "?X-Amz-Algorithm=AWS4-HMAC-SHA256\
            &X-Amz-Credential={credentials}\
            &X-Amz-Date={long_date}\
            &X-Amz-Expires={expires}\
            &X-Amz-SignedHeaders={signed_headers}",
        credentials = credentials,
        long_date = datetime.format(LONG_DATETIME)?,
        expires = expires,
        signed_headers = signed_headers,
    );

    if let Some(token) = token {
        write!(
            query_params,
            "&X-Amz-Security-Token={}",
            utf8_percent_encode(token, FRAGMENT_SLASH)
        )
        .expect("Could not write token");
    }

    Ok(query_params)
}

pub fn flatten_queries(queries: Option<&HashMap<String, String>>) -> Result<String, S3Error> {
    match queries {
        None => Ok(String::new()),
        Some(queries) => {
            let mut query_str = String::new();
            for (k, v) in queries {
                write!(
                    query_str,
                    "&{}={}",
                    utf8_percent_encode(k, FRAGMENT_SLASH),
                    utf8_percent_encode(v, FRAGMENT_SLASH),
                )?;
            }
            Ok(query_str)
        }
    }
}
