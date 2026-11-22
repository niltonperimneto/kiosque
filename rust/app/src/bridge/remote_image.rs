//! `RemoteImage`: turns an image URL into a sanitized local PNG.
//!
//! QML sets `url` and `kind` (`icon:<px>`, `screenshot`, `avatar`) and binds
//! an `Image.source` to the read-only `source`, which is either empty or a
//! `file://` URL of a PNG written by `kiosque_core::sanitized_image`. A cache hit
//! is applied synchronously; otherwise the loader runs and `source` follows.

use qtbridge::qobject;

pub use object::RemoteImage;

#[qobject(Base = QParserStatus, NoQmlElement)]
mod object {
    use kiosque_core::sanitized_image::ImageKind;
    use qtbridge::{QParserStatus, QmlObject};

    #[derive(Default)]
    pub struct RemoteImage {
        pub(super) url: String,
        pub(super) kind: String,
        pub(super) source: String,
        pub(super) failed: bool,
        pub(super) complete: bool,
        pub(super) generation: u64,
    }

    impl QParserStatus for RemoteImage {
        fn class_begin(&mut self) {}
        fn component_complete(&mut self) {
            self.complete = true;
            self.update();
        }
    }

    impl RemoteImage {
        qproperty!("url", Read = get_url, Write = set_url, Notify = url_changed);
        qproperty!(
            "kind",
            Read = get_kind,
            Write = set_kind,
            Notify = kind_changed
        );
        qproperty!("source", Read = get_source, Notify = source_changed);
        qproperty!("failed", Read = get_failed, Notify = failed_changed);

        fn get_url(&self) -> String {
            self.url.clone()
        }
        fn set_url(&mut self, url: String) {
            if url != self.url {
                self.url = url;
                self.update();
                self.url_changed();
            }
        }
        fn get_kind(&self) -> String {
            self.kind.clone()
        }
        fn set_kind(&mut self, kind: String) {
            if kind != self.kind {
                self.kind = kind;
                self.update();
                self.kind_changed();
            }
        }
        fn get_source(&self) -> String {
            self.source.clone()
        }
        fn get_failed(&self) -> bool {
            self.failed
        }

        #[qsignal]
        pub fn url_changed(&mut self);
        #[qsignal]
        pub fn kind_changed(&mut self);
        #[qsignal]
        pub fn source_changed(&mut self);
        #[qsignal]
        pub fn failed_changed(&mut self);
    }

    impl RemoteImage {
        fn image_kind(&self) -> ImageKind {
            ImageKind::parse(&self.kind).unwrap_or(ImageKind::Icon(128))
        }

        fn update(&mut self) {
            if !self.complete {
                return;
            }
            self.generation += 1;
            let generation = self.generation;
            let url = self.url.clone();
            let kind = self.image_kind();
            let loader = crate::services::services().images.clone();
            if url.is_empty() {
                self.publish(String::new(), false);
                return;
            }
            if let Some(path) = loader.cached(&url, kind) {
                self.publish(file_url(&path), false);
                return;
            }
            self.publish(String::new(), false);
            let handle = self.get_thread_handle();
            crate::bridge::spawn(async move {
                let result = loader.ensure(&url, kind).await;
                handle.invoke_mut(move |img| {
                    if img.generation != generation {
                        return;
                    }
                    match result {
                        Ok(path) => img.publish(file_url(&path), false),
                        Err(e) => {
                            klog!("image {}: {}", url, e);
                            img.publish(String::new(), true);
                        }
                    }
                });
            });
        }

        fn publish(&mut self, source: String, failed: bool) {
            let source_changed = source != self.source;
            let failed_changed = failed != self.failed;
            self.source = source;
            self.failed = failed;
            if source_changed {
                self.source_changed();
            }
            if failed_changed {
                self.failed_changed();
            }
        }
    }

    fn file_url(path: &std::path::Path) -> String {
        // Cache paths are `<hex>-<kind>.png` below the cache dir; percent-encode
        // anything outside the unreserved set in the directory part.
        let mut out = String::from("file://");
        for b in path.to_string_lossy().bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(b as char)
                }
                _ => out.push_str(&format!("%{b:02X}")),
            }
        }
        out
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn file_urls_are_percent_encoded() {
            assert_eq!(
                super::file_url(std::path::Path::new("/home/a b/c-icon64.png")),
                "file:///home/a%20b/c-icon64.png"
            );
        }
    }
}

qml_element!(RemoteImage, "RemoteImage");
