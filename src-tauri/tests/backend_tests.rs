mod agent {
    pub mod codex_bridge {
        pub struct CodexBridge;

        impl CodexBridge {
            pub fn binary_path() -> Option<String> {
                None
            }
        }
    }

    pub mod responses_proxy {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/agent/responses_proxy.rs"));
    }
}

#[path = "../src/models.rs"]
mod models;

#[path = "../src/export.rs"]
mod export;

#[path = "../src/storage.rs"]
mod storage;

#[path = "../src/agent/local_model.rs"]
mod local_model;
