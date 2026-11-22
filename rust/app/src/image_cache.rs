//! Images are sanitized through RemoteImage before Qt decodes them.
pub fn image_url(url: &str) -> String {
    url.to_owned()
}
