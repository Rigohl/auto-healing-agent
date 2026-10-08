#![cfg(not(target_arch = "wasm32"))]
//! El artefacto `model/current.txt` debe cargar a traves de la frontera
//! wasm-bindgen de esta crate.
//!
//! Rol del payload check: `repair_nn_wasm` es la superficie publica JS del
//! modelo; este test la usa para validar el artefacto KV REAL que consume
//! el worker (`MODEL_KV:model/current`). Si alguien cambia WEIGHT_COUNT o
//! el formato del payload sin regenerarlo, CI se pone rojo ANTES de que el
//! promote suba un modelo que el runtime rechazaria (blocked_no_model).
//!
//! Gate wasm32: este test lee el sistema de archivos del host
//! (CARGO_MANIFEST_DIR + std::fs), que NO existe bajo wasm32. El job
//! node-boundary (wasm-pack test --node) compila TODOS los integration
//! tests a wasm32; sin este gate el archivo romperia esa frontera. La
//! carga del artefacto a traves de la frontera wasm ya la ejerce
//! tests/js_boundary.rs (from_weights sobre el payload real).

use repair_nn_wasm::RepairModel;

#[test]
fn current_payload_loads_across_wasm_boundary() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("model")
        .join("current.txt");
    let raw = std::fs::read_to_string(&path).expect("model/current.txt debe existir");
    let weights: Vec<f32> = raw
        .split_whitespace()
        .map(|token| {
            token
                .parse::<f32>()
                .expect("token f32 valido en el payload")
        })
        .collect();
    assert_eq!(
        weights.len(),
        repair_nn_core::WEIGHT_COUNT,
        "el payload debe tener exactamente WEIGHT_COUNT pesos"
    );
    RepairModel::from_weights(&weights).expect("from_weights acepta el artefacto");
}
