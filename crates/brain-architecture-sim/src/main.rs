// brain-architecture-sim/src/main.rs
// Minimal, production-style crate modeling diffusion dynamics on a brain connectome graph.
// Properties:
// - Uses symmetric Laplacian dynamics x_{t+1} = x_t - eta L_g x_t
// - Quadratic energy E(x) = 0.5 x^T L_g x is non-increasing for sufficiently small eta
// - No rollback path: we never store past L_g versions and we monotonically append a log
// - OTA-style "upgrade" only allowed if simple safety invariants are met

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Debug, Clone)]
struct Connectome {
    // Symmetric adjacency weights; in practice backed by a sparse structure,
    // but here kept dense for clarity. Size n x n.
    weights: Vec<f64>,
    n: usize,
}

impl Connectome {
    fn from_csv<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut rows: Vec<Vec<f64>> = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let row: Vec<f64> = line
                .split(',')
                .map(|tok| tok.trim().parse::<f64>())
                .collect::<Result<Vec<_>, _>>()?;
            rows.push(row);
        }

        let n = rows.len();
        for row in &rows {
            if row.len() != n {
                anyhow::bail!("Non-square matrix in connectome CSV");
            }
        }

        // Flatten to row-major
        let mut weights = Vec::with_capacity(n * n);
        for r in 0..n {
            for c in 0..n {
                let w = 0.5 * (rows[r][c] + rows[c][r]); // symmetrize
                weights.push(w.max(0.0)); // no negative weights
            }
        }

        Ok(Connectome { weights, n })
    }

    fn idx(&self, r: usize, c: usize) -> usize {
        r * self.n + c
    }

    /// Build the graph Laplacian L_g = D - W (dense).
    fn laplacian(&self) -> Vec<f64> {
        let n = self.n;
        let mut lap = vec![0.0_f64; n * n];

        // Compute degrees
        let mut deg = vec![0.0_f64; n];
        for r in 0..n {
            let mut sum = 0.0;
            for c in 0..n {
                sum += self.weights[self.idx(r, c)];
            }
            deg[r] = sum;
        }

        // Fill Laplacian
        for r in 0..n {
            for c in 0..n {
                let idx = self.idx(r, c);
                if r == c {
                    lap[idx] = deg[r];
                } else {
                    lap[idx] = -self.weights[idx];
                }
            }
        }

        lap
    }
}

#[derive(Debug, Clone)]
struct BrainState {
    /// Node activity vector x (e.g., band-limited power or firing rate).
    x: Vec<f64>,
}

impl BrainState {
    fn new_random(n: usize, seed: u64) -> Self {
        // Simple deterministic LCG for reproducible "random" values.
        let mut s = seed;
        let mut x = Vec::with_capacity(n);
        for _ in 0..n {
            // LCG parameters (numerically harmless here, no cryptographic role).
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
            let v = ((s >> 33) as f64) / ((1u64 << 31) as f64);
            x.push(v);
        }
        BrainState { x }
    }

    fn energy(&self, laplacian: &[f64], n: usize) -> f64 {
        // E(x) = 0.5 x^T L x
        let mut tmp = vec![0.0_f64; n];
        for r in 0..n {
            let mut acc = 0.0;
            for c in 0..n {
                acc += laplacian[r * n + c] * self.x[c];
            }
            tmp[r] = acc;
        }
        let mut dot = 0.0;
        for i in 0..n {
            dot += self.x[i] * tmp[i];
        }
        0.5 * dot
    }

    /// Single Euler step: x_{t+1} = x_t - eta * L x_t
    fn step(&mut self, laplacian: &[f64], n: usize, eta: f64) {
        let mut dx = vec![0.0_f64; n];
        for r in 0..n {
            let mut acc = 0.0;
            for c in 0..n {
                acc += laplacian[r * n + c] * self.x[c];
            }
            dx[r] = -eta * acc;
        }
        for i in 0..n {
            self.x[i] += dx[i];
        }
    }
}

/// Simple invariant bundle for OTA "upgrades".
#[derive(Debug, Clone)]
struct Invariants {
    /// Max allowed node-wise risk proxy (e.g., activity variance bound).
    max_risk: f64,
    /// Minimum allowed number of nodes (no structural rollback).
    min_nodes: usize,
}

/// Guarded OTA-style update to a new connectome.
fn try_upgrade_connectome(
    current: &Connectome,
    candidate: &Connectome,
    invariants: &Invariants,
) -> anyhow::Result<Connectome> {
    // Invariant 1: do not reduce number of nodes (no capability rollback).
    if candidate.n < invariants.min_nodes || candidate.n < current.n {
        anyhow::bail!("Rejected upgrade: node count would decrease");
    }

    // Invariant 2: approximate a node-wise risk proxy as sum of weights.
    // Enforce that max row-sum (degree) does not exceed max_risk.
    let mut max_deg = 0.0_f64;
    for r in 0..candidate.n {
        let mut sum = 0.0;
        for c in 0..candidate.n {
            sum += candidate.weights[candidate.idx(r, c)];
        }
        if sum > max_deg {
            max_deg = sum;
        }
    }
    if max_deg > invariants.max_risk {
        anyhow::bail!("Rejected upgrade: degree-based risk exceeds bound");
    }

    Ok(candidate.clone())
}

fn main() -> anyhow::Result<()> {
    // In a real system these would come from actual connectome data (e.g., structural MRI/EM).
    let path = "data/connectome_weights.csv";
    let base_connectome = Connectome::from_csv(path)?;

    let lap = base_connectome.laplacian();
    let n = base_connectome.n;

    let mut state = BrainState::new_random(n, 0xC0FFEE_u64);
    let eta = 0.01_f64;

    let mut prev_energy = state.energy(&lap, n);
    println!("Initial energy: {:.6}", prev_energy);

    for step_idx in 0..1000 {
        state.step(&lap, n, eta);
        let e = state.energy(&lap, n);

        if e > prev_energy + 1e-9 {
            eprintln!(
                "Warning: energy increased at step {} (prev {:.6}, new {:.6})",
                step_idx, prev_energy, e
            );
        }

        prev_energy = e;
        if step_idx % 100 == 0 {
            println!("Step {:4}, energy {:.6}", step_idx, e);
        }
    }

    // Demonstrate guarded OTA-like upgrade.
    let invariants = Invariants {
        max_risk: 1000.0,
        min_nodes: n,
    };

    // Here we simply re-use the same connectome as a "candidate".
    // In real usage this would be a new version learned from additional data.
    let upgraded = try_upgrade_connectome(&base_connectome, &base_connectome, &invariants)?;
    println!(
        "Upgrade accepted: n = {}, invariants = {:?}",
        upgraded.n, invariants
    );

    Ok(())
}
