use serde_json::{Value, json};
use std::fs;
use stem_core::model::manifest::ModelPackage;

fn manifest() -> Value {
    json!({
        "schema_version": 1, "id": "htdemucs-test", "version": "1",
        "backend": "htdemucs-onnx-v1", "profile": "graph-internal-v1",
        "sample_rate": 44100, "channels": 2, "chunk_frames": 343980, "overlap_frames": 85995,
        "input": "mix", "output": "stems", "sources": [
            {"id":"drums","label":"Drums"}, {"id":"bass","label":"Bass"},
            {"id":"other","label":"Other"}, {"id":"vocals","label":"Vocals"}],
        "model": {"path":"model.onnx", "sha256":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"},
        "license": {"path":"LICENSE", "sha256":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"},
        "provenance": "local validation fixture", "tested_devices": ["cpu"]
    })
}

#[test]
fn manifest_contract_validation() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("model.onnx"), b"abc").unwrap();
    fs::write(dir.path().join("LICENSE"), b"abc").unwrap();
    let save = |value: &Value| {
        fs::write(
            dir.path().join("manifest.json"),
            serde_json::to_vec(value).unwrap(),
        )
        .unwrap()
    };
    let good = manifest();
    save(&good);
    let package = ModelPackage::load(dir.path()).unwrap();
    assert_eq!(
        package
            .capabilities()
            .stems()
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>(),
        ["drums", "bass", "other", "vocals"]
    );
    for (key, value) in [
        ("schema_version", json!(2)),
        ("backend", json!("unknown")),
        ("profile", json!("unknown")),
        ("channels", json!(3)),
        ("input", json!("wrong")),
        ("chunk_frames", json!(0)),
        ("overlap_frames", json!(343980)),
        ("provenance", json!("")),
    ] {
        let mut invalid = good.clone();
        invalid[key] = value;
        save(&invalid);
        assert!(ModelPackage::load(dir.path()).is_err(), "accepted {key}");
    }
    for path in [
        "../outside.onnx",
        "/tmp/outside.onnx",
        "missing.onnx",
        "C:\\model.onnx",
    ] {
        let mut invalid = good.clone();
        invalid["model"]["path"] = json!(path);
        save(&invalid);
        assert!(ModelPackage::load(dir.path()).is_err());
    }
    let mut invalid = good.clone();
    invalid["sources"][1]["id"] = json!("drums");
    save(&invalid);
    assert!(ModelPackage::load(dir.path()).is_err());
    save(&good);
    fs::write(dir.path().join("model.onnx"), b"corrupt").unwrap();
    assert!(ModelPackage::load(dir.path()).is_err());
}

#[cfg(unix)]
#[test]
fn package_rejects_symlink_assets_outside_root() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("model.onnx"), b"abc").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("model.onnx"),
        dir.path().join("model.onnx"),
    )
    .unwrap();
    fs::write(dir.path().join("LICENSE"), b"abc").unwrap();
    fs::write(
        dir.path().join("manifest.json"),
        serde_json::to_vec(&manifest()).unwrap(),
    )
    .unwrap();
    assert!(ModelPackage::load(dir.path()).is_err());
}

#[test]
fn specialist_package_is_strict_and_canonical() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("model.onnx"), b"abc").unwrap();
    fs::write(dir.path().join("LICENSE"), b"abc").unwrap();
    let mut good = manifest();
    good["schema_version"] = json!(2);
    good["backend"] = json!("htdemucs-ft-bag-v1");
    good["profile"] = json!("graph-internal-ft-v1");
    let model = good.as_object_mut().unwrap().remove("model").unwrap();
    good["specialists"] = json!(
        ["drums", "bass", "other", "vocals"]
            .iter()
            .enumerate()
            .map(|(row, id)| json!({"source_id":id,"output_row":row,"model":model}))
            .collect::<Vec<_>>()
    );
    let save = |v: &Value| {
        fs::write(
            dir.path().join("manifest.json"),
            serde_json::to_vec(v).unwrap(),
        )
        .unwrap()
    };
    save(&good);
    assert!(
        ModelPackage::load(dir.path()).is_ok(),
        "valid specialist package rejected"
    );
    for mode in 0..5 {
        let mut bad = good.clone();
        match mode {
            0 => {
                bad["specialists"].as_array_mut().unwrap().pop();
            }
            1 => bad["specialists"][1]["source_id"] = json!("drums"),
            2 => bad["specialists"][0]["output_row"] = json!(3),
            3 => bad["specialists"][0]["model"]["path"] = json!("../escape"),
            _ => bad["model"] = model.clone(),
        }
        save(&bad);
        assert!(
            ModelPackage::load(dir.path()).is_err(),
            "accepted invalid mapping {mode}"
        );
    }
}
