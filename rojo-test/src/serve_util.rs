use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

use rbx_dom_weak::RbxId;
use serde::Serialize;
use tempfile::{tempdir, TempDir};

use librojo::web_interface::{Instance, ReadResponse, ServerInfoResponse, SubscribeResponse};
use rojo_insta_ext::RedactionMap;

use crate::util::{
    copy_recursive, get_rojo_path, get_serve_tests_path, get_working_dir_path, KillOnDrop,
};

pub fn run_serve_test(test_name: &str, callback: impl FnOnce(TestServeSession, RedactionMap)) {
    let _ = env_logger::try_init();

    let mut settings = insta::Settings::new();

    let snapshot_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("serve-test-snapshots");
    settings.set_snapshot_path(snapshot_path);
    settings.set_sort_maps(true);

    let mut redactions = RedactionMap::new();

    let mut session = TestServeSession::new(test_name);
    let info = session.wait_to_come_online();

    redactions.intern(info.session_id);
    redactions.intern(info.root_instance_id);

    settings.bind(move || callback(session, redactions));
}

fn get_port_number() -> usize {
    static NEXT_PORT_NUMBER: AtomicUsize = AtomicUsize::new(35103);

    NEXT_PORT_NUMBER.fetch_add(1, Ordering::SeqCst)
}

pub struct TestServeSession {
    // Drop order is important here: we want the process to be killed before the
    // directory it's operating on is destroyed.
    rojo_process: KillOnDrop,
    _dir: TempDir,

    port: usize,
    project_path: PathBuf,
}

impl TestServeSession {
    pub fn new(name: &str) -> Self {
        let serve_test_path = get_serve_tests_path();
        let working_dir = get_working_dir_path();
        let exe_path = get_rojo_path();

        let source_path = serve_test_path.join(name);
        let dir = tempdir().expect("Couldn't create temporary directory");
        let project_path = dir.path().join(name);

        let source_is_file = fs::metadata(&source_path).unwrap().is_file();

        if source_is_file {
            fs::copy(&source_path, &project_path).expect("couldn't copy project file");
        } else {
            fs::create_dir(&project_path).expect("Couldn't create temporary project subdirectory");

            copy_recursive(&source_path, &project_path)
                .expect("Couldn't copy project to temporary directory");
        };

        let port = get_port_number();
        let port_string = port.to_string();

        let rojo_process = Command::new(exe_path)
            .args(&[
                "serve",
                project_path.to_str().unwrap(),
                "--port",
                port_string.as_str(),
            ])
            .current_dir(working_dir)
            .spawn()
            .expect("Couldn't start Rojo");

        TestServeSession {
            rojo_process: KillOnDrop(rojo_process),
            _dir: dir,
            port,
            project_path,
        }
    }

    pub fn path(&self) -> &Path {
        &self.project_path
    }

    pub fn wait_to_come_online(&mut self) -> ServerInfoResponse {
        loop {
            match self.rojo_process.0.try_wait() {
                Ok(Some(status)) => panic!("Rojo process exited with status {}", status),
                Ok(None) => { /* process is still running */ }
                Err(err) => panic!("Failed to wait on Rojo process: {}", err),
            }

            let info = match self.get_api_rojo() {
                Ok(info) => info,
                Err(err) => {
                    log::debug!("Server error, retrying: {}", err);
                    std::thread::sleep(std::time::Duration::from_millis(30));
                    continue;
                }
            };

            log::info!("Got session info: {:?}", info);

            return info;
        }
    }

    pub fn get_api_rojo(&self) -> Result<ServerInfoResponse, reqwest::Error> {
        let url = format!("http://localhost:{}/api/rojo", self.port);
        let body = reqwest::get(&url)?.text()?;

        Ok(serde_json::from_str(&body).expect("Server returned malformed response"))
    }

    pub fn get_api_read(&self, id: RbxId) -> Result<ReadResponse, reqwest::Error> {
        let url = format!("http://localhost:{}/api/read/{}", self.port, id);
        let body = reqwest::get(&url)?.text()?;

        Ok(serde_json::from_str(&body).expect("Server returned malformed response"))
    }

    pub fn get_api_subscribe(
        &self,
        cursor: u32,
    ) -> Result<SubscribeResponse<'static>, reqwest::Error> {
        let url = format!("http://localhost:{}/api/subscribe/{}", self.port, cursor);

        reqwest::get(&url)?.json()
    }
}
