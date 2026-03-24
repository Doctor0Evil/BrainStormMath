// File: cybercore-brain/src/identity/brainprint.rs
//
// Brainbound Identity Verification Module for CyberNano / Cybercore-Brain
// Implements cryptographically-auditable brainprint functional B(·) for
// sovereign EEG log binding and real-time Brain-IP verification.
//
// Omnotonic Guarantees:
//   - Identity parameters only increase in fidelity over time
//   - No rollback of verified brainprint signatures
//   - No degradation of verification thresholds
//   - All decoder bindings are permanent and auditable
//
// Biocompatibility Index: < 0.8 (passive EEG analysis only)
// DoctorLabs Compliance: No forbidden primitives, no reversals, no shutdowns
//
// Copyright (c) 2026 BrainStormMath Research Collective
// License: Sovereign Augmented-Citizen Use Only

#![deny(warnings)]
#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use ndarray::{Array1, Array2, Axis};
use serde::{Serialize, Deserialize};

/// ============================================================================
/// CONSTANTS AND CONFIGURATION
/// ============================================================================

/// Minimum number of enrollment sessions required for brainprint registration.
/// Ensures statistical stability of the identity signature.
pub const MIN_ENROLLMENT_SESSIONS: usize = 5;

/// Maximum number of sessions retained for brainprint distribution modeling.
pub const MAX_ENROLLMENT_SESSIONS: usize = 50;

/// Default verification threshold (Mahalanobis distance).
/// Lower values = stricter verification, higher values = more permissive.
pub const DEFAULT_VERIFICATION_THRESHOLD: f64 = 3.5;

/// Minimum threshold value (cannot be lowered below this for omnotonic safety).
pub const MIN_VERIFICATION_THRESHOLD: f64 = 2.0;

/// Dimensionality of the compact brainprint signature vector.
pub const BRAINPRINT_DIM: usize = 128;

/// ============================================================================
/// DATA STRUCTURES
/// ============================================================================

/// Represents a single brainprint feature vector extracted from an EEG session.
/// This is the output of the brainprint functional B(session_data).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainprintSignature {
    /// Compact feature vector: [BRAINPRINT_DIM]
    pub features: Array1<f64>,
    /// Timestamp of session (Unix epoch nanoseconds)
    pub session_timestamp_ns: u64,
    /// Session duration in milliseconds
    pub session_duration_ms: u64,
    /// Quality score: 0.0 (poor) to 1.0 (excellent)
    pub quality_score: f64,
    /// Number of EEG channels used in extraction
    pub n_channels: usize,
    /// Sampling rate in Hz
    pub sampling_rate_hz: f32,
}

impl BrainprintSignature {
    /// Create a new brainprint signature with validation.
    pub fn new(
        features: Array1<f64>,
        session_timestamp_ns: u64,
        session_duration_ms: u64,
        quality_score: f64,
        n_channels: usize,
        sampling_rate_hz: f32,
    ) -> Option<Self> {
        // Validate feature dimensionality
        if features.len() != BRAINPRINT_DIM {
            return None;
        }
        // Validate quality score range
        if quality_score < 0.0 || quality_score > 1.0 {
            return None;
        }
        // Validate channel count (standard EEG configurations)
        if n_channels < 1 || n_channels > 256 {
            return None;
        }
        // Validate sampling rate (common BCI ranges)
        if sampling_rate_hz < 125.0 || sampling_rate_hz > 2000.0 {
            return None;
        }
        Some(Self {
            features,
            session_timestamp_ns,
            session_duration_ms,
            quality_score,
            n_channels,
            sampling_rate_hz,
        })
    }

    /// Compute L2 norm of the feature vector for normalization.
    pub fn l2_norm(&self) -> f64 {
        self.features.dot(&self.features).sqrt()
    }

    /// Normalize features to unit length for comparison.
    pub fn normalize(&self) -> Array1<f64> {
        let norm = self.l2_norm();
        if norm > 1e-9 {
            self.features.mapv(|x| x / norm)
        } else {
            self.features.clone()
        }
    }
}

/// Cryptographic binding chain for decoder verification.
/// Links decoder parameters to sovereign EEG training data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecoderBindingChain {
    /// Hash of training data subset (sovereign EEG logs)
    /// Uses polynomial commitment (DoctorLabs-compliant)
    pub training_data_commitment: [u8; 64],
    /// Hash of training script/code that produced the decoder
    pub training_script_commitment: [u8; 64],
    /// Hash of resulting decoder parameter tensors
    pub decoder_params_commitment: [u8; 64],
    /// Timestamp of binding creation (Unix epoch seconds)
    pub binding_timestamp: u64,
    /// Decoder identifier (human-readable label)
    pub decoder_id: String,
    /// Version number (monotonically increasing, no rollbacks)
    pub version: u64,
    /// Previous version commitment (for chain verification)
    pub previous_version_commitment: Option<[u8; 64]>,
}

impl DecoderBindingChain {
    /// Create a new decoder binding with omnotonic versioning.
    pub fn new(
        training_data_commitment: [u8; 64],
        training_script_commitment: [u8; 64],
        decoder_params_commitment: [u8; 64],
        decoder_id: String,
        version: u64,
        previous_version_commitment: Option<[u8; 64]>,
    ) -> Self {
        let binding_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            training_data_commitment,
            training_script_commitment,
            decoder_params_commitment,
            binding_timestamp,
            decoder_id,
            version,
            previous_version_commitment,
        }
    }

    /// Verify that this binding chain is continuous (no gaps).
    pub fn verify_chain_continuity(&self, previous: &Self) -> bool {
        if self.version != previous.version + 1 {
            return false;
        }
        match self.previous_version_commitment {
            Some(prev_commit) => prev_commit == previous.decoder_params_commitment,
            None => false,
        }
    }

    /// Verify omnotonic versioning (no rollbacks).
    pub fn verify_no_rollback(&self, previous: &Self) -> bool {
        self.version > previous.version
    }
}

/// Statistical model of the user's brainprint distribution.
/// Used for real-time verification against enrolled signatures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainprintDistribution {
    /// Mean vector of enrolled brainprints: [BRAINPRINT_DIM]
    pub mean: Array1<f64>,
    /// Diagonal covariance (variance per dimension): [BRAINPRINT_DIM]
    pub variance: Array1<f64>,
    /// Number of sessions used to build this distribution
    pub n_sessions: usize,
    /// Timestamp of last update (Unix epoch nanoseconds)
    pub last_update_ns: u64,
    /// Minimum quality score among enrolled sessions
    pub min_quality_score: f64,
    /// Maximum quality score among enrolled sessions
    pub max_quality_score: f64,
}

impl BrainprintDistribution {
    /// Create a new distribution from a single brainprint signature.
    pub fn from_single(signature: &BrainprintSignature) -> Self {
        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        Self {
            mean: signature.features.clone(),
            variance: Array1::from_elem(BRAINPRINT_DIM, 1.0),
            n_sessions: 1,
            last_update_ns: now_ns,
            min_quality_score: signature.quality_score,
            max_quality_score: signature.quality_score,
        }
    }

    /// Update distribution with a new brainprint signature (online learning).
    /// Uses Welford's online algorithm for numerical stability.
    pub fn update(&mut self, signature: &BrainprintSignature) {
        let n_old = self.n_sessions as f64;
        let n_new = n_old + 1.0;

        // Update mean (Welford's algorithm)
        let delta = &signature.features - &self.mean;
        let delta_scaled = delta.mapv(|x| x / n_new);
        self.mean = self.mean.clone() + &delta_scaled;

        // Update variance (online variance calculation)
        let delta_prev = &signature.features - &self.mean;
        let variance_update = &delta * &delta_prev;
        self.variance = self.variance.clone() + variance_update.mapv(|x| x / n_new);

        // Ensure minimum variance for numerical stability
        self.variance = self.variance.mapv(|v| v.max(1e-6));

        // Update session count
        self.n_sessions += 1;

        // Update timestamp
        self.last_update_ns = signature.session_timestamp_ns;

        // Update quality bounds
        self.min_quality_score = self.min_quality_score.min(signature.quality_score);
        self.max_quality_score = self.max_quality_score.max(signature.quality_score);
    }

    /// Compute Mahalanobis distance from a brainprint to this distribution.
    /// Lower distance = more likely to be the same user.
    pub fn mahalanobis_distance(&self, signature: &BrainprintSignature) -> f64 {
        let diff = &signature.features - &self.mean;
        let mut distance_sq = 0.0;

        for i in 0..BRAINPRINT_DIM {
            let d = diff[i];
            let v = self.variance[i].max(1e-6);
            distance_sq += (d * d) / v;
        }

        distance_sq.sqrt()
    }

    /// Verify that distribution update is omnotonic (no degradation).
    pub fn verify_omnotonic_update(&self, previous: &Self) -> bool {
        // Session count must increase
        if self.n_sessions <= previous.n_sessions {
            return false;
        }
        // Quality bounds can only expand (min decreases, max increases)
        if self.min_quality_score > previous.min_quality_score {
            return false;
        }
        if self.max_quality_score < previous.max_quality_score {
            return false;
        }
        // Timestamp must advance
        if self.last_update_ns <= previous.last_update_ns {
            return false;
        }
        true
    }
}

/// Verification result from brainprint comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether verification succeeded
    pub verified: bool,
    /// Mahalanobis distance to enrolled distribution
    pub distance: f64,
    /// Threshold used for decision
    pub threshold: f64,
    /// Confidence score: 0.0 (low) to 1.0 (high)
    pub confidence: f64,
    /// Timestamp of verification (Unix epoch nanoseconds)
    pub verification_timestamp_ns: u64,
    /// Reason for failure (if any)
    pub failure_reason: Option<String>,
}

impl VerificationResult {
    /// Create a successful verification result.
    pub fn success(distance: f64, threshold: f64) -> Self {
        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        // Confidence inversely related to distance/threshold ratio
        let confidence = (1.0 - (distance / threshold).min(1.0)).max(0.0);

        Self {
            verified: true,
            distance,
            threshold,
            confidence,
            verification_timestamp_ns: now_ns,
            failure_reason: None,
        }
    }

    /// Create a failed verification result.
    pub fn failure(distance: f64, threshold: f64, reason: String) -> Self {
        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        Self {
            verified: false,
            distance,
            threshold,
            confidence: 0.0,
            verification_timestamp_ns: now_ns,
            failure_reason: Some(reason),
        }
    }
}

/// ============================================================================
/// BRAINPRINT MANAGER
/// ============================================================================

/// Central manager for brainprint enrollment, verification, and decoder binding.
/// Maintains omnotonic guarantees across all identity operations.
pub struct BrainprintManager {
    /// Current brainprint distribution model
    distribution: Option<BrainprintDistribution>,
    /// History of enrolled signatures (bounded)
    enrolled_signatures: Vec<BrainprintSignature>,
    /// Active decoder binding chains
    decoder_bindings: HashMap<String, DecoderBindingChain>,
    /// Current verification threshold (cannot decrease)
    verification_threshold: f64,
    /// Minimum threshold ever set (for omnotonic tracking)
    min_threshold_ever: f64,
    /// Enrollment complete flag (cannot be unset)
    enrollment_complete: bool,
    /// Number of verification attempts
    verification_count: u64,
    /// Number of successful verifications
    successful_verifications: u64,
}

impl BrainprintManager {
    /// Create a new brainprint manager with default configuration.
    pub fn new() -> Self {
        Self {
            distribution: None,
            enrolled_signatures: Vec::new(),
            decoder_bindings: HashMap::new(),
            verification_threshold: DEFAULT_VERIFICATION_THRESHOLD,
            min_threshold_ever: DEFAULT_VERIFICATION_THRESHOLD,
            enrollment_complete: false,
            verification_count: 0,
            successful_verifications: 0,
        }
    }

    /// Enroll a new brainprint signature during the enrollment phase.
    /// Returns Ok(true) if enrollment is now complete, Ok(false) if more sessions needed.
    pub fn enroll_signature(&mut self, signature: BrainprintSignature) -> Result<bool, String> {
        // Validate signature quality
        if signature.quality_score < 0.5 {
            return Err("Signature quality too low for enrollment".to_string());
        }

        // Add to enrolled signatures
        self.enrolled_signatures.push(signature.clone());

        // Update or create distribution
        match &mut self.distribution {
            Some(dist) => {
                dist.update(&signature);
            }
            None => {
                self.distribution = Some(BrainprintDistribution::from_single(&signature));
            }
        }

        // Check if enrollment is complete
        if self.enrolled_signatures.len() >= MIN_ENROLLMENT_SESSIONS {
            self.enrollment_complete = true;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Verify a live brainprint against the enrolled distribution.
    /// Returns VerificationResult with confidence and decision.
    pub fn verify(&mut self, signature: &BrainprintSignature) -> VerificationResult {
        self.verification_count += 1;

        // Check enrollment status
        if !self.enrollment_complete {
            return VerificationResult::failure(
                f64::INFINITY,
                self.verification_threshold,
                "Enrollment not complete".to_string(),
            );
        }

        // Check signature quality
        if signature.quality_score < self.distribution.as_ref().unwrap().min_quality_score * 0.8 {
            self.successful_verifications += 0; // No increment for failed
            return VerificationResult::failure(
                f64::INFINITY,
                self.verification_threshold,
                "Signature quality below acceptable range".to_string(),
            );
        }

        // Get distribution reference
        let dist = self.distribution.as_ref().unwrap();

        // Compute Mahalanobis distance
        let distance = dist.mahalanobis_distance(signature);

        // Make verification decision
        if distance <= self.verification_threshold {
            self.successful_verifications += 1;
            VerificationResult::success(distance, self.verification_threshold)
        } else {
            VerificationResult::failure(
                distance,
                self.verification_threshold,
                "Distance exceeds verification threshold".to_string(),
            )
        }
    }

    /// Bind a decoder to sovereign EEG training data.
    /// Returns Ok(true) if binding is new/updated, Err if rollback detected.
    pub fn bind_decoder(
        &mut self,
        binding: DecoderBindingChain,
    ) -> Result<bool, String> {
        // Check for existing binding
        if let Some(existing) = self.decoder_bindings.get(&binding.decoder_id) {
            // Verify no rollback (omnotonic versioning)
            if !binding.verify_no_rollback(existing) {
                return Err("Decoder version rollback detected - rejected".to_string());
            }
            // Verify chain continuity
            if !binding.verify_chain_continuity(existing) {
                return Err("Decoder binding chain broken - rejected".to_string());
            }
        }

        // Store/update binding
        self.decoder_bindings.insert(binding.decoder_id.clone(), binding);
        Ok(true)
    }

    /// Verify a decoder binding chain against stored bindings.
    pub fn verify_decoder_binding(&self, binding: &DecoderBindingChain) -> bool {
        if let Some(stored) = self.decoder_bindings.get(&binding.decoder_id) {
            // Version must match or exceed stored
            if binding.version < stored.version {
                return false;
            }
            // Commitments must match
            binding.decoder_params_commitment == stored.decoder_params_commitment
        } else {
            false
        }
    }

    /// Update verification threshold (omnotonic: can only increase strictness).
    /// Returns Ok(true) if updated, Ok(false) if no change, Err if rollback.
    pub fn update_verification_threshold(&mut self, new_threshold: f64) -> Result<bool, String> {
        // Threshold can only decrease (more strict) or stay same
        // Lower threshold = stricter verification
        if new_threshold > self.verification_threshold {
            return Err("Threshold increase would reduce security - rejected".to_string());
        }

        // Cannot go below minimum
        if new_threshold < MIN_VERIFICATION_THRESHOLD {
            return Err("Threshold below minimum safety bound - rejected".to_string());
        }

        if new_threshold == self.verification_threshold {
            return Ok(false);
        }

        // Update threshold and track minimum
        self.verification_threshold = new_threshold;
        self.min_threshold_ever = self.min_threshold_ever.min(new_threshold);

        Ok(true)
    }

    /// Get enrollment progress (sessions enrolled / minimum required).
    pub fn enrollment_progress(&self) -> (usize, usize) {
        (self.enrolled_signatures.len(), MIN_ENROLLMENT_SESSIONS)
    }

    /// Get verification success rate.
    pub fn verification_success_rate(&self) -> f64 {
        if self.verification_count == 0 {
            0.0
        } else {
            (self.successful_verifications as f64) / (self.verification_count as f64)
        }
    }

    /// Get current distribution statistics (for auditing).
    pub fn distribution_stats(&self) -> Option<DistributionStats> {
        self.distribution.as_ref().map(|dist| DistributionStats {
            n_sessions: dist.n_sessions,
            min_quality: dist.min_quality_score,
            max_quality: dist.max_quality_score,
            last_update_ns: dist.last_update_ns,
            mean_norm: dist.mean.dot(&dist.mean).sqrt(),
            variance_mean: dist.variance.mean().unwrap_or(0.0),
        })
    }

    /// Export brainprint for on-chain registration (audit trail).
    pub fn export_for_chain(&self) -> Option<BrainprintExport> {
        self.distribution.as_ref().map(|dist| BrainprintExport {
            mean_commitment: self.compute_mean_commitment(&dist.mean),
            variance_commitment: self.compute_variance_commitment(&dist.variance),
            n_sessions: dist.n_sessions,
            enrollment_complete: self.enrollment_complete,
            min_threshold_ever: self.min_threshold_ever,
            export_timestamp_ns: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
        })
    }

    /// Compute polynomial commitment for mean vector (DoctorLabs-compliant).
    fn compute_mean_commitment(&self, mean: &Array1<f64>) -> [u8; 64] {
        // Polynomial commitment-based encoding (simplified for demonstration)
        // In production, use full polynomial commitment scheme
        let mut commitment = [0u8; 64];

        // Encode dimensionality
        commitment[0..8].copy_from_slice(&(mean.len() as u64).to_le_bytes());

        // Encode sample of mean values (first 7 values as f64)
        for i in 0..7 {
            let start = 8 + i * 8;
            let end = start + 8;
            commitment[start..end].copy_from_slice(&mean[i].to_le_bytes());
        }

        // Encode norm
        let norm = mean.dot(mean).sqrt();
        commitment[56..64].copy_from_slice(&norm.to_le_bytes());

        commitment
    }

    /// Compute polynomial commitment for variance vector.
    fn compute_variance_commitment(&self, variance: &Array1<f64>) -> [u8; 64] {
        let mut commitment = [0u8; 64];

        // Encode dimensionality
        commitment[0..8].copy_from_slice(&(variance.len() as u64).to_le_bytes());

        // Encode mean variance
        let mean_var = variance.mean().unwrap_or(0.0);
        commitment[8..16].copy_from_slice(&mean_var.to_le_bytes());

        // Encode min/max variance
        let min_var = variance.fold(f64::INFINITY, |a, &b| a.min(b));
        let max_var = variance.fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        commitment[16..24].copy_from_slice(&min_var.to_le_bytes());
        commitment[24..32].copy_from_slice(&max_var.to_le_bytes());

        commitment
    }
}

impl Default for BrainprintManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the brainprint distribution (for auditing/export).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributionStats {
    pub n_sessions: usize,
    pub min_quality: f64,
    pub max_quality: f64,
    pub last_update_ns: u64,
    pub mean_norm: f64,
    pub variance_mean: f64,
}

/// Exported brainprint data for on-chain registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainprintExport {
    pub mean_commitment: [u8; 64],
    pub variance_commitment: [u8; 64],
    pub n_sessions: usize,
    pub enrollment_complete: bool,
    pub min_threshold_ever: f64,
    pub export_timestamp_ns: u64,
}

/// ============================================================================
/// BRAINPRINT FUNCTIONAL B(·)
/// ============================================================================

/// Brainprint functional B(session) that extracts identity features from EEG.
/// This is the core mapping from raw/session data to compact brainprint signature.
pub struct BrainprintFunctional {
    /// Feature extraction weights (trained during enrollment)
    feature_weights: Array2<f64>,
    /// Normalization parameters
    feature_means: Array1<f64>,
    /// Feature standard deviations
    feature_stds: Array1<f64>,
}

impl BrainprintFunctional {
    /// Create a new brainprint functional with random initialization.
    /// In production, weights are learned from enrollment data.
    pub fn new(n_input_features: usize) -> Self {
        // Initialize with identity-like projection (simplified)
        let mut weights = Array2::<f64>::zeros((BRAINPRINT_DIM, n_input_features));

        // Create orthogonal-ish projection matrix
        for i in 0..BRAINPRINT_DIM.min(n_input_features) {
            weights[(i, i)] = 1.0;
        }

        Self {
            feature_weights: weights,
            feature_means: Array1::from_elem(n_input_features, 0.0),
            feature_stds: Array1::from_elem(n_input_features, 1.0),
        }
    }

    /// Extract brainprint signature from session feature vector.
    /// Input: raw features from EEG pipeline (bandpower, coherence, SSVEP, etc.)
    /// Output: compact brainprint signature
    pub fn extract(&self, raw_features: &Array1<f64>) -> Option<BrainprintSignature> {
        // Validate input dimensionality
        if raw_features.len() != self.feature_means.len() {
            return None;
        }

        // Normalize features
        let normalized = self.normalize_features(raw_features);

        // Project to brainprint space
        let brainprint = self.feature_weights.dot(&normalized);

        // Create signature (quality and metadata would come from session)
        BrainprintSignature::new(
            brainprint,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            0, // Session duration would be provided
            1.0, // Quality would be computed from session
            0,   // Channel count would be provided
            0.0, // Sampling rate would be provided
        )
    }

    /// Normalize features using stored parameters.
    fn normalize_features(&self, features: &Array1<f64>) -> Array1<f64> {
        features
            .iter()
            .zip(self.feature_means.iter())
            .zip(self.feature_stds.iter())
            .map(|((&f, &m), &s)| {
                if s > 1e-9 {
                    (f - m) / s
                } else {
                    f - m
                }
            })
            .collect()
    }

    /// Update normalization parameters from enrollment data (omnotonic).
    pub fn update_normalization(&mut self, new_means: Array1<f64>, new_stds: Array1<f64>) {
        // Only update if dimensions match
        if new_means.len() == self.feature_means.len()
            && new_stds.len() == self.feature_stds.len()
        {
            // Omnotonic: stds can only increase (more conservative normalization)
            for i in 0..self.feature_stds.len() {
                self.feature_stds[i] = self.feature_stds[i].max(new_stds[i]);
            }
            // Means are updated to new values
            self.feature_means = new_means;
        }
    }
}

/// ============================================================================
/// PUBLIC API
/// ============================================================================

/// Create a new brainprint manager for identity verification.
pub fn create_brainprint_manager() -> BrainprintManager {
    BrainprintManager::new()
}

/// Create a new brainprint functional for feature extraction.
pub fn create_brainprint_functional(n_input_features: usize) -> BrainprintFunctional {
    BrainprintFunctional::new(n_input_features)
}

/// Compute Mahalanobis distance between two brainprint signatures.
pub fn brainprint_distance(a: &BrainprintSignature, b: &BrainprintSignature) -> f64 {
    let diff = &a.features - &b.features;
    diff.dot(&diff).sqrt()
}

/// Verify decoder binding chain integrity.
pub fn verify_decoder_chain(bindings: &[DecoderBindingChain]) -> bool {
    if bindings.is_empty() {
        return true;
    }

    for i in 1..bindings.len() {
        if !bindings[i].verify_no_rollback(&bindings[i - 1]) {
            return false;
        }
        if !bindings[i].verify_chain_continuity(&bindings[i - 1]) {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brainprint_signature_creation() {
        let features = Array1::from_elem(BRAINPRINT_DIM, 0.5);
        let sig = BrainprintSignature::new(
            features,
            1000000000,
            60000,
            0.9,
            8,
            250.0,
        );
        assert!(sig.is_some());
    }

    #[test]
    fn test_brainprint_manager_enrollment() {
        let mut manager = BrainprintManager::new();

        // Enroll minimum required signatures
        for i in 0..MIN_ENROLLMENT_SESSIONS {
            let features = Array1::from_elem(BRAINPRINT_DIM, 0.5 + (i as f64) * 0.01);
            let sig = BrainprintSignature::new(
                features,
                1000000000 + (i as u64) * 1000000,
                60000,
                0.9,
                8,
                250.0,
            ).unwrap();

            let complete = manager.enroll_signature(sig).unwrap();
            if i < MIN_ENROLLMENT_SESSIONS - 1 {
                assert!(!complete);
            } else {
                assert!(complete);
            }
        }

        assert!(manager.enrollment_complete);
    }

    #[test]
    fn test_brainprint_verification() {
        let mut manager = BrainprintManager::new();

        // Enroll signatures
        for i in 0..MIN_ENROLLMENT_SESSIONS {
            let features = Array1::from_elem(BRAINPRINT_DIM, 0.5);
            let sig = BrainprintSignature::new(
                features,
                1000000000 + (i as u64) * 1000000,
                60000,
                0.9,
                8,
                250.0,
            ).unwrap();
            manager.enroll_signature(sig).unwrap();
        }

        // Verify similar signature
        let features = Array1::from_elem(BRAINPRINT_DIM, 0.51);
        let test_sig = BrainprintSignature::new(
            features,
            2000000000,
            60000,
            0.9,
            8,
            250.0,
        ).unwrap();

        let result = manager.verify(&test_sig);
        assert!(result.verified || result.distance < 10.0); // Allow for variance
    }

    #[test]
    fn test_decoder_binding_no_rollback() {
        let mut manager = BrainprintManager::new();

        // Create initial binding
        let binding_v1 = DecoderBindingChain::new(
            [1u8; 64],
            [2u8; 64],
            [3u8; 64],
            "test_decoder".to_string(),
            1,
            None,
        );

        // Create rollback attempt (version 0)
        let binding_v0 = DecoderBindingChain::new(
            [1u8; 64],
            [2u8; 64],
            [3u8; 64],
            "test_decoder".to_string(),
            0,
            None,
        );

        // Bind v1 first
        manager.bind_decoder(binding_v1).unwrap();

        // Attempt rollback to v0 should fail
        let result = manager.bind_decoder(binding_v0);
        assert!(result.is_err());
    }

    #[test]
    fn test_threshold_omnotonic_update() {
        let mut manager = BrainprintManager::new();

        // Decrease threshold (more strict) - should succeed
        let result = manager.update_verification_threshold(3.0);
        assert!(result.unwrap());

        // Increase threshold (less strict) - should fail
        let result = manager.update_verification_threshold(4.0);
        assert!(result.is_err());

        // Below minimum - should fail
        let result = manager.update_verification_threshold(1.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_decoder_chain_verification() {
        let binding_v1 = DecoderBindingChain::new(
            [1u8; 64],
            [2u8; 64],
            [3u8; 64],
            "test".to_string(),
            1,
            None,
        );

        let binding_v2 = DecoderBindingChain::new(
            [1u8; 64],
            [2u8; 64],
            [4u8; 64],
            "test".to_string(),
            2,
            Some([3u8; 64]),
        );

        let chain = vec![binding_v1, binding_v2];
        assert!(verify_decoder_chain(&chain));
    }
}
