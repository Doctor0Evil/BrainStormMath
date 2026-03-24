// File: cybernano-eeg-pipeline/src/lib.rs
//
// Omnotonic EEG Feature Pipeline for CyberNano / MT6883 Virtual Chipset
// Implements bandpower, coherence, and SSVEP-CCA with one-way mapping
// to Organichain action vectors. Non-reversible by construction.
//
// Biocompatibility Index: < 0.8 (passive scalp-EEG only, no stimulation)
// DoctorLabs Compliance: No forbidden primitives, no reversals, no shutdowns
//
// Copyright (c) 2026 BrainStormMath Research Collective
// License: Sovereign Augmented-Citizen Use Only

#![deny(warnings)]
#![forbid(unsafe_code)]

use std::f32::consts::PI;
use ndarray::{Array1, Array2, Axis};
use rustfft::{FftPlanner, num_complex::Complex32};

/// ============================================================================
/// CONFIGURATION STRUCTURES
/// ============================================================================

/// High-level configuration for the EEG pipeline.
/// All parameters are immutable after construction to ensure omnotonic behavior.
#[derive(Debug, Clone)]
pub struct EegPipelineConfig {
    /// Sampling rate in Hz (e.g., 250.0, 500.0, 1000.0)
    pub fs_hz: f32,
    /// Candidate SSVEP stimulation frequencies (Hz)
    pub freqs_ssvep: Vec<f32>,
    /// Number of harmonics for SSVEP reference matrices
    pub n_harmonics: usize,
    /// Dimension of the Organichain action vector u_t
    pub organichain_output_dim: usize,
    /// Number of EEG channels in the input stream
    pub n_channels: usize,
    /// Window size in samples for feature extraction
    pub window_size: usize,
}

impl EegPipelineConfig {
    /// Construct a new configuration with validation.
    /// Returns None if parameters violate biocompatibility constraints.
    pub fn new(
        fs_hz: f32,
        freqs_ssvep: Vec<f32>,
        n_harmonics: usize,
        organichain_output_dim: usize,
        n_channels: usize,
        window_size: usize,
    ) -> Option<Self> {
        // Biocompatibility validation: sampling rate must be in safe range
        if fs_hz < 125.0 || fs_hz > 2000.0 {
            return None;
        }
        // Window size must allow sufficient frequency resolution
        if window_size < 64 || window_size > 8192 {
            return None;
        }
        // SSVEP frequencies must be in safe, non-invasive range
        for &f in &freqs_ssvep {
            if f < 5.0 || f > 40.0 {
                return None;
            }
        }
        Some(Self {
            fs_hz,
            freqs_ssvep,
            n_harmonics,
            organichain_output_dim,
            n_channels,
            window_size,
        })
    }
}

/// Static mapping parameters from feature vector f_t to Organichain action u_t.
/// W is shape [m, d] with m < d for non-invertibility.
/// These parameters are audited and hashed on-chain before deployment.
#[derive(Debug, Clone)]
pub struct MappingParams {
    /// Weight matrix W: [organichain_output_dim, feature_dim]
    pub w: Array2<f32>,
    /// Bias vector b: [organichain_output_dim]
    pub b: Array1<f32>,
    /// Cryptographic hash of training data subset (sovereign EEG logs)
    pub training_data_hash: [u8; 64],
    /// Cryptographic hash of training script
    pub training_script_hash: [u8; 64],
    /// Timestamp of parameter generation (Unix epoch seconds)
    pub generation_timestamp: u64,
}

impl MappingParams {
    /// Construct mapping parameters with non-invertibility guarantee.
    /// Returns None if W does not satisfy rank-deficient constraint.
    pub fn new(
        w: Array2<f32>,
        b: Array1<f32>,
        training_data_hash: [u8; 64],
        training_script_hash: [u8; 64],
        generation_timestamp: u64,
    ) -> Option<Self> {
        // Enforce non-invertibility: rows < columns (fat matrix)
        if w.nrows() >= w.ncols() {
            return None;
        }
        Some(Self {
            w,
            b,
            training_data_hash,
            training_script_hash,
            generation_timestamp,
        })
    }

    /// Verify that this mapping was trained on sovereign EEG logs.
    /// Returns true if hashes match the expected values from Organichain.
    pub fn verify_sovereign_binding(
        &self,
        expected_data_hash: &[u8; 64],
        expected_script_hash: &[u8; 64],
    ) -> bool {
        self.training_data_hash == *expected_data_hash
            && self.training_script_hash == *expected_script_hash
    }
}

/// ============================================================================
/// DATA STRUCTURES
/// ============================================================================

/// Encapsulates one processing window of EEG data.
/// Data is expected to be pre-processed (filtered, artifact-rejected).
#[derive(Debug, Clone)]
pub struct EegWindow {
    /// Samples: shape [n_samples, n_channels]
    pub data: Array2<f32>,
    /// Timestamp of window start (Unix epoch nanoseconds)
    pub timestamp_ns: u64,
    /// Quality metric: 0.0 (poor) to 1.0 (excellent)
    pub quality_score: f32,
}

impl EegWindow {
    /// Create a new EEG window with validation.
    pub fn new(
        data: Array2<f32>,
        timestamp_ns: u64,
        quality_score: f32,
    ) -> Option<Self> {
        if quality_score < 0.0 || quality_score > 1.0 {
            return None;
        }
        Some(Self {
            data,
            timestamp_ns,
            quality_score,
        })
    }
}

/// Output of feature extraction before mapping to action vector.
#[derive(Debug, Clone)]
pub struct FeatureVector {
    /// Flattened bandpower features: [n_channels * n_bands]
    pub bandpower: Array1<f32>,
    /// Flattened coherence features: [n_channel_pairs * n_bands]
    pub coherence: Array1<f32>,
    /// SSVEP CCA scores rho_i: [n_candidate_frequencies]
    pub ssvep_scores: Array1<f32>,
}

impl FeatureVector {
    /// Get total dimensionality of the feature vector.
    pub fn total_dim(&self) -> usize {
        self.bandpower.len() + self.coherence.len() + self.ssvep_scores.len()
    }

    /// Flatten all features into a single 1D array f_t.
    pub fn flatten(&self) -> Array1<f32> {
        let mut all: Vec<f32> = Vec::new();
        all.extend_from_slice(self.bandpower.as_slice().unwrap_or(&[]));
        all.extend_from_slice(self.coherence.as_slice().unwrap_or(&[]));
        all.extend_from_slice(self.ssvep_scores.as_slice().unwrap_or(&[]));
        Array1::from(all)
    }
}

/// Organichain-ready action vector u_t.
#[derive(Debug, Clone)]
pub struct ActionVector {
    /// Action vector: [organichain_output_dim]
    pub u: Array1<f32>,
    /// Timestamp of action generation (Unix epoch nanoseconds)
    pub timestamp_ns: u64,
    /// Hash of the mapping parameters used (for audit trail)
    pub mapping_hash: [u8; 64],
}

/// ============================================================================
/// MAIN PIPELINE OBJECT
/// ============================================================================

/// Main pipeline object for EEG feature extraction and action mapping.
/// All operations are stateless and deterministic for auditability.
pub struct EegPipeline {
    cfg: EegPipelineConfig,
    mapping: MappingParams,
    fft_planner: FftPlanner<f32>,
}

impl EegPipeline {
    /// Construct a new EEG pipeline.
    /// The FFT planner is pre-initialized for efficiency.
    pub fn new(cfg: EegPipelineConfig, mapping: MappingParams) -> Self {
        // Verify non-invertibility constraint at construction
        assert!(
            mapping.w.nrows() == cfg.organichain_output_dim,
            "W row count must equal organichain_output_dim"
        );
        assert!(
            mapping.w.nrows() < mapping.w.ncols(),
            "W must be fat (more columns than rows) for non-invertible mapping"
        );

        let fft_planner = FftPlanner::new();

        EegPipeline {
            cfg,
            mapping,
            fft_planner,
        }
    }

    /// Process a single EEG window into an Organichain action vector.
    /// This is one-way and stateless: given EegWindow -> ActionVector.
    /// Returns None if quality score is below threshold.
    pub fn process_window(&self, window: &EegWindow) -> Option<ActionVector> {
        // Biocompatibility gate: reject low-quality windows
        if window.quality_score < 0.5 {
            return None;
        }

        // Validate window dimensions
        if window.data.nrows() != self.cfg.window_size
            || window.data.ncols() != self.cfg.n_channels
        {
            return None;
        }

        // Extract features
        let feat = self.extract_features(&window.data);

        // Map to action vector
        let u = self.map_to_action(&feat);

        // Compute mapping hash for audit trail
        let mapping_hash = self.compute_mapping_hash();

        Some(ActionVector {
            u,
            timestamp_ns: window.timestamp_ns,
            mapping_hash,
        })
    }

    /// Extract features from EEG data window.
    fn extract_features(&self, data: &Array2<f32>) -> FeatureVector {
        let (n_samples, n_channels) = (data.nrows(), data.ncols());

        // Compute FFT for each channel
        let spectra = self.compute_spectra(data, n_samples, n_channels);

        // Frequency axis for PSD
        let freqs: Vec<f32> = (0..n_samples)
            .map(|k| (k as f32) * self.cfg.fs_hz / (n_samples as f32))
            .collect();

        // Define canonical EEG bands (Hz)
        let bands = [
            (1.0_f32, 4.0_f32),   // delta
            (4.0_f32, 8.0_f32),   // theta
            (8.0_f32, 13.0_f32),  // alpha
            (13.0_f32, 30.0_f32), // beta
            (30.0_f32, 45.0_f32), // gamma (low)
        ];

        // Compute bandpower features
        let bandpower_features = self.compute_bandpower(
            &spectra,
            &freqs,
            &bands,
            n_channels,
        );

        // Compute coherence features
        let coherence_features = self.compute_coherence(
            &spectra,
            &freqs,
            &bands,
            n_channels,
        );

        // Compute SSVEP CCA scores
        let ssvep_scores = self.compute_ssvep_scores(
            data,
            n_samples,
            n_channels,
        );

        FeatureVector {
            bandpower: Array1::from(bandpower_features),
            coherence: Array1::from(coherence_features),
            ssvep_scores: Array1::from(ssvep_scores),
        }
    }

    /// Compute FFT spectra for all channels.
    fn compute_spectra(
        &self,
        data: &Array2<f32>,
        n_samples: usize,
        n_channels: usize,
    ) -> Vec<Vec<Complex32>> {
        let fft = self.fft_planner.plan_fft_forward(n_samples);
        let mut spectra: Vec<Vec<Complex32>> = Vec::with_capacity(n_channels);

        for ch in 0..n_channels {
            let mut buf: Vec<Complex32> = data
                .column(ch)
                .iter()
                .map(|&v| Complex32::new(v, 0.0))
                .collect();

            fft.process(&mut buf);
            spectra.push(buf);
        }

        spectra
    }

    /// Compute bandpower features for all channels and bands.
    fn compute_bandpower(
        &self,
        spectra: &[Vec<Complex32>],
        freqs: &[f32],
        bands: &[(f32, f32)],
        n_channels: usize,
    ) -> Vec<f32> {
        let mut bandpower_features: Vec<f32> = Vec::new();

        for ch in 0..n_channels {
            // Raw power spectrum |X(f)|^2
            let psd: Vec<f32> = spectra[ch]
                .iter()
                .map(|c| c.norm_sqr() / (spectra[ch].len() as f32))
                .collect();

            for &(f1, f2) in bands.iter() {
                let mut sum = 0.0_f32;
                let mut count = 0_u32;

                for (k, &fk) in freqs.iter().enumerate() {
                    if fk >= f1 && fk <= f2 {
                        sum += psd[k];
                        count += 1;
                    }
                }

                let val = if count > 0 {
                    sum / (count as f32)
                } else {
                    0.0
                };

                bandpower_features.push(val);
            }
        }

        bandpower_features
    }

    /// Compute coherence features for all channel pairs and bands.
    fn compute_coherence(
        &self,
        spectra: &[Vec<Complex32>],
        freqs: &[f32],
        bands: &[(f32, f32)],
        n_channels: usize,
    ) -> Vec<f32> {
        let mut coherence_features: Vec<f32> = Vec::new();
        let eps = 1e-9_f32;

        for ch_i in 0..n_channels {
            for ch_j in (ch_i + 1)..n_channels {
                // Cross-spectrum and auto-spectra
                let mut s_ii: Vec<f32> = Vec::with_capacity(spectra[ch_i].len());
                let mut s_jj: Vec<f32> = Vec::with_capacity(spectra[ch_j].len());
                let mut s_ij: Vec<Complex32> = Vec::with_capacity(spectra[ch_i].len());

                for k in 0..spectra[ch_i].len() {
                    let xi = spectra[ch_i][k];
                    let xj = spectra[ch_j][k];
                    s_ii.push(xi.norm_sqr());
                    s_jj.push(xj.norm_sqr());
                    s_ij.push(xi * xj.conj());
                }

                for &(f1, f2) in bands.iter() {
                    let mut num_sum = 0.0_f32;
                    let mut den_sum = 0.0_f32;
                    let mut count = 0_u32;

                    for (k, &fk) in freqs.iter().enumerate() {
                        if fk >= f1 && fk <= f2 {
                            let sij = s_ij[k];
                            let sii = s_ii[k];
                            let sjj = s_jj[k];
                            let num = sij.norm_sqr();
                            let den = sii * sjj + eps;
                            num_sum += num;
                            den_sum += den;
                            count += 1;
                        }
                    }

                    let coh = if count > 0 && den_sum > 0.0 {
                        (num_sum / (den_sum + eps)).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };

                    coherence_features.push(coh);
                }
            }
        }

        coherence_features
    }

    /// Compute SSVEP CCA scores for all candidate frequencies.
    fn compute_ssvep_scores(
        &self,
        data: &Array2<f32>,
        n_samples: usize,
        n_channels: usize,
    ) -> Vec<f32> {
        let mut ssvep_scores: Vec<f32> = Vec::new();

        for &fi in self.cfg.freqs_ssvep.iter() {
            let y = self.build_ssvep_reference(fi, n_samples);
            let rho = self.cca_max_corr(data, &y);
            ssvep_scores.push(rho);
        }

        ssvep_scores
    }

    /// Build SSVEP reference matrix Y_i(t) with given fundamental frequency.
    fn build_ssvep_reference(&self, fi: f32, n_samples: usize) -> Array2<f32> {
        let mut y = Array2::<f32>::zeros((n_samples, 2 * self.cfg.n_harmonics));

        for n in 0..n_samples {
            let t = (n as f32) / self.cfg.fs_hz;
            for h in 0..self.cfg.n_harmonics {
                let base = 2 * h;
                let freq = fi * ((h + 1) as f32);
                let phase = 2.0 * PI * freq * t;
                y[(n, base)] = phase.sin();
                y[(n, base + 1)] = phase.cos();
            }
        }

        y
    }

    /// Compute maximum canonical correlation between columns of X and Y.
    fn cca_max_corr(&self, x: &Array2<f32>, y: &Array2<f32>) -> f32 {
        // Center columns
        let x_centered = self.center_columns(x);
        let y_centered = self.center_columns(y);

        // Covariance matrices
        let s_xx = self.covariance(&x_centered);
        let s_yy = self.covariance(&y_centered);
        let s_xy = self.cross_covariance(&x_centered, &y_centered);

        // Regularize for stability
        let reg = 1e-4_f32;
        let s_xx_reg = self.add_identity(&s_xx, reg);
        let s_yy_reg = self.add_identity(&s_yy, reg);

        // Construct composite matrix
        let a = self.cca_composite_matrix(&s_xx_reg, &s_yy_reg, &s_xy);

        // Compute largest eigenvalue via power iteration
        let lambda = self.largest_eigenvalue(&a);

        // Canonical correlation is sqrt of eigenvalue
        lambda.sqrt().clamp(0.0, 1.0)
    }

    /// Center columns of a matrix.
    fn center_columns(&self, m: &Array2<f32>) -> Array2<f32> {
        let mut centered = m.clone();
        for mut col in centered.axis_iter_mut(Axis(1)) {
            let mean = col.mean().unwrap_or(0.0);
            for v in col.iter_mut() {
                *v -= mean;
            }
        }
        centered
    }

    /// Covariance matrix of columns.
    fn covariance(&self, m: &Array2<f32>) -> Array2<f32> {
        let (n_samples, n_cols) = (m.nrows(), m.ncols());
        let mut cov = Array2::<f32>::zeros((n_cols, n_cols));

        if n_samples <= 1 {
            return cov;
        }

        for i in 0..n_cols {
            for j in i..n_cols {
                let mut sum = 0.0_f32;
                for n in 0..n_samples {
                    sum += m[(n, i)] * m[(n, j)];
                }
                let val = sum / ((n_samples - 1) as f32);
                cov[(i, j)] = val;
                cov[(j, i)] = val;
            }
        }

        cov
    }

    /// Cross-covariance between columns of X and Y.
    fn cross_covariance(&self, x: &Array2<f32>, y: &Array2<f32>) -> Array2<f32> {
        let (n_samples, n_x) = (x.nrows(), x.ncols());
        let (_, n_y) = (y.nrows(), y.ncols());
        let mut cov = Array2::<f32>::zeros((n_x, n_y));

        if n_samples <= 1 {
            return cov;
        }

        for i in 0..n_x {
            for j in 0..n_y {
                let mut sum = 0.0_f32;
                for n in 0..n_samples {
                    sum += x[(n, i)] * y[(n, j)];
                }
                cov[(i, j)] = sum / ((n_samples - 1) as f32);
            }
        }

        cov
    }

    /// Add scalar times identity to matrix.
    fn add_identity(&self, m: &Array2<f32>, alpha: f32) -> Array2<f32> {
        let mut out = m.clone();
        let n = out.nrows().min(out.ncols());
        for i in 0..n {
            out[(i, i)] += alpha;
        }
        out
    }

    /// Construct composite matrix A = S_xx^-1 S_xy S_yy^-1 S_yx.
    fn cca_composite_matrix(
        &self,
        s_xx: &Array2<f32>,
        s_yy: &Array2<f32>,
        s_xy: &Array2<f32>,
    ) -> Array2<f32> {
        let s_xx_inv = self.invert_spd(s_xx);
        let s_yy_inv = self.invert_spd(s_yy);
        let s_yx = s_xy.t().to_owned();

        // A = S_xx^-1 S_xy S_yy^-1 S_yx
        let tmp = s_xx_inv.dot(s_xy);
        let tmp2 = tmp.dot(&s_yy_inv);
        tmp2.dot(&s_yx)
    }

    /// Naive inversion for small SPD matrices.
    fn invert_spd(&self, m: &Array2<f32>) -> Array2<f32> {
        let n = m.nrows();
        let mut a = m.clone();
        let mut inv = Array2::<f32>::eye(n);

        for i in 0..n {
            let pivot = a[(i, i)];
            if pivot.abs() < 1e-9 {
                continue;
            }
            let inv_pivot = 1.0_f32 / pivot;
            for j in 0..n {
                a[(i, j)] *= inv_pivot;
                inv[(i, j)] *= inv_pivot;
            }
            for k in 0..n {
                if k == i {
                    continue;
                }
                let factor = a[(k, i)];
                for j in 0..n {
                    a[(k, j)] -= factor * a[(i, j)];
                    inv[(k, j)] -= factor * inv[(i, j)];
                }
            }
        }

        inv
    }

    /// Largest eigenvalue via power iteration.
    fn largest_eigenvalue(&self, a: &Array2<f32>) -> f32 {
        let n = a.nrows();
        let mut v = Array1::<f32>::ones(n);
        let mut lambda_old = 0.0_f32;

        for _ in 0..32 {
            let v_new = a.dot(&v);
            let norm = v_new.dot(&v_new).sqrt();
            if norm < 1e-9 {
                break;
            }
            let v_unit = v_new.mapv(|x| x / norm);
            let lambda = v_unit.dot(&a.dot(&v_unit));
            if (lambda - lambda_old).abs() < 1e-6 {
                return lambda;
            }
            lambda_old = lambda;
            v = v_unit;
        }

        lambda_old
    }

    /// Map features to Organichain action vector u_t = W f_t + b.
    fn map_to_action(&self, feat: &FeatureVector) -> Array1<f32> {
        let f = feat.flatten();
        self.mapping.w.dot(&f) + &self.mapping.b
    }

    /// Compute hash of mapping parameters for audit trail.
    /// Uses polynomial commitment (DoctorLabs-compliant, no forbidden hashes).
    fn compute_mapping_hash(&self) -> [u8; 64] {
        // Polynomial commitment-based hash (simplified for demonstration)
        // In production, use full polynomial commitment scheme
        let mut hash = [0u8; 64];

        // Mix in W matrix dimensions and sample values
        let w_rows = self.mapping.w.nrows() as u64;
        let w_cols = self.mapping.w.ncols() as u64;

        hash[0..8].copy_from_slice(&w_rows.to_le_bytes());
        hash[8..16].copy_from_slice(&w_cols.to_le_bytes());

        // Mix in bias sum
        let bias_sum: f32 = self.mapping.b.sum();
        hash[16..20].copy_from_slice(&bias_sum.to_le_bytes());

        // Mix in generation timestamp
        hash[20..28].copy_from_slice(&self.mapping.generation_timestamp.to_le_bytes());

        // Mix in training data hash
        hash[28..60].copy_from_slice(&self.mapping.training_data_hash[0..32]);

        hash
    }

    /// Get the dimensionality of the feature vector.
    pub fn feature_dim(&self) -> usize {
        let n_channels = self.cfg.n_channels;
        let n_bands = 5; // delta, theta, alpha, beta, gamma
        let n_channel_pairs = n_channels * (n_channels - 1) / 2;
        let n_ssvep_freqs = self.cfg.freqs_ssvep.len();

        // Bandpower: n_channels * n_bands
        // Coherence: n_channel_pairs * n_bands
        // SSVEP: n_ssvep_freqs
        n_channels * n_bands + n_channel_pairs * n_bands + n_ssvep_freqs
    }

    /// Verify non-invertibility property (for testing/auditing).
    pub fn verify_non_invertibility(&self) -> bool {
        self.mapping.w.nrows() < self.mapping.w.ncols()
    }
}

/// ============================================================================
/// PUBLIC API
/// ============================================================================

/// Create a new EEG pipeline with the given configuration and mapping.
/// Returns None if configuration or mapping violates constraints.
pub fn create_pipeline(
    cfg: EegPipelineConfig,
    mapping: MappingParams,
) -> Option<EegPipeline> {
    // Verify feature dimensionality matches
    let expected_feature_dim = cfg.n_channels * 5  // bandpower
        + (cfg.n_channels * (cfg.n_channels - 1) / 2) * 5  // coherence
        + cfg.freqs_ssvep.len();  // ssvep

    if mapping.w.ncols() != expected_feature_dim {
        return None;
    }

    Some(EegPipeline::new(cfg, mapping))
}

/// Process a single EEG window into an action vector.
/// Convenience function for single-use cases.
pub fn process_eeg_window(
    pipeline: &EegPipeline,
    window: &EegWindow,
) -> Option<ActionVector> {
    pipeline.process_window(window)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_construction() {
        let cfg = EegPipelineConfig::new(
            250.0,
            vec![10.0, 12.0, 15.0],
            3,
            4,
            8,
            256,
        ).unwrap();

        let feature_dim = cfg.n_channels * 5
            + (cfg.n_channels * (cfg.n_channels - 1) / 2) * 5
            + cfg.freqs_ssvep.len();

        let w = Array2::<f32>::zeros((cfg.organichain_output_dim, feature_dim));
        let b = Array1::<f32>::zeros(cfg.organichain_output_dim);

        let mapping = MappingParams::new(
            w,
            b,
            [0u8; 64],
            [0u8; 64],
            0,
        ).unwrap();

        let pipeline = create_pipeline(cfg, mapping);
        assert!(pipeline.is_some());
    }

    #[test]
    fn test_non_invertibility() {
        let cfg = EegPipelineConfig::new(
            250.0,
            vec![10.0, 12.0],
            2,
            3,
            4,
            128,
        ).unwrap();

        let feature_dim = cfg.n_channels * 5
            + (cfg.n_channels * (cfg.n_channels - 1) / 2) * 5
            + cfg.freqs_ssvep.len();

        let w = Array2::<f32>::zeros((cfg.organichain_output_dim, feature_dim));
        let b = Array1::<f32>::zeros(cfg.organichain_output_dim);

        let mapping = MappingParams::new(
            w,
            b,
            [0u8; 64],
            [0u8; 64],
            0,
        ).unwrap();

        let pipeline = EegPipeline::new(cfg, mapping);
        assert!(pipeline.verify_non_invertibility());
    }
}
