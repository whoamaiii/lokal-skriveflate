mod agent {
    pub mod codex_bridge {
        pub struct CodexBridge;

        impl CodexBridge {
            pub fn binary_path() -> Option<String> {
                None
            }
        }
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
