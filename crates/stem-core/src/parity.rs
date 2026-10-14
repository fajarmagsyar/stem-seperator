use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

#[derive(Debug)]
pub struct ParityMetrics {
    pub samples: u64,
    pub max_abs: f64,
    pub rmse: f64,
}

pub fn compare_f32(
    actual: &Path,
    reference: &Path,
    expected_samples: u64,
    max_abs: f64,
) -> Result<ParityMetrics, String> {
    if expected_samples == 0 || !max_abs.is_finite() || max_abs < 0.0 {
        return Err("Provide a nonzero sample count and a finite nonnegative tolerance".into());
    }
    let bytes = expected_samples
        .checked_mul(4)
        .ok_or("Sample count is too large")?;
    let open = |path: &Path| -> Result<BufReader<File>, String> {
        let file =
            File::open(path).map_err(|error| format!("Cannot open {}: {error}", path.display()))?;
        let metadata = file.metadata().map_err(|error| error.to_string())?;
        if !metadata.is_file() || metadata.len() != bytes {
            return Err(format!(
                "{} must contain exactly {expected_samples} little-endian float32 samples ({bytes} bytes)",
                path.display()
            ));
        }
        Ok(BufReader::new(file))
    };
    let mut actual = open(actual)?;
    let mut reference = open(reference)?;
    let mut metrics = ParityMetrics {
        samples: expected_samples,
        max_abs: 0.0,
        rmse: 0.0,
    };
    let mut squared_error = 0.0;
    for sample in 0..expected_samples {
        let mut a = [0_u8; 4];
        let mut r = [0_u8; 4];
        actual
            .read_exact(&mut a)
            .map_err(|error| format!("Reading actual sample {sample}: {error}"))?;
        reference
            .read_exact(&mut r)
            .map_err(|error| format!("Reading reference sample {sample}: {error}"))?;
        let a = f32::from_le_bytes(a);
        let r = f32::from_le_bytes(r);
        if !a.is_finite() || !r.is_finite() {
            return Err(format!(
                "Non-finite actual or reference value at sample {sample}"
            ));
        }
        let difference = f64::from(a) - f64::from(r);
        metrics.max_abs = metrics.max_abs.max(difference.abs());
        squared_error += difference * difference;
    }
    let mut trailing = [0_u8; 1];
    if actual
        .read(&mut trailing)
        .map_err(|error| error.to_string())?
        != 0
        || reference
            .read(&mut trailing)
            .map_err(|error| error.to_string())?
            != 0
    {
        return Err("Output length changed during comparison".into());
    }
    metrics.rmse = (squared_error / expected_samples as f64).sqrt();
    if metrics.max_abs > max_abs {
        return Err(format!(
            "Reference parity failed: max absolute error {} exceeds {}; RMSE {}",
            metrics.max_abs, max_abs, metrics.rmse
        ));
    }
    Ok(metrics)
}
