//! MLP V0: 64 → 32 → 16 latent → operator/confidence/risk heads.
//! No LLM. Weights from exported artifact. no_std + alloc.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use repair_types::{FeatureVector, OperatorId, OPERATOR_COUNT, RepairAction};

const INPUT: usize = 64;
const HIDDEN: usize = 32;
const LATENT: usize = 16;
const OPS: usize = OPERATOR_COUNT;

/// Flat weight layout length.
pub const WEIGHT_COUNT: usize =
    INPUT * HIDDEN + HIDDEN + HIDDEN * LATENT + LATENT + LATENT * OPS + OPS + LATENT + 1 + LATENT + 1;

pub struct RepairNet {
    weights: Vec<f32>,
}

impl RepairNet {
    pub fn from_weights(w: &[f32]) -> Result<Self, &'static str> {
        if w.len() != WEIGHT_COUNT {
            return Err("weight length mismatch");
        }
        Ok(Self {
            weights: w.to_vec(),
        })
    }

    pub fn zeros() -> Self {
        Self {
            weights: alloc::vec![0.0; WEIGHT_COUNT],
        }
    }

    pub fn predict(&self, features: &FeatureVector) -> RepairAction {
        let x = features.as_slice();
        let mut h = [0.0f32; HIDDEN];
        for j in 0..HIDDEN {
            let mut s = self.w(INPUT * HIDDEN + j);
            for i in 0..INPUT {
                s += x[i] * self.w(i * HIDDEN + j);
            }
            h[j] = relu(s);
        }
        let o1 = INPUT * HIDDEN + HIDDEN;
        let mut z = [0.0f32; LATENT];
        for k in 0..LATENT {
            let mut s = self.w(o1 + HIDDEN * LATENT + k);
            for j in 0..HIDDEN {
                s += h[j] * self.w(o1 + j * LATENT + k);
            }
            z[k] = relu(s);
        }
        let o2 = o1 + HIDDEN * LATENT + LATENT;
        let mut logits = [0.0f32; OPS];
        for k in 0..OPS {
            let mut s = self.w(o2 + LATENT * OPS + k);
            for j in 0..LATENT {
                s += z[j] * self.w(o2 + j * OPS + k);
            }
            logits[k] = s;
        }
        let (idx, sm) = soft_argmax(&logits);
        let oc = o2 + LATENT * OPS + OPS;
        let mut cr = self.w(oc + LATENT);
        for j in 0..LATENT {
            cr += z[j] * self.w(oc + j);
        }
        let confidence = sigmoid(cr).max(sm * 0.5);
        let or = oc + LATENT + 1;
        let mut rr = self.w(or + LATENT);
        for j in 0..LATENT {
            rr += z[j] * self.w(or + j);
        }
        let risk = sigmoid(rr);

        let mut parameters = BTreeMap::new();
        parameters.insert(String::from("schema"), String::from("v2"));

        RepairAction {
            node_id: String::from("unlocalized"),
            repair_operator: OperatorId::from_u8(idx as u8),
            parameters,
            confidence,
            risk,
        }
    }

    #[inline]
    fn w(&self, i: usize) -> f32 {
        self.weights[i]
    }
}

fn relu(x: f32) -> f32 {
    if x > 0.0 {
        x
    } else {
        0.0
    }
}
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + libm::expf(-x))
}
fn soft_argmax(logits: &[f32; OPS]) -> (usize, f32) {
    let max = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let mut ex = [0.0f32; OPS];
    let mut sum = 0.0f32;
    for i in 0..OPS {
        let e = libm::expf(logits[i] - max);
        ex[i] = e;
        sum += e;
    }
    let mut bi = 0;
    let mut bp = 0.0f32;
    for i in 0..OPS {
        let p = if sum > 0.0 { ex[i] / sum } else { 0.0 };
        if p > bp {
            bp = p;
            bi = i;
        }
    }
    (bi, bp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zeros_predicts() {
        let net = RepairNet::zeros();
        let a = net.predict(&FeatureVector::zeros());
        assert!((0.0..=1.0).contains(&a.confidence));
        assert!((0.0..=1.0).contains(&a.risk));
    }

    #[test]
    fn weight_count_stable() {
        assert!(WEIGHT_COUNT > 1000);
    }
}
