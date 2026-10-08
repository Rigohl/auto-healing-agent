//! Frontera REAL JS<->WASM para los exports publicos del crate. Antes solo
//! from_weights tenia consumidor (tests/payload_check.rs); con este test
//! los exports que veria un consumidor JS (weightCount, featureDim,
//! predictFromFeatures, operator_name) se EJERCEN de verdad bajo Node
//! (wasm-pack test --node, job node-boundary de wasm.yml), no solo se
//! compilan. Fail-closed: dimension incorrecta rechazada, nunca defaults.
//!
//! Solo corre en target wasm32 (cfg de archivo): bajo cargo test nativo el
//! archivo compila a nada, asi que el job workspace-* no lo duplica.

#![cfg(target_arch = "wasm32")]

use repair_nn_wasm::RepairModel;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

// El modo de ejecucion (node) lo elige `wasm-pack test --node`; el macro
// de wasm-bindgen-test 0.2.50 solo acepta run_in_browser.
wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn dims_and_weight_count_are_exported() {
    assert_eq!(RepairModel::feature_dim(), 64);
    assert_eq!(RepairModel::weight_count(), repair_nn_core::WEIGHT_COUNT);
}

#[wasm_bindgen_test]
fn from_weights_rejects_wrong_length() {
    assert!(RepairModel::from_weights(&[0.0f32; 8]).is_err());
}

#[wasm_bindgen_test]
fn predict_from_features_validates_dim() {
    let zeros = vec![0.0f32; repair_nn_core::WEIGHT_COUNT];
    let model = RepairModel::from_weights(&zeros).expect("red de zeros carga");
    // Dimension incorrecta: rechazada, nunca rellenada con ceros.
    assert!(model.predict_from_features(&[0.0f32; 32]).is_err());
}

#[wasm_bindgen_test]
fn predict_from_features_returns_action() {
    let zeros = vec![0.0f32; repair_nn_core::WEIGHT_COUNT];
    let model = RepairModel::from_weights(&zeros).expect("red de zeros carga");
    let action = model
        .predict_from_features(&[0.0f32; 64])
        .expect("dimension exacta");
    // Red de zeros: probs uniformes => operador 0 (NO_OP). Lo importante es
    // que la accion cruza la frontera con nombre estable, no el valor.
    assert_eq!(action.operator_name(), "NO_OP");
}
