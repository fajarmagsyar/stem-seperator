use super::ModelCapabilities;
use crate::{StemError, types::StemId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufReader, Read},
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub id: String,
    pub version: String,
    pub backend: String,
    pub profile: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub chunk_frames: usize,
    pub overlap_frames: usize,
    pub input: String,
    pub output: String,
    pub sources: Vec<Source>,
    pub model: Asset,
    pub license: Asset,
    pub provenance: String,
    pub tested_devices: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Specialist {
    pub source_id: String,
    pub output_row: usize,
    pub model: Asset,
}
#[derive(Debug, Clone)]
pub struct ValidatedSpecialist {
    pub source_id: StemId,
    pub output_row: usize,
    pub model_path: PathBuf,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecialistManifest {
    pub schema_version: u32,
    pub id: String,
    pub version: String,
    pub backend: String,
    pub profile: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub chunk_frames: usize,
    pub overlap_frames: usize,
    pub input: String,
    pub output: String,
    pub sources: Vec<Source>,
    pub specialists: Vec<Specialist>,
    pub license: Asset,
    pub provenance: String,
    pub tested_devices: Vec<String>,
}

#[derive(Debug)]
pub struct ModelPackage {
    root: PathBuf,
    manifest: Manifest,
    model: PathBuf,
    capabilities: ModelCapabilities,
    specialists: Vec<ValidatedSpecialist>,
}

fn invalid(message: impl Into<String>) -> StemError {
    StemError::InvalidPackage(message.into())
}

pub fn sha256(path: &Path) -> Result<String, StemError> {
    let mut file = BufReader::new(File::open(path)?);
    let mut hash = Sha256::new();
    let mut bytes = [0; 65536];
    loop {
        let count = file.read(&mut bytes)?;
        if count == 0 {
            break;
        }
        hash.update(&bytes[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn asset(root: &Path, value: &Asset) -> Result<PathBuf, StemError> {
    let path = Path::new(&value.path);
    if value.path.is_empty()
        || value.path.contains(['\\', ':'])
        || path
            .components()
            .any(|p| !matches!(p, Component::Normal(_)))
    {
        return Err(invalid(
            "Package asset path must be a relative path inside the package",
        ));
    }
    let resolved = root
        .join(path)
        .canonicalize()
        .map_err(|e| invalid(format!("Missing asset {}: {e}", value.path)))?;
    if !resolved.starts_with(root) || !resolved.is_file() {
        return Err(invalid("Package asset escapes the package directory"));
    }
    if value.sha256.len() != 64
        || !value
            .sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || sha256(&resolved)? != value.sha256
    {
        return Err(invalid(format!("SHA-256 mismatch for {}", value.path)));
    }
    Ok(resolved)
}

impl ModelPackage {
    pub fn load(path: &Path) -> Result<Self, StemError> {
        let root = path.canonicalize()?;
        let manifest_path = root.join("manifest.json").canonicalize()?;
        if !manifest_path.starts_with(&root) {
            return Err(invalid("Manifest escapes package directory"));
        }
        let file = File::open(manifest_path)?;
        if file.metadata()?.len() > 1024 * 1024 {
            return Err(invalid("Manifest exceeds 1 MiB"));
        }
        let value: serde_json::Value =
            serde_json::from_reader(file).map_err(|e| invalid(format!("Invalid manifest: {e}")))?;
        let mut specialists = Vec::new();
        let manifest: Manifest = match value.get("schema_version").and_then(|v| v.as_u64()) {
            Some(1) => serde_json::from_value(value)
                .map_err(|e| invalid(format!("Invalid manifest: {e}")))?,
            Some(2) => {
                let bag: SpecialistManifest = serde_json::from_value(value)
                    .map_err(|e| invalid(format!("Invalid specialist manifest: {e}")))?;
                if bag.backend != "htdemucs-ft-bag-v1"
                    || bag.profile != "graph-internal-ft-v1"
                    || bag.specialists.len() != 4
                    || bag.sources.len() != 4
                {
                    return Err(invalid("Invalid specialist profile or source count"));
                }
                for (row, expected) in ["drums", "bass", "other", "vocals"].iter().enumerate() {
                    let specialist = &bag.specialists[row];
                    if specialist.source_id != *expected
                        || specialist.output_row != row
                        || bag.sources[row].id != *expected
                    {
                        return Err(invalid(
                            "Specialists must use canonical drums/bass/other/vocals mapping",
                        ));
                    }
                    specialists.push(ValidatedSpecialist {
                        source_id: StemId::new(*expected)?,
                        output_row: row,
                        model_path: asset(&root, &specialist.model)?,
                    });
                }
                let mut common = serde_json::to_value(&bag).map_err(|e| invalid(e.to_string()))?;
                common.as_object_mut().unwrap().remove("specialists");
                common["model"] = serde_json::to_value(&bag.specialists[0].model)
                    .map_err(|e| invalid(e.to_string()))?;
                serde_json::from_value(common).map_err(|e| invalid(e.to_string()))?
            }
            _ => return Err(invalid("Unsupported package schema")),
        };
        StemId::new(manifest.id.clone())?;
        if manifest.version.trim().is_empty()
            || (manifest.schema_version == 1
                && (manifest.backend != "htdemucs-onnx-v1"
                    || manifest.profile != "graph-internal-v1"))
        {
            return Err(invalid(
                "Unsupported package schema, backend or preprocessing profile",
            ));
        }
        if manifest.sample_rate != 44100
            || manifest.channels != 2
            || manifest.chunk_frames != 343980
            || manifest.overlap_frames != 85995
            || manifest.input != "mix"
            || manifest.output != "stems"
            || ![4, 6].contains(&manifest.sources.len())
        {
            return Err(invalid("Unsupported HTDemucs audio or tensor contract"));
        }
        if manifest.provenance.trim().is_empty()
            || !manifest.tested_devices.iter().any(|d| d == "cpu")
        {
            return Err(invalid(
                "Package requires provenance and a tested CPU profile",
            ));
        }
        let capabilities = ModelCapabilities::new(
            manifest.sample_rate,
            manifest.channels,
            manifest.chunk_frames,
            manifest
                .sources
                .iter()
                .map(|s| Ok((StemId::new(s.id.clone())?, s.label.clone())))
                .collect::<Result<_, crate::types::ContractError>>()?,
        )?;
        let model = asset(&root, &manifest.model)?;
        asset(&root, &manifest.license)?;
        Ok(Self {
            root,
            manifest,
            model,
            capabilities,
            specialists,
        })
    }
    pub fn specialists(&self) -> &[ValidatedSpecialist] {
        &self.specialists
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn model_path(&self) -> &Path {
        &self.model
    }
    pub fn capabilities(&self) -> &ModelCapabilities {
        &self.capabilities
    }
}
