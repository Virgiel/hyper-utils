use hyper::{
    body::{Bytes, HttpBody},
    header::{self, HeaderName},
    Body, HeaderMap, Response, StatusCode, Uri,
};
use libdeflater::{CompressionLvl, Compressor};

pub mod error;

/// Get body content as bytes if its length is under a limit
pub async fn body_bytes_max(body: Body, max: u64) -> Result<Option<Bytes>, hyper::Error> {
    Ok(if body.size_hint().upper().unwrap_or(u64::MAX) > max {
        None
    } else {
        Some(hyper::body::to_bytes(body).await?)
    })
}

/// Get str header value
pub fn str_header<'a>(map: &'a HeaderMap, name: &'static str) -> Option<&'a str> {
    map.get(&HeaderName::from_static(name))
        .and_then(|h| h.to_str().ok())
}

/// Get first str header value
pub fn str_header_first<'a>(map: &'a HeaderMap, name: &'static str) -> Option<&'a str> {
    str_header(map, name).and_then(|h| h.split(',').next().map(|it| it.trim()))
}

/// Resolve client ip from headers
pub fn client_ip(map: &HeaderMap) -> Option<&str> {
    // fly-client-ip first as client can spoof x-forwarded-for
    str_header(map, "fly-client-ip").or_else(|| str_header_first(map, "x-forwarded-for"))
}

/// Parse request scheme
pub fn parse_scheme<'a>(map: &'a HeaderMap, uri: &'a Uri) -> &'a str {
    str_header_first(map, "x-forwarded-proto")
        .or_else(|| uri.scheme_str())
        .unwrap_or("http")
}

/// Parse request host
pub fn parse_host<'a>(map: &'a HeaderMap, uri: &'a Uri) -> &'a str {
    str_header_first(map, "x-forwarded-host")
        .or_else(|| str_header(map, "host"))
        .or_else(|| uri.authority().map(|a| a.host()))
        .unwrap_or("localhost")
}

/// Resolve client base url
pub fn parse_base_url(map: &HeaderMap, uri: &Uri) -> String {
    format!("{}://{}", parse_scheme(map, uri), parse_host(map, uri))
}

/// Create a redirect response if the base scheme is http and we are not in localhost
pub fn redirect_https(map: &HeaderMap, uri: &Uri) -> Option<Response<Body>> {
    let scheme = parse_scheme(map, uri);
    let host = parse_host(map, uri);

    (scheme == "http" && !host.starts_with("127.0.0.1") && !host.starts_with("localhost")).then(
        || {
            Response::builder()
                .status(StatusCode::PERMANENT_REDIRECT)
                .header(header::LOCATION, &format!("https://{}{}", host, uri))
                .body(Body::empty())
                .unwrap()
        },
    )
}

/// Fast in memory gzip compression
pub fn compress(in_data: &[u8]) -> Vec<u8> {
    let mut compressor = Compressor::new(CompressionLvl::default());
    let max_size = compressor.gzip_compress_bound(in_data.len());
    let mut gzip = vec![0; max_size];
    let gzip_size = compressor.gzip_compress(in_data, &mut gzip).unwrap();
    gzip.resize(gzip_size, 0);
    gzip
}
