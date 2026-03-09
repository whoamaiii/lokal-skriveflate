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

use crate::models::{
    AppSettings, LocalModelOption, RuntimeComponentStatus, RuntimePhase, RuntimeStatus,
    DEFAULT_MODEL, QUALITY_MODEL,
};

const LOCAL_API_KEY: &str = "lokal-skriveflate";
const RUNTIME_DIR_NAME: &str = "runtime";
const RUNTIME_MANIFEST_RELATIVE_PATH: &str = "embedded-runtime/manifest.json";

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
    model_id: String,
}

#[derive(Debug, Clone)]
struct ShaCacheEntry {
    len: u64,
    modified: Option<SystemTime>,
    digest: String,
}

#[derive(Debug, Deserialize)]
struct EmbeddedRuntimeManifest {
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

    fn bundled_model_source(&self, model: &RuntimeModelAsset) -> PathBuf {
        self.resource_root.join(&model.resource)
    }

    fn resolve_engine_source(&self, manifest: &EmbeddedRuntimeManifest) -> PathBuf {
        env::var("LOKAL_AI_BINARY_PATH")
            .ok()
            .map(PathBuf::from)
            .filter(|path| path.exists())
            .unwrap_or_else(|| self.bundled_engine_source(manifest))
    }

    fn resolve_model_source(&self, model: &RuntimeModelAsset) -> PathBuf {
        let override_key = match model.id.as_str() {
            DEFAULT_MODEL => "LOKAL_AI_STANDARD_MODEL_PATH",
            QUALITY_MODEL => "LOKAL_AI_QUALITY_MODEL_PATH",
            _ => "",
        };

        if !override_key.is_empty() {
            if let Some(path) = env::var(override_key)
                .ok()
                .map(PathBuf::from)
                .filter(|path| path.exists())
            {
                return path;
            }
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

    fn clear_sha_cache(&self) {
        if let Ok(mut cache) = self.sha_cache.lock() {
            cache.clear();
        }
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

        let engine_source = self.resolve_engine_source(&manifest);
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

    async fn current_runtime_snapshot(&self) -> Option<(u16, String)> {
        let mut guard = self.managed_runtime.lock().await;
        let Some(runtime) = guard.as_mut() else {
            return None;
        };

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
        let Some(mut runtime) = guard.take() else {
            return Ok(());
        };
        drop(guard);

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

    async fn wait_for_health(&self, port: u16) -> Result<()> {
        for _ in 0..40 {
            if self.healthcheck_port(port).await {
                return Ok(());
            }
            sleep(Duration::from_millis(500)).await;
        }

        bail!("Lokal runtime startet ikke innen tidsfristen på localhost:{port}")
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
            Some("Valgt modell finnes ikke i katalogen.".to_string())
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
        let port = self.select_free_port()?;

        let mut child = Command::new(&binary_path)
            .arg("-m")
            .arg(&model_path)
            .arg("-a")
            .arg(&settings.selected_model)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .arg("--ctx-size")
            .arg("4096")
            .arg("--api-key")
            .arg(LOCAL_API_KEY)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| {
                format!(
                    "Kunne ikke starte lokal runtime via {}",
                    binary_path.display()
                )
            })?;

        if let Err(error) = self.wait_for_health(port).await {
            let _ = cleanup_child(&mut child).await;
            return Err(error);
        }

        let mut guard = self.managed_runtime.lock().await;
        *guard = Some(ManagedRuntime {
            child,
            port,
            model_id: settings.selected_model.clone(),
        });

        Ok(format!(
            "Startet lokal llama.cpp-runtime på http://127.0.0.1:{port}/v1"
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
            RuntimeStatus {
                offline_mode: true,
                local_only: true,
                selected_model: settings.selected_model.clone(),
                runtime_state: self.runtime_state_for(
                    inspection,
                    &settings.selected_model,
                    local_running,
                ),
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
                "Runtime-mappen er klargjort i {}, men den valgte modellpakken er ikke installert ennå. Legg inn ekte llama.cpp- og modellfiler eller bruk miljøvariablene for lokal test.",
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
                "Lokal runtime kjører ikke akkurat nå for {}",
                settings.selected_model
            );
        };

        if model_id != settings.selected_model {
            bail!("Lokal runtime kjører, men med en annen modell enn den valgte.")
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

        Ok(format!("Aktiverte {}", catalog.label))
    }

    async fn repair_runtime(&self, settings: &AppSettings) -> Result<String> {
        let _lifecycle = self.lifecycle_gate.lock().await;
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
    fn prepare_runtime_files_skips_missing_optional_model() {
        let resource_root = temp_dir("resources");
        let app_data_root = temp_dir("app");
        let engine_bytes = b"engine";
        let standard_bytes = b"standard";

        write_file(
            &resource_root.join("embedded-runtime/bin/llama-server"),
            engine_bytes,
        );
        write_file(
            &resource_root.join("embedded-runtime/models/qwen3-4b-instruct-q4_k_m.gguf"),
            standard_bytes,
        );

        let manifest = serde_json::json!({
            "engine": {
                "resource": "embedded-runtime/bin/llama-server",
                "target_name": "llama-server",
                "sha256": sha(engine_bytes),
                "placeholder": false
            },
            "models": [
                {
                    "id": DEFAULT_MODEL,
                    "label": "Standard",
                    "tier": "Standard",
                    "resource": "embedded-runtime/models/qwen3-4b-instruct-q4_k_m.gguf",
                    "filename": "qwen3-4b-instruct-q4_k_m.gguf",
                    "sha256": sha(standard_bytes),
                    "bundled": true,
                    "placeholder": false
                },
                {
                    "id": QUALITY_MODEL,
                    "label": "Quality",
                    "tier": "Bedre kvalitet",
                    "resource": "embedded-runtime/models/qwen3-8b-instruct-q4_k_m.gguf",
                    "filename": "qwen3-8b-instruct-q4_k_m.gguf",
                    "sha256": sha(b"optional"),
                    "bundled": false,
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

        assert_eq!(copied, 2);
        assert!(inspection.engine.installed);
        assert!(
            inspection
                .models
                .iter()
                .find(|(model, _)| model.id == DEFAULT_MODEL)
                .unwrap()
                .1
                .installed
        );
        assert!(
            !inspection
                .models
                .iter()
                .find(|(model, _)| model.id == QUALITY_MODEL)
                .unwrap()
                .1
                .installed
        );
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
