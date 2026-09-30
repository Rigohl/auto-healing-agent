//! Pure-Rust MLP classifier for Auto-Healing Agent.
//!
//! Architecture (tiny, edge-friendly):
//!   Input  16  → Hidden 32 (ReLU) → Output 11 (softmax over OperatorId 0..10)
//!
//! Weights are loaded from a flat f32 buffer (exported by `scripts/export_weights`).
//! No Burn / no external ML runtime at inference time — only arithmetic.
//! Compatible with `wasm32-unknown-unknown` and `no_std` + alloc.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::String;
use repair_types::{FeatureVector, OperatorId, RepairAction};

const INPUT: usize = 16;
const HIDDEN: usize = 32;
const OUTPUT: usize = 11; // OperatorId 0..10

/// Total weights: W1 (16*32) + b1 (32) + W2 (32*11) + b2 (11) = 512 + 32 + 352 + 11 = 907
pub const WEIGHT_COUNT: usize = INPUT * HIDDEN + HIDDEN + HIDDEN * OUTPUT + OUTPUT;

pub struct RepairNet {
    /// Flat buffer: [W1 | b1 | W2 | b2]
    weights: [f32; WEIGHT_COUNT],
}

impl RepairNet {
    /// Create from a weight slice. Panics if length != WEIGHT_COUNT.
    pub fn from_weights(w: &[f32]) -> Self {
        assert_eq!(w.len(), WEIGHT_COUNT, "weight buffer must be {}", WEIGHT_COUNT);
        let mut weights = [0.0f32; WEIGHT_COUNT];
        weights.copy_from_slice(w);
        Self { weights }
    }

    /// Zero-initialized network (useful for tests / cold start).
    pub fn zeros() -> Self {
        Self {
            weights: [0.0; WEIGHT_COUNT],
        }
    }

    /// Forward pass → RepairAction.
    pub fn predict(&self, features: &FeatureVector) -> RepairAction {
        let x = features.as_slice();

        // Layer 1: 16 → 32 + ReLU
        let mut h = [0.0f32; HIDDEN];
        for j in 0..HIDDEN {
            let mut sum = self.bias1(j);
            for i in 0..INPUT {
                sum += x[i] * self.w1(i, j);
            }
            h[j] = if sum > 0.0 { sum } else { 0.0 };
        }

        // Layer 2: 32 → 11 (logits)
        let mut logits = [0.0f32; OUTPUT];
        for k in 0..OUTPUT {
            let mut sum = self.bias2(k);
            for j in 0..HIDDEN {
                sum += h[j] * self.w2(j, k);
            }
            logits[k] = sum;
        }

        // Softmax + argmax
        let (op_idx, confidence) = softmax_argmax(&logits);
        let risk = 1.0 - confidence;

        RepairAction {
            operator_id: OperatorId::from_u8(op_idx as u8),
            confidence,
            risk,
            rationale: String::new(),
        }
    }

    #[inline]
    fn w1(&self, i: usize, j: usize) -> f32 {
        self.weights[i * HIDDEN + j]
    }

    #[inline]
    fn bias1(&self, j: usize) -> f32 {
        self.weights[INPUT * HIDDEN + j]
    }

    #[inline]
    fn w2(&self, j: usize, k: usize) -> f32 {
        self.weights[INPUT * HIDDEN + HIDDEN + j * OUTPUT + k]
    }

    #[inline]
    fn bias2(&self, k: usize) -> f32 {
        self.weights[INPUT * HIDDEN + HIDDEN + HIDDEN * OUTPUT + k]
    }
}

fn softmax_argmax(logits: &[f32; OUTPUT]) -> (usize, f32) {
    let max = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let mut exps = [0.0f32; OUTPUT];
    let mut sum = 0.0f32;
    for i in 0..OUTPUT {
        let e = (logits[i] - max).exp();
        exps[i] = e;
        sum += e;
    }
    let mut best_i = 0;
    let mut best_p = 0.0f32;
    for i in 0..OUTPUT {
        let p = exps[i] / sum;
        if p > best_p {
            best_p = p;
            best_i = i;
        }
    }
    (best_i, best_p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use repair_types::FeatureVector;

    #[test]
    fn zeros_predicts_something() {
        let net = RepairNet::zeros();
        let fv = FeatureVector::zeros();
        let action = net.predict(&fv);
        // With zero weights all logits are 0 → uniform softmax → confidence ~1/11
        assert!(action.confidence > 0.05 && action.confidence < 0.2);
        assert_eq!(action.operator_id, OperatorId::NoOp); // argmax of equals → first
    }

    #[test]
    fn weight_count_matches() {
        assert_eq!(WEIGHT_COUNT, 907);
    }
}
