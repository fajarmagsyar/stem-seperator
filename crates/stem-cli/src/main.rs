use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};
use stem_core::parity::compare_f32;
use stem_core::{
    model::{SeparationBackend, onnx::OnnxBackend},
    types::AudioChunk,
};

const HELP: &str = "Offline CPU stem separation and model validation.

Usage:
  stem-cli compare-reference ACTUAL.f32 REFERENCE.f32 --samples N --max-abs T
  stem-cli infer-chunk --model MODEL.onnx --runtime LIBRARY --input INPUT.f32 --output OUTPUT.f32 [--threads N]
  stem-cli separate --package DIRECTORY --runtime LIBRARY --input SONG --output DIRECTORY [--device auto|cpu|nvidia]
  stem-cli validate-model --package DIRECTORY --runtime LIBRARY --fixture INPUT.f32 --reference OUTPUT.f32 --max-abs T
  stem-cli --help

Inputs are raw little-endian float32 tensors in identical flattened order.
Specify the expected tensor sample count and an explicit finite tolerance.
Exit codes: 0 succeeded, 1 processing or comparison failed, 2 invalid command.";

fn model_command(command: &str, args: &[OsString]) -> ExitCode {
    let parsed = (|| -> Result<std::collections::HashMap<&str, &OsString>, String> {
        let allowed = if command == "separate" {
            vec!["--package", "--runtime", "--input", "--output", "--device"]
        } else {
            vec![
                "--package",
                "--runtime",
                "--fixture",
                "--reference",
                "--max-abs",
            ]
        };
        let mut values = std::collections::HashMap::new();
        let mut iter = args.iter();
        while let Some(key) = iter.next() {
            let key = key.to_str().ok_or("Invalid option")?;
            let value = iter.next().ok_or("Each option requires a value")?;
            if !allowed.contains(&key) || values.insert(key, value).is_some() {
                return Err(format!("Unknown or repeated option: {key}"));
            }
        }
        if allowed
            .iter()
            .filter(|key| **key != "--device")
            .any(|key| !values.contains_key(key))
        {
            return Err("Missing required options".into());
        }
        if let Some(device) = values.get("--device") {
            device
                .to_str()
                .ok_or("Invalid device")?
                .parse::<stem_core::model::device::ProcessingDevice>()
                .map_err(|e| e.to_string())?;
        }
        if command == "validate-model" {
            let tolerance = values["--max-abs"]
                .to_str()
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v >= 0.0);
            if tolerance.is_none() {
                return Err("Tolerance must be finite and nonnegative".into());
            }
        }
        Ok(values)
    })();
    let values = match parsed {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}\n{HELP}");
            return ExitCode::from(2);
        }
    };
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let package = stem_core::model::ModelPackage::load(Path::new(values["--package"]))?;
        let options = stem_core::model::device::BackendOptions {
            device: values
                .get("--device")
                .map(|s| s.to_str().unwrap().parse())
                .transpose()?
                .unwrap_or_default(),
            ..Default::default()
        };
        if command == "separate" {
            let cache = tempfile::tempdir()?;
            let cancel = stem_core::job::CancellationToken::default();
            eprintln!("Decoding...");
            let input = stem_core::audio::decode_to_cache(
                Path::new(values["--input"]),
                cache.path(),
                &cancel,
            )?;
            let mut progress = |event| {
                if let stem_core::job::JobEvent::BackendSelected(report) = &event {
                    eprintln!(
                        "Backend: {:?}; fallback: {:?}",
                        report.active, report.fallback_reason
                    );
                }
                if let stem_core::job::JobEvent::Progress {
                    phase,
                    completed,
                    total,
                } = event
                {
                    eprintln!("{phase}: {completed}/{total}");
                }
            };
            let mut result = stem_core::pipeline::package::separate_package(
                &input,
                &package,
                Path::new(values["--runtime"]),
                &options,
                cache.path(),
                &cancel,
                &mut progress,
            )?;
            result.model_id = package.manifest().id.clone();
            let request = stem_core::export::ExportRequest {
                output_root: PathBuf::from(values["--output"]),
                stem_ids: result.stems.iter().map(|s| s.id.clone()).collect(),
                mix: None,
            };
            let output = stem_core::export::export(&result, &request, &cancel, &mut progress)?;
            println!(
                "Exported {} aligned stems to {}",
                result.stems.len(),
                output.display()
            );
        } else {
            let mut backend =
                stem_core::model::open_backend(&package, Path::new(values["--runtime"]))?;
            let read =
                |path: &OsString, count: usize| -> Result<Vec<f32>, Box<dyn std::error::Error>> {
                    use std::io::Read;
                    let file = fs::File::open(path)?;
                    if file.metadata()?.len() != count as u64 * 4 {
                        return Err(format!("Tensor must contain exactly {count} samples").into());
                    }
                    let mut bytes = Vec::with_capacity(count * 4);
                    file.take(count as u64 * 4 + 1).read_to_end(&mut bytes)?;
                    if bytes.len() != count * 4 {
                        return Err("Tensor length changed during reading".into());
                    }
                    let data = bytes
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|b| f32::from_le_bytes(*b))
                        .collect::<Vec<_>>();
                    if data.iter().any(|v| !v.is_finite()) {
                        return Err("Tensor contains non-finite samples".into());
                    }
                    Ok(data)
                };
            let caps = backend.capabilities();
            let frames = caps.chunk_frames();
            let count = frames * caps.channels() as usize;
            let reference = read(values["--reference"], count * caps.stems().len())?;
            let input = read(values["--fixture"], count)?;
            let input = AudioChunk::new(input.chunks_exact(frames).map(|c| c.to_vec()).collect())?;
            let output = backend.infer(&input)?;
            let tolerance = values["--max-abs"].to_str().unwrap().parse::<f64>()?;
            let mut failed = false;
            for ((id, audio), reference) in output.stems().iter().zip(reference.chunks_exact(count))
            {
                let mut max = 0.0_f64;
                let mut sum = 0.0_f64;
                for (&actual, &expected) in audio.channels().iter().flatten().zip(reference) {
                    let difference = f64::from(actual) - f64::from(expected);
                    max = max.max(difference.abs());
                    sum += difference * difference;
                }
                println!(
                    "{} max_abs={} rmse={}",
                    id.as_str(),
                    max,
                    (sum / count as f64).sqrt()
                );
                failed |= max > tolerance;
            }
            if failed {
                return Err("Model exceeds the reference tolerance".into());
            }
        }
        Ok(())
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

struct InferenceRequest {
    model: PathBuf,
    runtime: PathBuf,
    input: PathBuf,
    output: PathBuf,
    threads: usize,
}

fn inference_request(args: &[OsString]) -> Result<InferenceRequest, String> {
    let mut paths = std::collections::HashMap::new();
    let mut threads = None;
    let mut options = args.iter();
    while let Some(option) = options.next() {
        let name = option.to_str().ok_or("Invalid option")?;
        let value = options.next().ok_or("Each option needs a value")?;
        if name == "--threads" && threads.is_none() {
            let count = value
                .to_str()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|v| *v > 0)
                .ok_or("CPU thread count must be nonzero")?;
            threads = Some(count);
        } else if matches!(name, "--model" | "--runtime" | "--input" | "--output")
            && !paths.contains_key(name)
        {
            paths.insert(name, PathBuf::from(value));
        } else {
            return Err(format!("Unknown or repeated option: {name}"));
        }
    }
    let mut path = |key| paths.remove(key).ok_or_else(|| format!("Missing {key}"));
    Ok(InferenceRequest {
        model: path("--model")?,
        runtime: path("--runtime")?,
        input: path("--input")?,
        output: path("--output")?,
        threads: threads.unwrap_or(2),
    })
}

fn infer_chunk(request: InferenceRequest) -> Result<(), Box<dyn std::error::Error>> {
    let expected_bytes = 687960 * 4;
    let mut file = fs::File::open(&request.input)?;
    if file.metadata()?.len() != expected_bytes {
        return Err(
            "Input must contain exactly 687960 little-endian float32 samples (two planar channels)"
                .into(),
        );
    }
    use std::io::Read;
    let mut bytes = Vec::with_capacity(expected_bytes as usize);
    (&mut file)
        .take(expected_bytes + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() != expected_bytes as usize {
        return Err("Input changed during read".into());
    }
    let samples: Vec<f32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|v| f32::from_le_bytes(*v))
        .collect();
    let input = AudioChunk::new(
        samples
            .as_chunks::<343980>()
            .0
            .iter()
            .map(|v| v.to_vec())
            .collect(),
    )?;
    if request.output.try_exists()? {
        return Err("Output already exists; choose a fresh filename".into());
    }
    let mut backend =
        OnnxBackend::load_development(&request.model, &request.runtime, request.threads)?;
    let started = std::time::Instant::now();
    let output = backend.infer(&input)?;
    let mut writer = BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&request.output)?,
    );
    let write_result = (|| -> std::io::Result<()> {
        for (_, audio) in output.stems() {
            for sample in audio.channels().iter().flatten() {
                writer.write_all(&sample.to_le_bytes())?;
            }
        }
        writer.flush()?;
        writer.get_ref().sync_all()
    })();
    drop(writer);
    if let Err(error) = write_result {
        let _ = fs::remove_file(&request.output);
        return Err(error.into());
    }
    println!(
        "provider=CPU samples=4127760 inference_and_write_seconds={:.6}",
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

fn parameters(args: &[OsString]) -> Result<(u64, f64), String> {
    let mut samples = None;
    let mut tolerance = None;
    let mut options = args.iter();
    while let Some(option) = options.next() {
        let value = options
            .next()
            .and_then(|v| v.to_str())
            .ok_or("Each option needs a value")?;
        if option == "--samples" && samples.is_none() {
            let count = value.parse::<u64>().map_err(|_| "Invalid sample count")?;
            if count == 0 {
                return Err("Sample count must be nonzero".into());
            }
            samples = Some(count);
        } else if option == "--max-abs" && tolerance.is_none() {
            let maximum = value.parse::<f64>().map_err(|_| "Invalid tolerance")?;
            if !maximum.is_finite() || maximum < 0.0 {
                return Err("Tolerance must be finite and nonnegative".into());
            }
            tolerance = Some(maximum);
        } else {
            return Err(format!(
                "Unknown or repeated option: {}",
                option.to_string_lossy()
            ));
        }
    }
    Ok((
        samples.ok_or("Missing --samples")?,
        tolerance.ok_or("Missing --max-abs")?,
    ))
}

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!("{HELP}");
        return ExitCode::SUCCESS;
    }
    if let Some(command) = args
        .first()
        .and_then(|v| v.to_str())
        .filter(|c| matches!(*c, "separate" | "validate-model"))
    {
        return model_command(command, &args[1..]);
    }
    if args.first().is_some_and(|command| command == "infer-chunk") {
        return match inference_request(&args[1..]) {
            Err(error) => {
                eprintln!("{error}\n\n{HELP}");
                ExitCode::from(2)
            }
            Ok(request) => match infer_chunk(request) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::FAILURE
                }
            },
        };
    }
    if args.len() < 3 || args[0] != "compare-reference" {
        eprintln!("{HELP}");
        return ExitCode::from(2);
    }
    let (samples, tolerance) = match parameters(&args[3..]) {
        Ok(values) => values,
        Err(error) => {
            eprintln!("{error}\n\n{HELP}");
            return ExitCode::from(2);
        }
    };
    match compare_f32(Path::new(&args[1]), Path::new(&args[2]), samples, tolerance) {
        Ok(metrics) => {
            println!(
                "samples={} max_abs={} rmse={}",
                metrics.samples, metrics.max_abs, metrics.rmse
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
