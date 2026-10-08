//! Frontera JS real del adaptador WASM, ejecutada en Node via
//! wasm-bindgen-test (wasm.yml: "wasm-pack test --node"): ejercita los
//! CINCO exports publicos (fromWeights, predictFromFeatures, weightCount,
//! featureDim, operator_name) con el payload KV comprometido en
//! model/current.txt. Es el consumidor real de la superficie publica JS
//! del crate (auditoria DEAD_CODE 2026-10-08).

use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_node);

/// El payload KV del repo, tal como lo subiria promote-model.yml.
const PAYLOAD: &str = include_str!("../../../model/current.txt");

fn committed_weights() -> Vec<f32> {
    PAYLOAD
        .split_whitespace()
        .map(|t| t.parse::<f32>().expect("payload KV: token f32"))
        .collect()
}

#[wasm_bindgen_test]
fn exports_match_the_committed_payload() {
    let weights = committed_weights();
    assert_eq!(
        weights.len(),
        repair_nn_wasm::RepairModel::weight_count(),
        "model/current.txt debe tener WEIGHT_COUNT tokens"
    );
    assert_eq!(repair_nn_wasm::RepairModel::feature_dim(), 64);

    let model = repair_nn_wasm::RepairModel::from_weights(&weights)
        .expect("el payload comprometido cruza la frontera wasm-bindgen");

    // Vector con el bias del slot 63 activado: mismo formato que produce
    // feature_engine::extract con todas las senales en 0.
    let mut features = vec![0.0f32; 64];
    features[63] = 1.0;
    let action = model
        .predict_from_features(&features)
        .expect("inferencia con el payload comprometido");
    assert!(action.confidence.is_finite());
    assert!(action.risk.is_finite());
    assert!(
        !action.operator_name().is_empty(),
        "operator_name nunca devuelve vacio"
    );
}

#[wasm_bindgen_test]
fn wrong_feature_dim_is_rejected_fail_closed() {
    let weights = vec![0.0f32; repair_nn_wasm::RepairModel::weight_count()];
    let model = repair_nn_wasm::RepairModel::from_weights(&weights).expect("longitud exacta");
    assert!(model.predict_from_features(&[0.0; 63]).is_err());
    assert!(model.predict_from_features(&[0.0; 65]).is_err());
}
