// eeg-math/src/ota/omnotonic_guard.rs

#![allow(clippy::needless_return)]

use serde::{Deserialize, Serialize};

/// High-level CyberNano / Cybernetic state vector.
/// This is intentionally generic: you can embed EEG network coords,
/// validator params, and energy metrics into this struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CyberState {
    /// Capability / protection dimensions (monotone non-decreasing).
    pub upgrade_dims: Vec<f64>,
    /// Load / burden / risk dimensions (monotone non-increasing).
    pub burden_dims: Vec<f64>,
    /// Free dimensions (may move both ways inside safe region).
    pub free_dims: Vec<f64>,
    /// Biophysical / energy snapshot used for Biocompatibility Index.
    pub energy_vec: Vec<f64>, // e.g., [norm_sugar, norm_protein, norm_blood, norm_temp, ...]
    /// Workload / neuromorphic stress snapshot.
    pub workload_vec: Vec<f64>, // e.g., [spike_rate_norm, duty_cycle, etc.]
}

/// Decoded control vector from EEG (u_t).
/// This is the "raw request" from the BCI decoder layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlVector {
    pub delta_upgrade: Vec<f64>,
    pub delta_burden: Vec<f64>,
    pub delta_free: Vec<f64>,
}

/// Static parameters governing Biocompatibility and Lyapunov risk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardParams {
    /// Linear weights for energy contribution in Biocompatibility Index.
    pub biocomp_energy_weights: Vec<f64>,
    /// Linear weights for workload contribution.
    pub biocomp_workload_weights: Vec<f64>,
    /// Threshold for biocompatibility (must remain strictly below this).
    pub biocomp_threshold: f64, // should be < 0.8 by your spec.
    /// Risk weights for Lyapunov-like functional V(z).
    pub risk_weights_upgrade: Vec<f64>,
    pub risk_weights_burden: Vec<f64>,
    pub risk_weights_free: Vec<f64>,
}

/// Helper: dot product between two equal-length slices.
fn dot(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len(), "Dot: length mismatch");
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Sigmoid used to keep Biocompatibility Index in (0, 1).
fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// Compute Biocompatibility Index B(z) from CyberState and parameters.
/// Index in (0, 1); must remain < threshold to be allowed.
pub fn biocompatibility_index(z: &CyberState, params: &GuardParams) -> f64 {
    let e = &z.energy_vec;
    let w = &z.workload_vec;

    assert_eq!(
        e.len(),
        params.biocomp_energy_weights.len(),
        "Energy vector and weights mismatch"
    );
    assert_eq!(
        w.len(),
        params.biocomp_workload_weights.len(),
        "Workload vector and weights mismatch"
    );

    let energy_term = dot(e, &params.biocomp_energy_weights);
    let workload_term = dot(w, &params.biocomp_workload_weights);
    // Combined "stress level"
    let stress = energy_term + workload_term;
    sigmoid(stress)
}

/// Simple Lyapunov-like risk functional V(z).
/// Larger values mean "higher systemic risk".
pub fn risk_functional(z: &CyberState, params: &GuardParams) -> f64 {
    assert_eq!(
        z.upgrade_dims.len(),
        params.risk_weights_upgrade.len(),
        "Upgrade dims vs risk weights mismatch"
    );
    assert_eq!(
        z.burden_dims.len(),
        params.risk_weights_burden.len(),
        "Burden dims vs risk weights mismatch"
    );
    assert_eq!(
        z.free_dims.len(),
        params.risk_weights_free.len(),
        "Free dims vs risk weights mismatch"
    );

    let v_up = dot(&z.upgrade_dims, &params.risk_weights_upgrade);
    let v_burden = dot(&z.burden_dims, &params.risk_weights_burden);
    let v_free = dot(&z.free_dims, &params.risk_weights_free);
    v_up + v_burden + v_free
}

/// Projected update map Φ(z_t, u_t) enforcing:
/// - upgrade_dims are monotone non-decreasing,
/// - burden_dims are monotone non-increasing,
/// - Biocompatibility Index stays below threshold,
/// - risk_functional is non-increasing (if needed by configuration).
///
/// Returns (z_{t+1}, applied_control_scale):
///   applied_control_scale ∈ [0, 1] indicates how much of u_t was applied
///   before hitting constraints (1.0 means full application, 0.0 means no change).
pub fn omnotonic_update(
    z_t: &CyberState,
    u_t: &ControlVector,
    params: &GuardParams,
    enforce_risk_nonincreasing: bool,
) -> (CyberState, f64) {
    assert_eq!(
        z_t.upgrade_dims.len(),
        u_t.delta_upgrade.len(),
        "Upgrade dims vs control mismatch"
    );
    assert_eq!(
        z_t.burden_dims.len(),
        u_t.delta_burden.len(),
        "Burden dims vs control mismatch"
    );
    assert_eq!(
        z_t.free_dims.len(),
        u_t.delta_free.len(),
        "Free dims vs control mismatch"
    );

    // Baseline risk and biocompatibility.
    let v_prev = risk_functional(z_t, params);
    let b_prev = biocompatibility_index(z_t, params);

    // Start from full application and back off if constraints are violated.
    let mut alpha_hi = 1.0;
    let mut alpha_lo = 0.0;
    let mut alpha_mid = 1.0;
    let max_iters = 32; // enough for high precision on [0,1]

    let mut best_state = z_t.clone();
    let mut best_alpha = 0.0;

    for _ in 0..max_iters {
        alpha_mid = 0.5 * (alpha_lo + alpha_hi);

        // Proposal: tilde{z}_{t+1} = z_t + alpha * G u_t; here G = I for simplicity.
        let mut z_candidate = z_t.clone();

        // Upgrade dims: propose additive step, then project with max().
        for (i, du) in u_t.delta_upgrade.iter().enumerate() {
            let proposed = z_t.upgrade_dims[i] + alpha_mid * du;
            z_candidate.upgrade_dims[i] = proposed.max(z_t.upgrade_dims[i]);
        }

        // Burden dims: propose additive step, then project with min().
        for (i, db) in u_t.delta_burden.iter().enumerate() {
            let proposed = z_t.burden_dims[i] + alpha_mid * db;
            z_candidate.burden_dims[i] = proposed.min(z_t.burden_dims[i]);
        }

        // Free dims: unprojected, but still subject to biocompatibility & risk.
        for (i, df) in u_t.delta_free.iter().enumerate() {
            let proposed = z_t.free_dims[i] + alpha_mid * df;
            z_candidate.free_dims[i] = proposed;
        }

        // Recompute Biocompatibility and risk.
        let b_new = biocompatibility_index(&z_candidate, params);
        let v_new = risk_functional(&z_candidate, params);

        let biocomp_ok = b_new < params.biocomp_threshold && b_new < 0.8;
        let risk_ok = if enforce_risk_nonincreasing {
            v_new <= v_prev + 1e-9 // small epsilon for float tolerance
        } else {
            true
        };

        if biocomp_ok && risk_ok {
            // Candidate is safe: accept and try to increase alpha.
            best_state = z_candidate;
            best_alpha = alpha_mid;
            alpha_lo = alpha_mid;
        } else {
            // Candidate violates constraints: reduce alpha.
            alpha_hi = alpha_mid;
        }

        if (alpha_hi - alpha_lo).abs() < 1e-4 {
            break;
        }
    }

    (best_state, best_alpha)
}
