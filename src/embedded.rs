include!(concat!(env!("OUT_DIR"), "/embedded_files.rs"));

pub(crate) fn content(path: &str) -> Option<&'static str> {
    CONTENT_FILES
        .iter()
        .find_map(|(candidate, source)| (*candidate == path).then_some(*source))
}

pub(crate) fn content_files() -> impl Iterator<Item = (&'static str, &'static str)> {
    CONTENT_FILES.iter().copied()
}

pub(crate) fn asset_url(path: &str) -> Option<&'static str> {
    STATIC_FILES
        .iter()
        .find_map(|(logical, _, public_url, _)| (*logical == path).then_some(*public_url))
}

pub(crate) fn static_file(path: &str) -> Option<&'static [u8]> {
    STATIC_FILES
        .iter()
        .find_map(|(_, fingerprinted, _, source)| (*fingerprinted == path).then_some(*source))
}
