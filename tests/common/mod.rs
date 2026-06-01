use std::ffi::OsString;
use std::io::{Cursor, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use zip::write::SimpleFileOptions;
use zip::ZipWriter;

pub struct TestHome {
    saved_home: Option<OsString>,
    #[cfg(windows)]
    saved_profile: Option<OsString>,
}

impl TestHome {
    pub fn set(path: &Path) -> Self {
        let saved_home = std::env::var_os("HOME");
        #[cfg(windows)]
        let saved_profile = std::env::var_os("USERPROFILE");
        std::env::set_var("HOME", path);
        #[cfg(windows)]
        std::env::set_var("USERPROFILE", path);
        Self {
            saved_home,
            #[cfg(windows)]
            saved_profile,
        }
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        restore_env("HOME", self.saved_home.take());
        #[cfg(windows)]
        restore_env("USERPROFILE", self.saved_profile.take());
    }
}

fn restore_env(key: &str, value: Option<OsString>) {
    match value {
        Some(v) => std::env::set_var(key, v),
        None => std::env::remove_var(key),
    }
}

pub fn build_cell_zip(cell_name: &str, payload: &[u8]) -> Vec<u8> {
    let mut buffer = Vec::new();
    {
        let cursor = Cursor::new(&mut buffer);
        let mut zip = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default();
        let entry = format!("ENC_ROOT/{cell_name}/{cell_name}.000");
        zip.start_file(entry, options).unwrap();
        zip.write_all(payload).unwrap();
        zip.finish().unwrap();
    }
    buffer
}

pub fn catalog_xml(base_url: &str) -> String {
    format!(
        r#"<?xml version="1.0"?>
<catalog>
  <cell>
    <name>US5CA01M</name>
    <zipfile_location>{base_url}/US5CA01M.zip</zipfile_location>
    <zipfile_datetime_iso8601>2024-06-01T00:00:00Z</zipfile_datetime_iso8601>
    <state>CA</state>
  </cell>
  <cell>
    <name>US5FL01M</name>
    <zipfile_location>{base_url}/US5FL01M.zip</zipfile_location>
    <zipfile_datetime_iso8601>2024-06-02T00:00:00Z</zipfile_datetime_iso8601>
    <state>FL</state>
  </cell>
</catalog>
"#
    )
}

pub struct MockHttpServer {
    pub zip_downloads: Arc<AtomicUsize>,
    pub base_url: String,
    _handle: JoinHandle<()>,
}

impl MockHttpServer {
    pub fn start(ca_zip: Vec<u8>, fl_zip: Vec<u8>) -> Self {
        let zip_downloads = Arc::new(AtomicUsize::new(0));
        let counts = zip_downloads.clone();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{port}");
        let catalog = catalog_xml(&base_url);
        let server = tiny_http::Server::from_listener(listener, None).unwrap();

        let handle = thread::spawn(move || {
            for request in server.incoming_requests() {
                let path = request.url().split('?').next().unwrap_or("");
                let response = if path.ends_with("ENCProdCat.xml") {
                    tiny_http::Response::from_string(catalog.clone()).with_status_code(200)
                } else if path.ends_with("US5CA01M.zip") {
                    counts.fetch_add(1, Ordering::SeqCst);
                    tiny_http::Response::from_data(ca_zip.clone()).with_status_code(200)
                } else if path.ends_with("US5FL01M.zip") {
                    counts.fetch_add(1, Ordering::SeqCst);
                    tiny_http::Response::from_data(fl_zip.clone()).with_status_code(200)
                } else {
                    tiny_http::Response::from_string("not found").with_status_code(404)
                };
                let _ = request.respond(response);
            }
        });

        Self {
            zip_downloads,
            base_url,
            _handle: handle,
        }
    }
}
