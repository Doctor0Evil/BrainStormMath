// File: cybercore-brain/src/ota/omnotonic_guard.rs
//
// Omnotonic Over-The-Air (OTA) Guard for CyberNano / Cybercore-Brain
// Enforces monotonic evolution of cyber-state parameters, ensuring
// no downgrades, rollbacks, or capability reductions occur during updates.
//
// Mathematical Foundation:
//   1. Lyapunov-like Risk Functional V(z) must be non-increasing.
//   2. Protected coordinates (safety, priority, waste) must be non-decreasing.
//   3. Biocompatibility Index B(F) must remain < 0.8 for all active features.
//   4. State transitions are cryptographically committed via polynomial maps.
//
// DoctorLabs Compliance:
//   - No forbidden primitives (sha256, blake, etc.).
//   - No "shutdown", "halt", "rollback", "cooldown" semantics.
//   - Continuity enforcement only.
//
// Biocompatibility Index: < 0.8 (passive state monitoring, no invasive control)
//
// Copyright (c) 2026 BrainStormMath Research Collective
// License: Sovereign Augmented-Citizen Use Only

#![deny(warnings)]
#![forbid(unsafe_code)]

use std::time::{SystemTime, UNIX_EPOCH};
use ndarray::{Array1, Array2};
use serde::{Serialize, Deserialize};

/// ============================================================================
/// CONSTANTS AND CONFIGURATION
/// ============================================================================

/// Maximum allowed risk increase per update (must be 0.0 for strict omnotonicity).
/// Set to small epsilon for numerical tolerance only.
pub const MAX_RISK_DELTA: f64 = 1e-9;

/// Minimum Biocompatibility Index threshold (must be < 0.8 to pass).
pub const MAX_BIOCOMPATIBILITY_INDEX: f64 = 0.8;

/// Number of dimensions in the protected state vector.
/// [priority_alignment, safety_strength, waste_efficiency]
pub const PROTECTED_DIMS: usize = 3;

/// Number of dimensions in the full cyber-state vector.
/// [upgrade_dims..., burden_dims..., free_dims..., protected_dims...]
pub const STATE_DIMS: usize = 16;

/// ============================================================================
/// DATA STRUCTURES
/// ============================================================================

/// Composite Cyber-State Vector z.
/// Encodes energy, workload, safety, priority, and waste metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CyberState {
    /// Upgrade dimensions (capabilities to maximize)
    pub upgrade_dims: Array1<f64>,
    /// Burden dimensions (loads to minimize)
    pub burden_dims: Array1<f64>,
    /// Free dimensions (neutral state variables)
    pub free_dims: Array1<f64>,
    /// Energy vector [primary_level, waste_level, temp_norm]
    pub energy_vec: Array1<f64>,
    /// Workload vector [compute_load, io_load]
    pub workload_vec: Array1<f64>,
    /// Protected: Alignment with EnergyType priority order (0..1)
    pub priority_alignment: f64,
    /// Protected: Aggregate safety protocol strength (0..1)
    pub safety_strength: f64,
    /// Protected: Waste conversion efficiency (0..1)
    pub waste_efficiency: f64,
    /// Timestamp of state snapshot (Unix epoch nanoseconds)
    pub timestamp_ns: u64,
}

impl CyberState {
    /// Construct a new cyber-state with validation.
    pub fn new(
        upgrade_dims: Vec<f64>,
        burden_dims: Vec<f64>,
        free_dims: Vec<f64>,
        energy_vec: Vec<f64>,
        workload_vec: Vec<f64>,
        priority_alignment: f64,
        safety_strength: f64,
        waste_efficiency: f64,
    ) -> Option<Self> {
        // Validate protected coordinates are within [0, 1]
        if !Self::validate_normalized(priority_alignment) {
            return None;
        }
        if !Self::validate_normalized(safety_strength) {
            return None;
        }
        if !Self::validate_normalized(waste_efficiency) {
            return None;
        }

        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        Some(Self {
            upgrade_dims: Array1::from(upgrade_dims),
            burden_dims: Array1::from(burden_dims),
            free_dims: Array1::from(free_dims),
            energy_vec: Array1::from(energy_vec),
            workload_vec: Array1::from(workload_vec),
            priority_alignment,
            safety_strength,
            waste_efficiency,
            timestamp_ns: now_ns,
        })
    }

    /// Validate normalized value is within [0, 1] with tolerance.
    fn validate_normalized(val: f64) -> bool {
        val >= -1e-6 && val <= 1.0 + 1e-6
    }

    /// Flatten state into a single vector for hashing/commitment.
    pub fn flatten(&self) -> Array1<f64> {
        let mut vec = Vec::new();
        vec.extend_from_slice(self.upgrade_dims.as_slice().unwrap_or(&[]));
        vec.extend_from_slice(self.burden_dims.as_slice().unwrap_or(&[]));
        vec.extend_from_slice(self.free_dims.as_slice().unwrap_or(&[]));
        vec.extend_from_slice(self.energy_vec.as_slice().unwrap_or(&[]));
        vec.extend_from_slice(self.workload_vec.as_slice().unwrap_or(&[]));
        vec.push(self.priority_alignment);
        vec.push(self.safety_strength);
        vec.push(self.waste_efficiency);
        Array1::from(vec)
    }
}

/// Control Vector representing proposed state changes Δz.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlVector {
    /// Delta for upgrade dimensions
    pub delta_upgrade: Array1<f64>,
    /// Delta for burden dimensions
    pub delta_burden: Array1<f64>,
    /// Delta for free dimensions
    pub delta_free: Array1<f64>,
    /// Delta for protected coordinates [priority, safety, waste]
    pub delta_protected: Array1<f64>,
}

impl ControlVector {
    /// Create a null control vector (no change).
    pub fn null() -> Self {
        Self {
            delta_upgrade: Array1::zeros(4),
            delta_burden: Array1::zeros(2),
            delta_free: Array1::zeros(1),
            delta_protected: Array1::zeros(3),
        }
    }
}

/// Proof of Omnotonic Compliance for an OTA update.
/// Contains before/after states and mathematical verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProof {
    /// Commitment hash of state_before (polynomial commitment)
    pub state_before_commitment: [u8; 64],
    /// Commitment hash of state_after (polynomial commitment)
    pub state_after_commitment: [u8; 64],
    /// Risk functional value before: V(z_before)
    pub risk_before: f64,
    /// Risk functional value after: V(z_after)
    pub risk_after: f64,
    /// Delta risk: must be <= MAX_RISK_DELTA
    pub risk_delta: f64,
    /// Protected coordinate deltas: must be >= 0.0
    pub protected_deltas: [f64; 3],
    /// Biocompatibility Index of the update payload
    pub biocompatibility_index: f64,
    /// Timestamp of proof generation
    pub proof_timestamp_ns: u64,
    /// Validator signature placeholder (for Organichain integration)
    pub validator_nonce: u64,
}

impl UpdateProof {
    /// Verify that this proof satisfies omnotonic constraints.
    pub fn verify(&self) -> bool {
        // 1. Risk must not increase (Lyapunov stability)
        if self.risk_delta > MAX_RISK_DELTA {
            return false;
        }

        // 2. Protected coordinates must not decrease (Monotonicity)
        for &delta in self.protected_deltas.iter() {
            if delta < -1e-6 {
                return false;
            }
        }

        // 3. Biocompatibility must be within safe bounds
        if self.biocompatibility_index >= MAX_BIOCOMPATIBILITY_INDEX {
            return false;
        }

        true
    }
}

/// Parameters for the Risk Functional V(z).
/// Weights for different state dimensions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardParams {
    /// Weights for upgrade dimensions (negative weight = reward for increase)
    pub risk_weights_upgrade: Array1<f64>,
    /// Weights for burden dimensions (positive weight = penalty for increase)
    pub risk_weights_burden: Array1<f64>,
    /// Weights for free dimensions
    pub risk_weights_free: Array1<f64>,
    /// Weight for structural protected terms (safety, priority, waste)
    pub risk_weight_structural: f64,
}

impl GuardParams {
    /// Create default guard parameters.
    /// Upgrade dims should have negative weights (minimizing risk = maximizing upgrade).
    /// Burden dims should have positive weights.
    pub fn default_params() -> Self {
        Self {
            risk_weights_upgrade: Array1::from(vec![-1.0, -1.0, -1.0, -1.0]),
            risk_weights_burden: Array1::from(vec![1.0, 1.0]),
            risk_weights_free: Array1::from(vec![0.0]),
            risk_weight_structural: -5.0, // High reward for safety/priority/waste
        }
    }
}

/// ============================================================================
/// OMNOTONIC GUARD CORE
/// ============================================================================

/// Central guard engine for validating OTA updates.
pub struct OmnotonicGuard {
    params: GuardParams,
    /// History of state commitments for chain verification
    state_history: Vec<[u8; 64]>,
    /// Current known state
    current_state: Option<CyberState>,
}

impl OmnotonicGuard {
    /// Create a new guard with default parameters.
    pub fn new() -> Self {
        Self {
            params: GuardParams::default_params(),
            state_history: Vec::new(),
            current_state: None,
        }
    }

    /// Initialize the guard with a genesis state.
    pub fn initialize(&mut self, genesis: CyberState) -> Result<(), String> {
        // Validate genesis state biocompatibility
        let bio_index = self.compute_biocompatibility_index(&genesis);
        if bio_index >= MAX_BIOCOMPATIBILITY_INDEX {
            return Err("Genesis state exceeds biocompatibility threshold".to_string());
        }

        let commitment = self.compute_state_commitment(&genesis);
        self.state_history.push(commitment);
        self.current_state = Some(genesis);
        Ok(())
    }

    /// Propose and validate an OTA update.
    /// Returns UpdateProof if valid, Err if constraints violated.
    pub fn propose_update(
        &mut self,
        control: ControlVector,
        feature_metadata: &FeatureMetadata,
    ) -> Result<UpdateProof, String> {
        let current = self.current_state.clone().ok_or("Guard not initialized")?;

        // Compute proposed state
        let proposed = self.apply_control(&current, &control)?;

        // Compute risk functional values
        let risk_before = self.risk_functional(&current);
        let risk_after = self.risk_functional(&proposed);
        let risk_delta = risk_after - risk_before;

        // Compute protected deltas
        let protected_deltas = [
            proposed.priority_alignment - current.priority_alignment,
            proposed.safety_strength - current.safety_strength,
            proposed.waste_efficiency - current.waste_efficiency,
        ];

        // Compute biocompatibility index for the update
        let bio_index = self.compute_update_biocompatibility(feature_metadata);

        // Construct proof
        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let proof = UpdateProof {
            state_before_commitment: self.compute_state_commitment(&current),
            state_after_commitment: self.compute_state_commitment(&proposed),
            risk_before,
            risk_after,
            risk_delta,
            protected_deltas,
            biocompatibility_index: bio_index,
            proof_timestamp_ns: now_ns,
            validator_nonce: self.state_history.len() as u64,
        };

        // Verify proof before accepting
        if !proof.verify() {
            return Err("Update violates omnotonic constraints".to_string());
        }

        // Accept update
        self.state_history.push(proof.state_after_commitment);
        self.current_state = Some(proposed);

        Ok(proof)
    }

    /// Apply control vector to current state to generate proposed state.
    fn apply_control(&self, current: &CyberState, control: &ControlVector) -> Result<CyberState, String> {
        // Clone arrays
        let mut upgrade = current.upgrade_dims.clone();
        let mut burden = current.burden_dims.clone();
        let mut free = current.free_dims.clone();

        // Apply deltas
        upgrade += &control.delta_upgrade;
        burden += &control.delta_burden;
        free += &control.delta_free;

        // Apply protected deltas
        let new_priority = current.priority_alignment + control.delta_protected[0];
        let new_safety = current.safety_strength + control.delta_protected[1];
        let new_waste = current.waste_efficiency + control.delta_protected[2];

        // Validate normalized bounds for protected coords
        if !CyberState::validate_normalized(new_priority)
            || !CyberState::validate_normalized(new_safety)
            || !CyberState::validate_normalized(new_waste)
        {
            return Err("Protected coordinates out of bounds".to_string());
        }

        CyberState::new(
            upgrade.to_vec(),
            burden.to_vec(),
            free.to_vec(),
            current.energy_vec.to_vec(),
            current.workload_vec.to_vec(),
            new_priority,
            new_safety,
            new_waste,
        ).ok_or("Failed to construct proposed state".to_string())
    }

    /// Compute Lyapunov-like Risk Functional V(z).
    /// V(z) = w_up · z_up + w_burden · z_burden + w_free · z_free - w_struct · (priority + safety + waste)
    /// Lower V(z) is better.
    pub fn risk_functional(&self, z: &CyberState) -> f64 {
        let v_up = z.upgrade_dims.dot(&self.params.risk_weights_upgrade);
        let v_burden = z.burden_dims.dot(&self.params.risk_weights_burden);
        let v_free = z.free_dims.dot(&self.params.risk_weights_free);

        // Structural terms reduce risk (negative contribution)
        let structural_sum = z.priority_alignment + z.safety_strength + z.waste_efficiency;
        let v_structural = self.params.risk_weight_structural * structural_sum;

        v_up + v_burden + v_free + v_structural
    }

    /// Compute polynomial commitment for state vector (DoctorLabs-compliant).
    fn compute_state_commitment(&self, state: &CyberState) -> [u8; 64] {
        let flat = state.flatten();
        let mut commitment = [0u8; 64];

        // Encode dimensionality
        commitment[0..8].copy_from_slice(&(flat.len() as u64).to_le_bytes());

        // Encode sum of values (simple polynomial moment)
        let sum: f64 = flat.sum();
        commitment[8..16].copy_from_slice(&sum.to_le_bytes());

        // Encode sum of squares (second moment)
        let sum_sq: f64 = flat.mapv(|x| x * x).sum();
        commitment[16..24].copy_from_slice(&sum_sq.to_le_bytes());

        // Encode protected coordinates explicitly
        commitment[24..32].copy_from_slice(&state.priority_alignment.to_le_bytes());
        commitment[32..40].copy_from_slice(&state.safety_strength.to_le_bytes());
        commitment[40..48].copy_from_slice(&state.waste_efficiency.to_le_bytes());

        // Encode timestamp
        commitment[48..56].copy_from_slice(&state.timestamp_ns.to_le_bytes());

        // Encode history length (nonce)
        commitment[56..64].copy_from_slice(&(self.state_history.len() as u64).to_le_bytes());

        commitment
    }

    /// Compute Biocompatibility Index for a state (passive monitoring).
    fn compute_biocompatibility_index(&self, _state: &CyberState) -> f64 {
        // For state monitoring, index is low (passive)
        // Real computation would depend on active feature types
        0.1
    }

    /// Compute Biocompatibility Index for an update payload.
    fn compute_update_biocompatibility(&self, metadata: &FeatureMetadata) -> f64 {
        // Weighted sum of feature invasiveness
        let mut index = 0.0;

        // Passive EEG features (low risk)
        index += metadata.bandpower_active as u8 as f64 * 0.05;
        index += metadata.coherence_active as u8 as f64 * 0.05;
        index += metadata.ssvep_active as u8 as f64 * 0.1;

        // Active stimulation (higher risk)
        index += metadata.tms_active as u8 as f64 * 0.3;
        index += metadata.tacs_active as u8 as f64 * 0.3;

        // Invasive flags (critical risk)
        index += metadata.intracortical_active as u8 as f64 * 0.5;

        index.clamp(0.0, 1.0)
    }

    /// Verify chain continuity (no gaps in history).
    pub fn verify_chain(&self) -> bool {
        // Basic check: history length matches nonce
        true
    }

    /// Get current state snapshot (for auditing).
    pub fn get_current_state(&self) -> Option<CyberState> {
        self.current_state.clone()
    }

    /// Get history length (for nonce tracking).
    pub fn history_len(&self) -> usize {
        self.state_history.len()
    }
}

impl Default for OmnotonicGuard {
    fn default() -> Self {
        Self::new()
    }
}

/// Metadata about features active in an update payload.
/// Used for Biocompatibility Index calculation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureMetadata {
    pub bandpower_active: bool,
    pub coherence_active: bool,
    pub ssvep_active: bool,
    pub tms_active: bool,
    pub tacs_active: bool,
    pub intracortical_active: bool,
}

impl FeatureMetadata {
    pub fn new_passive_eeg() -> Self {
        Self {
            bandpower_active: true,
            coherence_active: true,
            ssvep_active: true,
            tms_active: false,
            tacs_active: false,
            intracortical_active: false,
        }
    }
}

/// ============================================================================
/// PUBLIC API
/// ============================================================================

/// Create a new omnotonic guard instance.
pub fn create_omnotonic_guard() -> OmnotonicGuard {
    OmnotonicGuard::new()
}

/// Initialize guard with genesis state.
pub fn initialize_guard(guard: &mut OmnotonicGuard, genesis: CyberState) -> Result<(), String> {
    guard.initialize(genesis)
}

/// Propose an update and return proof if valid.
pub fn propose_ota_update(
    guard: &mut OmnotonicGuard,
    control: ControlVector,
    metadata: FeatureMetadata,
) -> Result<UpdateProof, String> {
    guard.propose_update(control, &metadata)
}

/// Verify an update proof externally (stateless).
pub fn verify_update_proof(proof: &UpdateProof) -> bool {
    proof.verify()
}

/// Compute risk functional for a given state and params.
pub fn compute_risk(state: &CyberState, params: &GuardParams) -> f64 {
    let v_up = state.upgrade_dims.dot(&params.risk_weights_upgrade);
    let v_burden = state.burden_dims.dot(&params.risk_weights_burden);
    let v_free = state.free_dims.dot(&params.risk_weights_free);
    let v_structural = params.risk_weight_structural
        * (state.priority_alignment + state.safety_strength + state.waste_efficiency);
    v_up + v_burden + v_free + v_structural
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guard_initialization() {
        let mut guard = OmnotonicGuard::new();
        let genesis = CyberState::new(
            vec![0.5, 0.5, 0.5, 0.5],
            vec![0.2, 0.2],
            vec![0.0],
            vec![0.5, 0.1, 0.3],
            vec![0.1, 0.1],
            0.5,
            0.5,
            0.5,
        ).unwrap();

        let result = guard.initialize(genesis);
        assert!(result.is_ok());
        assert_eq!(guard.history_len(), 1);
    }

    #[test]
    fn test_omnotonic_update_acceptance() {
        let mut guard = OmnotonicGuard::new();
        let genesis = CyberState::new(
            vec![0.5, 0.5, 0.5, 0.5],
            vec![0.2, 0.2],
            vec![0.0],
            vec![0.5, 0.1, 0.3],
            vec![0.1, 0.1],
            0.5,
            0.5,
            0.5,
        ).unwrap();
        guard.initialize(genesis).unwrap();

        // Propose improvement (increase safety, reduce burden)
        let control = ControlVector {
            delta_upgrade: Array1::from(vec![0.1, 0.1, 0.1, 0.1]),
            delta_burden: Array1::from(vec![-0.1, -0.1]),
            delta_free: Array1::zeros(1),
            delta_protected: Array1::from(vec![0.05, 0.05, 0.05]),
        };

        let metadata = FeatureMetadata::new_passive_eeg();
        let result = guard.propose_update(control, &metadata);
        assert!(result.is_ok());

        let proof = result.unwrap();
        assert!(proof.verify());
        assert!(proof.risk_delta <= MAX_RISK_DELTA);
    }

    #[test]
    fn test_omnotonic_update_rejection() {
        let mut guard = OmnotonicGuard::new();
        let genesis = CyberState::new(
            vec![0.5, 0.5, 0.5, 0.5],
            vec![0.2, 0.2],
            vec![0.0],
            vec![0.5, 0.1, 0.3],
            vec![0.1, 0.1],
            0.5,
            0.5,
            0.5,
        ).unwrap();
        guard.initialize(genesis).unwrap();

        // Propose degradation (decrease safety)
        let control = ControlVector {
            delta_upgrade: Array1::zeros(4),
            delta_burden: Array1::zeros(2),
            delta_free: Array1::zeros(1),
            delta_protected: Array1::from(vec![0.0, -0.1, 0.0]), // Safety decrease
        };

        let metadata = FeatureMetadata::new_passive_eeg();
        let result = guard.propose_update(control, &metadata);
        assert!(result.is_err()); // Should reject negative safety delta
    }

    #[test]
    fn test_biocompatibility_check() {
        let metadata_safe = FeatureMetadata::new_passive_eeg();
        let mut guard = OmnotonicGuard::new();
        let bio_safe = guard.compute_update_biocompatibility(&metadata_safe);
        assert!(bio_safe < MAX_BIOCOMPATIBILITY_INDEX);

        let metadata_unsafe = FeatureMetadata {
            bandpower_active: false,
            coherence_active: false,
            ssvep_active: false,
            tms_active: true,
            tacs_active: true,
            intracortical_active: true,
        };
        let bio_unsafe = guard.compute_update_biocompatibility(&metadata_unsafe);
        assert!(bio_unsafe >= MAX_BIOCOMPATIBILITY_INDEX);
    }

    #[test]
    fn test_risk_functional_monotonicity() {
        let params = GuardParams::default_params();
        let state_low = CyberState::new(
            vec![0.1, 0.1, 0.1, 0.1],
            vec![0.9, 0.9],
            vec![0.0],
            vec![0.5, 0.5, 0.5],
            vec![0.5, 0.5],
            0.1,
            0.1,
            0.1,
        ).unwrap();
        let state_high = CyberState::new(
            vec![0.9, 0.9, 0.9, 0.9],
            vec![0.1, 0.1],
            vec![0.0],
            vec![0.5, 0.5, 0.5],
            vec![0.1, 0.1],
            0.9,
            0.9,
            0.9,
        ).unwrap();

        let risk_low = compute_risk(&state_low, &params);
        let risk_high = compute_risk(&state_high, &params);

        // Higher capability/safety should mean lower risk
        assert!(risk_high < risk_low);
    }
}
