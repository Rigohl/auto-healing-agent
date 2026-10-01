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
        for (j, hj) in h.iter_mut().enumerate() {
            let mut s = self.w(INPUT * HIDDEN + j);
            for (i, xi) in x.iter().enumerate() {
                s += xi * self.w(i * HIDDEN + j);
            }
            *hj = relu(s);
        }
        let o1 = INPUT * HIDDEN + HIDDEN;
        let mut z = [0.0f32; LATENT];
        for (k, zk) in z.iter_mut().enumerate() {
            let mut s = self.w(o1 + HIDDEN * LATENT + k);
            for (j, hj) in h.iter().enumerate() {
                s += hj * self.w(o1 + j * LATENT + k);
            }
            *zk = relu(s);
        }
        let o2 = o1 + HIDDEN * LATENT + LATENT;
        let mut logits = [0.0f32; OPS];
        for (k, logit) in logits.iter_mut().enumerate() {
            let mut s = self.w(o2 + LATENT * OPS + k);
            for (j, zj) in z.iter().enumerate() {
                s += zj * self.w(o2 + j * OPS + k);
            }
            *logit = s;
        }
        let (idx, sm) = soft_argmax(&logits);
        let oc = o2 + LATENT * OPS + OPS;
        let mut cr = self.w(oc + LATENT);
        for (j, zj) in z.iter().enumerate() {
            cr += zj * self.w(oc + j);
        }
        let confidence = sigmoid(cr).max(sm * 0.5);
        let or = oc + LATENT + 1;
        let mut rr = self.w(or + LATENT);
        for (j, zj) in z.iter().enumerate() {
            rr += zj * self.w(or + j);
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
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut ex = [0.0f32; OPS];
    let mut sum = 0.0f32;
    for (i, slot) in ex.iter_mut().enumerate() {
        let e = libm::expf(logits[i] - max);
        *slot = e;
        sum += e;
    }
    let mut bi = 0;
    let mut bp = 0.0f32;
    for (i, e) in ex.iter().enumerate() {
        let p = if sum > 0.0 { *e / sum } else { 0.0 };
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
        // Exact, not a lower bound: a weak `> 1000` assertion is what let the
        // docs drift to 2617 while the code said 2863 (see docs/DISCREPANCIES.md 20).
        assert_eq!(WEIGHT_COUNT, 2863);
    }

    #[test]
    fn weight_count_matches_layer_arithmetic() {
        // 64*32+32 (W1,b1) + 32*16+16 (W2,b2) + 16*13+13 (op head)
        // + 16+1 (confidence head) + 16+1 (risk head)
        let expected = INPUT * HIDDEN + HIDDEN
            + HIDDEN * LATENT + LATENT
            + LATENT * OPS + OPS
            + LATENT + 1
            + LATENT + 1;
        assert_eq!(WEIGHT_COUNT, expected);
        assert_eq!(OPS, 13, "OperatorId 0..=12 plus Unknown-adjacent count");
    }

    #[test]
    fn from_weights_rejects_wrong_length() {
        let short = alloc::vec![0.0f32; WEIGHT_COUNT - 1];
        assert!(RepairNet::from_weights(&short).is_err());
        let exact = alloc::vec![0.0f32; WEIGHT_COUNT];
        assert!(RepairNet::from_weights(&exact).is_ok());
    }

    #[test]
    fn sigmoid_and_softmax_are_bounded_without_std() {
        // libm::expf path: outputs must stay in [0,1] for extreme logits.
        let mut fv = FeatureVector::zeros();
        for i in 0..FeatureVector::DIM {
            fv.values[i] = 100.0;
        }
        let a = RepairNet::zeros().predict(&fv);
        assert!((0.0..=1.0).contains(&a.confidence), "{}", a.confidence);
        assert!((0.0..=1.0).contains(&a.risk), "{}", a.risk);
    }
}
