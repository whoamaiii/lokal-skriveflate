use std::{
    collections::HashMap,
    env,
    fs::{self, File},
    io::{BufReader, Read},
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Mutex as StdMutex,
    time::{Duration, SystemTime},
};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::{
    process::{Child, Command},
    sync::Mutex,
    time::sleep,
};
use tracing::{info, warn};

use crate::models::{
    AppSettings, LocalModelOption, RuntimeBlockingReason, RuntimeComponentStatus, RuntimePhase,
    RuntimeStatus, DEFAULT_MODEL,
};

use crate::agent::responses_proxy::{start_responses_proxy, ResponsesProxyHandle};

const LOCAL_API_KEY: &str = "lokal-skriveflate";
const LOCAL_RUNTIME_CTX_SIZE: &str = "16384";
const RUNTIME_DIR_NAME: &str = "runtime";
const RUNTIME_MANIFEST_RELATIVE_PATH: &str = "embedded-runtime/manifest.json";
const RUNTIME_START_TIMEOUT_ATTEMPTS: usize = 240;
const RUNTIME_START_TIMEOUT_STEP_MS: u64 = 500;

#[allow(dead_code)]
#[async_trait]
pub trait LocalModelProvider {
    async fn runtime_status(&self, settings: &AppSettings) -> RuntimeStatus;
    async fn prepare_runtime(&self, settings: &AppSettings) -> Result<String>;
    async fn start_runtime(&self, settings: &AppSettings) -> Result<String>;
    async fn healthcheck(&self, settings: &AppSettings) -> Result<String>;
    async fn list_models(&self, settings: &AppSettings) -> Result<Vec<LocalModelOption>>;
    async fn activate_model(&self, model: &str) -> Result<String>;
    async fn repair_runtime(&self, settings: &AppSettings) -> Result<String>;
    async fn runtime_endpoint(&self, settings: &AppSettings) -> Result<String>;
    async fn shutdown(&self) -> Result<()>;
}

pub struct EmbeddedLlamaCppProvider {
    client: reqwest::Client,
    app_data_root: PathBuf,
    resource_root: PathBuf,
    app_version: String,
    managed_runtime: Mutex<Option<ManagedRuntime>>,
    lifecycle_gate: Mutex<()>,
    sha_cache: StdMutex<HashMap<PathBuf, ShaCacheEntry>>,
}

struct ManagedRuntime {
    child: Child,
    port: u16,
    _llama_port: u16,
    model_id: String,
    proxy: ResponsesProxyHandle,
}

#[derive(Debug, Clone)]
struct ShaCacheEntry {
    len: u64,
    modified: Option<SystemTime>,
    digest: String,
}

#[derive(Debug, Deserialize)]
struct EmbeddedRuntimeManifest {
    #[serde(default = "default_manifest_schema_version")]
    schema_version: u32,
    #[serde(default)]
    bundle_version: String,
    engine: RuntimeEngineAsset,
    models: Vec<RuntimeModelAsset>,
}

#[derive(Debug, Deserialize)]
struct RuntimeEngineAsset {
    resource: String,
    target_name: String,
    sha256: String,
    placeholder: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct RuntimeModelAsset {
    id: String,
    label: String,
    tier: String,
    resource: String,
    filename: String,
    sha256: String,
    bundled: bool,
    placeholder: bool,
}

#[derive(Debug, Clone)]
struct AssetInspection {
    installed: bool,
    needs_repair: bool,
    details: Option<String>,
}

#[derive(Debug, Clone)]
struct RuntimeInspection {
    runtime_home: PathBuf,
    engine_path: PathBuf,
    engine: AssetInspection,
    models: Vec<(RuntimeModelAsset, AssetInspection)>,
}

impl EmbeddedLlamaCppProvider {
    fn allow_dev_overrides() -> bool {
        cfg!(debug_assertions)
    }

    fn staged_sidecar_source(target_name: &str) -> Option<PathBuf> {
        let cwd = env::current_dir().ok()?;
        let binaries_dir = cwd.join("src-tauri").join("binaries");

        [
            binaries_dir.join(target_name),
            binaries_dir.join(format!("{target_name}-aarch64-apple-darwin")),
            binaries_dir.join(format!("{target_name}-x86_64-apple-darwin")),
        ]
        .into_iter()
        .find(|path| path.exists())
    }

    pub fn new(app_data_root: PathBuf, resource_root: PathBuf, app_version: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            app_data_root,
            resource_root,
            app_version,
            managed_runtime: Mutex::new(None),
            lifecycle_gate: Mutex::new(()),
            sha_cache: StdMutex::new(HashMap::new()),
        }
    }

    fn runtime_home(&self) -> PathBuf {
        self.app_data_root
            .join(RUNTIME_DIR_NAME)
            .join(&self.app_version)
    }

    fn runtime_log_dir(&self) -> PathBuf {
        self.app_data_root.join("logs")
    }

    fn manifest_path(&self) -> PathBuf {
        self.resource_root.join(RUNTIME_MANIFEST_RELATIVE_PATH)
    }

    fn engine_target_path(&self, manifest: &EmbeddedRuntimeManifest) -> PathBuf {
        self.runtime_home()
            .join("bin")
            .join(&manifest.engine.target_name)
    }

    fn model_target_path(&self, model: &RuntimeModelAsset) -> PathBuf {
        self.runtime_home().join("models").join(&model.filename)
    }

    fn catalog_model<'a>(
        manifest: &'a EmbeddedRuntimeManifest,
        model_id: &str,
    ) -> Result<&'a RuntimeModelAsset> {
        manifest
            .models
            .iter()
            .find(|model| model.id == model_id)
            .with_context(|| format!("Fant ikke modellkatalog for `{model_id}`"))
    }

    fn bundled_engine_source(&self, manifest: &EmbeddedRuntimeManifest) -> PathBuf {
        self.resource_root.join(&manifest.engine.resource)
    }

    fn packaged_sidecar_source(target_name: &str) -> Option<PathBuf> {
        let current_exe = env::current_exe().ok()?;
        let executable_dir = current_exe.parent()?;

        [
            executable_dir.join(target_name),
            executable_dir.join(format!("{target_name}-aarch64-apple-darwin")),
            executable_dir.join(format!("{target_name}-x86_64-apple-darwin")),
        ]
        .into_iter()
        .find(|path| path.exists())
    }

    fn packaged_sidecar_source_from_resource_root(&self, target_name: &str) -> Option<PathBuf> {
        let resources_dir = self.resource_root.parent()?;
        let contents_dir = resources_dir.parent()?;
        let executable_dir = contents_dir.join("MacOS");

        [
            executable_dir.join(target_name),
            executable_dir.join(format!("{target_name}-aarch64-apple-darwin")),
            executable_dir.join(format!("{target_name}-x86_64-apple-darwin")),
        ]
        .into_iter()
        .find(|path| path.exists())
    }

    fn bundled_model_source(&self, model: &RuntimeModelAsset) -> PathBuf {
        self.resource_root.join(&model.resource)
    }

    fn engine_library_source_dir(engine_source: &Path) -> Option<PathBuf> {
        let executable_dir = engine_source.parent()?;
        let sibling_dir = executable_dir.join("lib");
        if sibling_dir.exists() {
            return Some(sibling_dir);
        }

        executable_dir.parent().map(|parent| parent.join("lib")).filter(|path| path.exists())
    }

    fn resolve_engine_source(&self, manifest: &EmbeddedRuntimeManifest) -> Result<PathBuf> {
        if Self::allow_dev_overrides() {
            if let Some(path) = env::var("LOKAL_AI_BINARY_PATH")
                .ok()
                .map(PathBuf::from)
                .filter(|path| path.exists())
            {
                return Ok(path);
            }
        }

        if let Some(path) = self.packaged_sidecar_source_from_resource_root(&manifest.engine.target_name) {
            return Ok(path);
        }

        if !Self::allow_dev_overrides() {
            return Self::packaged_sidecar_source(&manifest.engine.target_name)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Fant ikke bundlet llama.cpp-sidecar `{}` i app-pakken.",
                        manifest.engine.target_name
                    )
                });
        }

        let bundled = self.bundled_engine_source(manifest);
        if bundled.exists() {
            return Ok(bundled);
        }

        if let Some(path) = Self::staged_sidecar_source(&manifest.engine.target_name) {
            return Ok(path);
        }

        Ok(bundled)
    }

    fn resolve_model_source(&self, model: &RuntimeModelAsset) -> PathBuf {
        if !Self::allow_dev_overrides() {
            return self.bundled_model_source(model);
        }

        if let Some(path) = env::var("LOKAL_AI_MODEL_PATH")
            .ok()
            .map(PathBuf::from)
            .filter(|path| path.exists())
        {
            return path;
        }

        if let Some(path) = env::var("LOKAL_AI_STANDARD_MODEL_PATH")
            .ok()
            .map(PathBuf::from)
            .filter(|path| path.exists())
        {
            return path;
        }

        self.bundled_model_source(model)
    }

    fn load_manifest(&self) -> Result<EmbeddedRuntimeManifest> {
        let manifest_path = self.manifest_path();
        let raw = fs::read_to_string(&manifest_path).with_context(|| {
            format!(
                "Fant ikke runtime-manifestet på {}",
                manifest_path.display()
            )
        })?;
        serde_json::from_str(&raw).with_context(|| {
            format!(
                "Kunne ikke lese runtime-manifestet på {}",
                manifest_path.display()
            )
        })
    }

    fn sha256_file(&self, path: &Path) -> Result<String> {
        let metadata = fs::metadata(path)
            .with_context(|| format!("Kunne ikke lese metadata for {}", path.display()))?;
        let len = metadata.len();
        let modified = metadata.modified().ok();

        if let Ok(cache) = self.sha_cache.lock() {
            if let Some(entry) = cache.get(path) {
                if entry.len == len && entry.modified == modified {
                    return Ok(entry.digest.clone());
                }
            }
        }

        let file = File::open(path)
            .with_context(|| format!("Kunne ikke lese {} for sjekksum", path.display()))?;
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];

        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }

        let digest = format!("{:x}", hasher.finalize());
        if let Ok(mut cache) = self.sha_cache.lock() {
            cache.insert(
                path.to_path_buf(),
                ShaCacheEntry {
                    len,
                    modified,
                    digest: digest.clone(),
                },
            );
        }

        Ok(digest)
    }

    fn clear_sha_cache(&self) {
        if let Ok(mut cache) = self.sha_cache.lock() {
            cache.clear();
        }
    }

    fn inspect_asset(
        &self,
        target_path: PathBuf,
        expected_sha: &str,
        placeholder: bool,
        kind_name: &str,
    ) -> AssetInspection {
        if !target_path.exists() {
            return AssetInspection {
                installed: false,
                needs_repair: false,
                details: Some(format!("{kind_name} er ikke klargjort ennå.")),
            };
        }

        match self.sha256_file(&target_path) {
            Ok(actual_sha) if actual_sha == expected_sha => {
                let details = if placeholder {
                    Some(format!(
                        "{kind_name} er en utvikler-placeholder. Erstatt ressursfila med ekte release-artefakt eller bruk miljøvariabler for lokal test."
                    ))
                } else {
                    Some(format!("{kind_name} er verifisert fra bundlet pakke."))
                };

                AssetInspection {
                    installed: !placeholder,
                    needs_repair: false,
                    details,
                }
            }
            Ok(_) if placeholder => AssetInspection {
                installed: true,
                needs_repair: false,
                details: Some(format!("{kind_name} bruker lokal override og ser klar ut.")),
            },
            Ok(_) => AssetInspection {
                installed: false,
                needs_repair: true,
                details: Some(format!("{kind_name} finnes, men sjekksummen stemmer ikke.")),
            },
            Err(error) => AssetInspection {
                installed: false,
                needs_repair: true,
                details: Some(format!("{kind_name} kunne ikke valideres: {error}")),
            },
        }
    }

    fn inspect_runtime(&self) -> Result<RuntimeInspection> {
        let manifest = self.load_manifest()?;
        info!(
            schema_version = manifest.schema_version,
            bundle_version = manifest.bundle_version,
            "leser runtime-manifest"
        );
        let runtime_home = self.runtime_home();
        let engine = self.inspect_asset(
            self.engine_target_path(&manifest),
            &manifest.engine.sha256,
            manifest.engine.placeholder,
            "Lokal runtime",
        );

        let models = manifest
            .models
            .iter()
            .cloned()
            .map(|model| {
                let inspection = self.inspect_asset(
                    self.model_target_path(&model),
                    &model.sha256,
                    model.placeholder,
                    &format!("Modellen {}", model.label),
                );
                (model, inspection)
            })
            .collect();

        Ok(RuntimeInspection {
            runtime_home,
            engine_path: self.engine_target_path(&manifest),
            engine,
            models,
        })
    }

    fn copy_asset(
        &self,
        source: &Path,
        target: &Path,
        expected_sha: Option<&str>,
        executable: bool,
        force: bool,
    ) -> Result<bool> {
        if !source.exists() {
            bail!("Fant ikke ressursfila {}", source.display());
        }

        if !force && target.exists() {
            if let Some(expected_sha) = expected_sha {
                if let Ok(actual_sha) = self.sha256_file(target) {
                    if actual_sha == expected_sha {
                        return Ok(false);
                    }
                }
            } else {
                return Ok(false);
            }
        }

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::copy(source, target).with_context(|| {
            format!(
                "Kunne ikke kopiere {} til {}",
                source.display(),
                target.display()
            )
        })?;

        if executable {
            let mut permissions = fs::metadata(target)?.permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(target, permissions)?;
        }

        if let Some(expected_sha) = expected_sha {
            let actual_sha = self.sha256_file(target)?;
            if actual_sha != expected_sha {
                bail!(
                    "Sjekksum stemmer ikke for {} etter kopiering",
                    target.display()
                );
            }
        }

        Ok(true)
    }

    fn prepare_runtime_files(&self, selected_model: &str, force: bool) -> Result<usize> {
        let manifest = self.load_manifest()?;
        let runtime_home = self.runtime_home();
        fs::create_dir_all(runtime_home.join("bin"))?;
        fs::create_dir_all(runtime_home.join("models"))?;

        let mut copied = 0usize;

        let engine_source = self.resolve_engine_source(&manifest)?;
        copied += usize::from(self.copy_asset(
            &engine_source,
            &self.engine_target_path(&manifest),
            if engine_source == self.bundled_engine_source(&manifest) {
                Some(manifest.engine.sha256.as_str())
            } else {
                None
            },
            true,
            force,
        )?);
        copied += self.copy_engine_libraries(&engine_source, force)?;

        for model in &manifest.models {
            let source = self.resolve_model_source(model);
            if !source.exists() {
                if model.bundled {
                    bail!("Fant ikke ressursfila {}", source.display());
                }
                if model.id == selected_model {
                    continue;
                }
                continue;
            }

            copied += usize::from(self.copy_asset(
                &source,
                &self.model_target_path(model),
                if source == self.bundled_model_source(model) {
                    Some(model.sha256.as_str())
                } else {
                    None
                },
                false,
                force,
            )?);
        }

        self.clear_sha_cache();
        Ok(copied)
    }

    fn copy_engine_libraries(&self, engine_source: &Path, force: bool) -> Result<usize> {
        let Some(source_dir) = Self::engine_library_source_dir(engine_source) else {
            return Ok(0);
        };

        let target_dir = self.runtime_home().join("lib");
        fs::create_dir_all(&target_dir)?;

        let mut copied = 0usize;
        for entry in fs::read_dir(&source_dir)
            .with_context(|| format!("Kunne ikke lese støttebibliotekene i {}", source_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            let is_dylib = path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("dylib"));

            if !is_dylib {
                continue;
            }

            let target = target_dir.join(entry.file_name());
            copied += usize::from(self.copy_asset(&path, &target, None, false, force)?);
        }

        Ok(copied)
    }

    async fn current_runtime_snapshot(&self) -> Option<(u16, String)> {
        let mut guard = self.managed_runtime.lock().await;
        let Some(runtime) = guard.as_mut() else {
            return None;
        };

        if runtime.proxy.is_finished() {
            *guard = None;
            return None;
        }

        match runtime.child.try_wait() {
            Ok(Some(_)) => {
                *guard = None;
                None
            }
            Ok(None) => Some((runtime.port, runtime.model_id.clone())),
            Err(_) => {
                *guard = None;
                None
            }
        }
    }

    async fn stop_runtime_locked(&self) -> Result<()> {
        let mut guard = self.managed_runtime.lock().await;
        let Some(runtime) = guard.take() else {
            return Ok(());
        };
        drop(guard);

        let mut runtime = runtime;
        runtime.proxy.shutdown().await?;
        cleanup_child(&mut runtime.child).await
    }

    async fn healthcheck_port(&self, port: u16) -> bool {
        let health_url = format!("http://127.0.0.1:{port}/health");
        if let Ok(response) = self.client.get(&health_url).send().await {
            if response.status().is_success() {
                return true;
            }
        }

        let models_url = format!("http://127.0.0.1:{port}/v1/models");
        self.client
            .get(&models_url)
            .bearer_auth(LOCAL_API_KEY)
            .send()
            .await
            .map(|response| response.status().is_success())
            .unwrap_or(false)
    }

    fn runtime_stderr_log_path(&self, port: u16) -> PathBuf {
        self.runtime_log_dir().join(format!("llama-server-{port}.stderr.log"))
    }

    fn read_log_tail(path: &Path, max_bytes: usize) -> String {
        let Ok(bytes) = fs::read(path) else {
            return String::new();
        };

        let start = bytes.len().saturating_sub(max_bytes);
        String::from_utf8_lossy(&bytes[start..]).trim().to_string()
    }

    async fn wait_for_health(&self, port: u16, child: &mut Child, stderr_log_path: &Path) -> Result<()> {
        for _ in 0..RUNTIME_START_TIMEOUT_ATTEMPTS {
            if self.healthcheck_port(port).await {
                return Ok(());
            }

            if let Some(status) = child.try_wait()? {
                let log_tail = Self::read_log_tail(stderr_log_path, 8 * 1024);
                if log_tail.is_empty() {
                    bail!(
                        "Lokal runtime avsluttet før den ble klar på localhost:{port} (status: {status}). Se {}",
                        stderr_log_path.display()
                    );
                }

                bail!(
                    "Lokal runtime avsluttet før den ble klar på localhost:{port} (status: {status}). Siste runtime-logg fra {}:\n{}",
                    stderr_log_path.display(),
                    log_tail
                );
            }

            sleep(Duration::from_millis(RUNTIME_START_TIMEOUT_STEP_MS)).await;
        }

        let log_tail = Self::read_log_tail(stderr_log_path, 8 * 1024);
        if log_tail.is_empty() {
            bail!(
                "Lokal runtime startet ikke innen tidsfristen på localhost:{port}. Se {}",
                stderr_log_path.display()
            );
        }

        bail!(
            "Lokal runtime startet ikke innen tidsfristen på localhost:{port}. Siste runtime-logg fra {}:\n{}",
            stderr_log_path.display(),
            log_tail
        )
    }

    fn select_free_port(&self) -> Result<u16> {
        let socket =
            TcpListener::bind("127.0.0.1:0").context("Kunne ikke finne en ledig localhost-port")?;
        let port = socket
            .local_addr()
            .context("Kunne ikke lese localhost-port")?
            .port();
        drop(socket);
        Ok(port)
    }

    async fn ensure_runtime_assets(&self, settings: &AppSettings) -> Result<RuntimeInspection> {
        let inspection = self.inspect_runtime()?;
        let selected = inspection
            .models
            .iter()
            .find(|(model, _)| model.id == settings.selected_model)
            .with_context(|| format!("Fant ikke valgt modell `{}`", settings.selected_model))?;

        if inspection.engine.installed && selected.1.installed {
            return Ok(inspection);
        }

        self.prepare_runtime_files(&settings.selected_model, false)?;
        let refreshed = self.inspect_runtime()?;
        let refreshed_selected = refreshed
            .models
            .iter()
            .find(|(model, _)| model.id == settings.selected_model)
            .with_context(|| format!("Fant ikke valgt modell `{}`", settings.selected_model))?;

        if !refreshed.engine.installed {
            bail!(
                "{}",
                refreshed
                    .engine
                    .details
                    .clone()
                    .unwrap_or_else(|| "Lokal runtime er ikke klar.".to_string())
            );
        }

        if !refreshed_selected.1.installed {
            bail!(
                "{}",
                refreshed_selected
                    .1
                    .details
                    .clone()
                    .unwrap_or_else(|| "Valgt modell er ikke klar.".to_string())
            );
        }

        Ok(refreshed)
    }

    fn runtime_state_for(
        &self,
        inspection: &RuntimeInspection,
        selected_model: &str,
        local_ai_running: bool,
    ) -> RuntimePhase {
        let selected = inspection
            .models
            .iter()
            .find(|(model, _)| model.id == selected_model)
            .map(|(_, inspection)| inspection);

        if inspection.engine.needs_repair || selected.is_some_and(|selected| selected.needs_repair)
        {
            RuntimePhase::RepairRequired
        } else if inspection.engine.installed && selected.is_some_and(|selected| selected.installed)
        {
            if local_ai_running {
                RuntimePhase::Ready
            } else {
                RuntimePhase::Degraded
            }
        } else {
            RuntimePhase::NotPrepared
        }
    }

    fn local_component_status(
        &self,
        inspection: &RuntimeInspection,
        selected_model: &str,
        is_running: bool,
    ) -> RuntimeComponentStatus {
        let selected = inspection
            .models
            .iter()
            .find(|(model, _)| model.id == selected_model);

        let available = inspection.engine.installed
            && selected.is_some_and(|(_, model_inspection)| model_inspection.installed);

        let details = if inspection.engine.needs_repair {
            inspection.engine.details.clone()
        } else if let Some((_, model_inspection)) = selected {
            if model_inspection.needs_repair || !model_inspection.installed {
                model_inspection.details.clone()
            } else if is_running {
                Some("Lokal runtime svarer via localhost.".to_string())
            } else {
                Some("Lokal runtime er klargjort og starter ved behov.".to_string())
            }
        } else {
            Some("Standardmodellen finnes ikke i runtime-katalogen.".to_string())
        };

        RuntimeComponentStatus {
            available,
            running: is_running,
            details,
        }
    }

    fn model_options(
        &self,
        inspection: &RuntimeInspection,
        selected_model: &str,
    ) -> Vec<LocalModelOption> {
        inspection
            .models
            .iter()
            .filter(|(model, _)| model.bundled || model.id == selected_model)
            .map(|(model, model_inspection)| LocalModelOption {
                id: model.id.clone(),
                label: model.label.clone(),
                tier: model.tier.clone(),
                bundled: model.bundled,
                installed: model_inspection.installed,
                active: model.id == selected_model,
                details: model_inspection.details.clone(),
            })
            .collect()
    }

    fn readiness_for(
        &self,
        runtime_state: &RuntimePhase,
        codex_available: bool,
    ) -> (bool, bool, Option<RuntimeBlockingReason>) {
        if !codex_available {
            return (false, false, Some(RuntimeBlockingReason::CodexUnavailable));
        }

        match runtime_state {
            RuntimePhase::Ready => (true, false, None),
            RuntimePhase::Degraded => (true, true, None),
            RuntimePhase::RepairRequired => {
                (false, false, Some(RuntimeBlockingReason::RuntimeRepairRequired))
            }
            RuntimePhase::NotPrepared | RuntimePhase::Extracting => {
                (false, false, Some(RuntimeBlockingReason::RuntimeNotPrepared))
            }
        }
    }

    async fn start_runtime_locked(&self, settings: &AppSettings) -> Result<String> {
        if let Some((port, model_id)) = self.current_runtime_snapshot().await {
            if model_id == settings.selected_model && self.healthcheck_port(port).await {
                return Ok(format!(
                    "Lokal runtime kjører allerede på http://127.0.0.1:{port}/v1"
                ));
            }
            self.stop_runtime_locked().await?;
        }

        let inspection = self.ensure_runtime_assets(settings).await?;
        let manifest = self.load_manifest()?;
        let selected_model = Self::catalog_model(&manifest, &settings.selected_model)?;
        let binary_path = inspection.engine_path.clone();
        let model_path = self.model_target_path(selected_model);
        let llama_port = self.select_free_port()?;
        let proxy_port = self.select_free_port()?;
        let stderr_log_path = self.runtime_stderr_log_path(llama_port);
        if let Some(parent) = stderr_log_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let stderr_log = File::create(&stderr_log_path).with_context(|| {
            format!(
                "Kunne ikke opprette runtime-logg på {}",
                stderr_log_path.display()
            )
        })?;
        info!(
            model_id = %settings.selected_model,
            llama_port,
            proxy_port,
            binary_path = %binary_path.display(),
            model_path = %model_path.display(),
            stderr_log_path = %stderr_log_path.display(),
            "starter lokal runtime"
        );

        let mut child = Command::new(&binary_path)
            .arg("-m")
            .arg(&model_path)
            .arg("-a")
            .arg(&settings.selected_model)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(llama_port.to_string())
            .arg("--ctx-size")
            .arg(LOCAL_RUNTIME_CTX_SIZE)
            .arg("--api-key")
            .arg(LOCAL_API_KEY)
            // Skip llama.cpp's empty warm-up run so first startup becomes deterministic in packaged builds.
            .arg("--no-warmup")
            .stdout(Stdio::null())
            .stderr(Stdio::from(stderr_log))
            .spawn()
            .with_context(|| {
                format!(
                    "Kunne ikke starte lokal runtime via {}",
                    binary_path.display()
                )
            })?;

        if let Err(error) = self
            .wait_for_health(llama_port, &mut child, &stderr_log_path)
            .await
        {
            warn!(model_id = %settings.selected_model, llama_port, error = %error, "lokal runtime ble ikke frisk i tide");
            let _ = cleanup_child(&mut child).await;
            return Err(error);
        }

        let proxy = match start_responses_proxy(
            proxy_port,
            format!("http://127.0.0.1:{llama_port}/v1"),
            LOCAL_API_KEY.to_string(),
            settings.selected_model.clone(),
        )
        .await
        {
            Ok(proxy) => proxy,
            Err(error) => {
                let _ = cleanup_child(&mut child).await;
                return Err(error);
            }
        };

        let mut guard = self.managed_runtime.lock().await;
        *guard = Some(ManagedRuntime {
            child,
            port: proxy.port(),
            _llama_port: llama_port,
            model_id: settings.selected_model.clone(),
            proxy,
        });

        Ok(format!(
            "Startet lokal llama.cpp-runtime på http://127.0.0.1:{llama_port}/v1 og responses-proxy på http://127.0.0.1:{proxy_port}/v1"
        ))
    }
}

#[async_trait]
impl LocalModelProvider for EmbeddedLlamaCppProvider {
    async fn runtime_status(&self, settings: &AppSettings) -> RuntimeStatus {
        let codex_path = crate::agent::codex_bridge::CodexBridge::binary_path();
        let codex_available = codex_path.is_some();

        let inspection = self.inspect_runtime();
        let active_runtime = self.current_runtime_snapshot().await;
        let local_running = if let Some((port, model_id)) = active_runtime.as_ref() {
            model_id == &settings.selected_model && self.healthcheck_port(*port).await
        } else {
            false
        };

        if let Ok(ref inspection) = inspection {
            let runtime_state =
                self.runtime_state_for(inspection, &settings.selected_model, local_running);
            let (can_send, will_start_on_demand, blocking_reason) =
                self.readiness_for(&runtime_state, codex_available);
            RuntimeStatus {
                offline_mode: true,
                local_only: true,
                selected_model: settings.selected_model.clone(),
                runtime_state,
                can_send,
                will_start_on_demand,
                blocking_reason,
                codex: RuntimeComponentStatus {
                    available: codex_available,
                    running: false,
                    details: codex_path.map(|path| format!("Codex funnet på {path}")),
                },
                local_ai: self.local_component_status(
                    inspection,
                    &settings.selected_model,
                    local_running,
                ),
                available_models: self.model_options(inspection, &settings.selected_model),
                runtime_home: Some(inspection.runtime_home.display().to_string()),
                listen_address: active_runtime
                    .as_ref()
                    .map(|(port, _)| format!("http://127.0.0.1:{port}/v1")),
            }
        } else {
            RuntimeStatus {
                offline_mode: true,
                local_only: true,
                selected_model: settings.selected_model.clone(),
                runtime_state: RuntimePhase::RepairRequired,
                can_send: false,
                will_start_on_demand: false,
                blocking_reason: Some(if codex_available {
                    RuntimeBlockingReason::RuntimeStartFailed
                } else {
                    RuntimeBlockingReason::CodexUnavailable
                }),
                codex: RuntimeComponentStatus {
                    available: codex_available,
                    running: false,
                    details: codex_path.map(|path| format!("Codex funnet på {path}")),
                },
                local_ai: RuntimeComponentStatus {
                    available: false,
                    running: false,
                    details: Some(
                        inspection
                            .err()
                            .map(|error| error.to_string())
                            .unwrap_or_else(|| "Kunne ikke lese lokal runtime-status.".to_string()),
                    ),
                },
                available_models: Vec::new(),
                runtime_home: Some(self.runtime_home().display().to_string()),
                listen_address: None,
            }
        }
    }

    async fn prepare_runtime(&self, settings: &AppSettings) -> Result<String> {
        let _lifecycle = self.lifecycle_gate.lock().await;
        info!(model_id = %settings.selected_model, "klargjør lokal runtime");
        let copied = self.prepare_runtime_files(&settings.selected_model, false)?;
        let inspection = self.inspect_runtime()?;
        let selected_model = inspection
            .models
            .iter()
            .find(|(model, _)| model.id == settings.selected_model)
            .map(|(_, status)| status);

        let message = if inspection.engine.installed
            && selected_model.is_some_and(|model| model.installed)
        {
            let started = self.start_runtime_locked(settings).await?;
            format!(
                "Lokal AI er klargjort i {}. Kopierte {copied} filer. {started}",
                inspection.runtime_home.display()
            )
        } else {
            format!(
                "Runtime-mappen er klargjort i {}, men standardmodellen er ikke installert ennå. Stag ekte release-artefakter eller bruk utvikler-overstyringer i debug-bygg.",
                inspection.runtime_home.display()
            )
        };

        Ok(message)
    }

    async fn start_runtime(&self, settings: &AppSettings) -> Result<String> {
        let _lifecycle = self.lifecycle_gate.lock().await;
        self.start_runtime_locked(settings).await
    }

    async fn healthcheck(&self, settings: &AppSettings) -> Result<String> {
        let Some((port, model_id)) = self.current_runtime_snapshot().await else {
            bail!(
                "Lokal runtime kjører ikke akkurat nå for standardmodellen."
            );
        };

        if model_id != settings.selected_model {
            bail!("Lokal runtime kjører, men med en annen modell enn v1-standardmodellen.")
        }

        if self.healthcheck_port(port).await {
            Ok(format!(
                "Lokal runtime svarer via http://127.0.0.1:{port}/v1"
            ))
        } else {
            bail!("Lokal runtime svarer ikke på localhost lenger.")
        }
    }

    async fn list_models(&self, settings: &AppSettings) -> Result<Vec<LocalModelOption>> {
        let inspection = self.inspect_runtime()?;
        Ok(self.model_options(&inspection, &settings.selected_model))
    }

    async fn activate_model(&self, model: &str) -> Result<String> {
        if model != DEFAULT_MODEL {
            bail!("Lokal Skriveflate v1 bruker bare standardmodellen.");
        }

        let inspection = self.inspect_runtime()?;
        let Some((catalog, model_inspection)) = inspection
            .models
            .iter()
            .find(|(catalog, _)| catalog.id == model)
        else {
            bail!("Fant ikke modellvalget `{model}`");
        };

        if !model_inspection.installed {
            bail!(
                "{}",
                model_inspection
                    .details
                    .clone()
                    .unwrap_or_else(|| "Denne modellpakken er ikke klargjort.".to_string())
            );
        }

        Ok(format!("{} er allerede aktiv som standardmodell.", catalog.label))
    }

    async fn repair_runtime(&self, settings: &AppSettings) -> Result<String> {
        let _lifecycle = self.lifecycle_gate.lock().await;
        info!(model_id = %settings.selected_model, "reparerer lokal runtime");
        self.stop_runtime_locked().await?;
        let copied = self.prepare_runtime_files(&settings.selected_model, true)?;
        let inspection = self.inspect_runtime()?;
        let selected_model = inspection
            .models
            .iter()
            .find(|(model, _)| model.id == settings.selected_model)
            .map(|(_, status)| status);

        let mut message = format!(
            "Lokal AI er reparert i {}. Oppdaterte {copied} filer.",
            inspection.runtime_home.display()
        );

        if inspection.engine.installed && selected_model.is_some_and(|model| model.installed) {
            let started = self.start_runtime_locked(settings).await?;
            message.push(' ');
            message.push_str(&started);
        }

        Ok(message)
    }

    async fn runtime_endpoint(&self, settings: &AppSettings) -> Result<String> {
        let _lifecycle = self.lifecycle_gate.lock().await;
        self.start_runtime_locked(settings).await?;
        let Some((port, _)) = self.current_runtime_snapshot().await else {
            bail!("Lokal runtime startet ikke riktig")
        };
        Ok(format!("http://127.0.0.1:{port}/v1"))
    }

    async fn shutdown(&self) -> Result<()> {
        let _lifecycle = self.lifecycle_gate.lock().await;
        info!("stopper lokal runtime ved app-avslutning");
        self.stop_runtime_locked().await
    }
}

fn default_manifest_schema_version() -> u32 {
    1
}

async fn cleanup_child(child: &mut Child) -> Result<()> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }

    let _ = child.start_kill();
    let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn temp_dir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "lokal-skriveflate-local-model-test-{label}-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_file(path: &Path, contents: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn sha(contents: &[u8]) -> String {
        format!("{:x}", Sha256::digest(contents))
    }

    #[test]
    fn prepare_runtime_files_copies_single_blessed_model() {
        let resource_root = temp_dir("resources");
        let app_data_root = temp_dir("app");
        let engine_bytes = b"engine";
        let dylib_bytes = b"lib";
        let standard_bytes = b"standard";

        write_file(
            &resource_root.join("embedded-runtime/staged/llama-server"),
            engine_bytes,
        );
        write_file(
            &resource_root.join("embedded-runtime/staged/lib/libllama.0.dylib"),
            dylib_bytes,
        );
        write_file(
            &resource_root.join("embedded-runtime/staged/neurologg-q4_k_m.gguf"),
            standard_bytes,
        );

        let manifest = serde_json::json!({
            "schema_version": 1,
            "bundle_version": "0.1.0",
            "engine": {
                "resource": "embedded-runtime/staged/llama-server",
                "target_name": "llama-server",
                "sha256": sha(engine_bytes),
                "placeholder": false
            },
            "models": [
                {
                    "id": DEFAULT_MODEL,
                    "label": "Standard",
                    "tier": "Standard",
                    "resource": "embedded-runtime/staged/neurologg-q4_k_m.gguf",
                    "filename": "neurologg-q4_k_m.gguf",
                    "sha256": sha(standard_bytes),
                    "bundled": true,
                    "placeholder": false
                }
            ]
        });
        write_file(
            &resource_root.join("embedded-runtime/manifest.json"),
            serde_json::to_vec_pretty(&manifest).unwrap().as_slice(),
        );

        let provider = EmbeddedLlamaCppProvider::new(
            app_data_root.clone(),
            resource_root,
            "0.1.0".to_string(),
        );

        let copied = provider
            .prepare_runtime_files(DEFAULT_MODEL, false)
            .unwrap();
        let inspection = provider.inspect_runtime().unwrap();
        let dylib_target = provider.runtime_home().join("lib/libllama.0.dylib");

        assert_eq!(copied, 3);
        assert!(inspection.engine.installed);
        assert_eq!(inspection.models.len(), 1);
        assert!(inspection.models[0].1.installed);
        assert_eq!(fs::read(dylib_target).unwrap(), dylib_bytes);
    }

    #[test]
    fn sha_cache_refreshes_when_file_changes() {
        let resource_root = temp_dir("sha-resources");
        let app_data_root = temp_dir("sha-app");
        let provider =
            EmbeddedLlamaCppProvider::new(app_data_root, resource_root, "0.1.0".to_string());
        let file_path = temp_dir("sha-file").join("asset.bin");

        write_file(&file_path, b"first");
        let first = provider.sha256_file(&file_path).unwrap();

        std::thread::sleep(Duration::from_millis(20));
        write_file(&file_path, b"second");
        let second = provider.sha256_file(&file_path).unwrap();

        assert_ne!(first, second);
    }
}
