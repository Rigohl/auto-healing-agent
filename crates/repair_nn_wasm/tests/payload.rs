//! El payload KV promovido (model/current.txt) debe cargar en la frontera
//! publica WASM: RepairModel::from_weights + UNA prediccion real. Si el
//! formato del artefacto diverge de la API publica, este test lo ve ANTES
//! del promote (docs/DEAD_CODE_AUDIT.md, item repair_nn_wasm).
//!
//! repair_nn_wasm deja asi de ser un canario puramente redundante: valida
//! que el artifact que vive en MODEL_KV:model/current es consumible por la
//! superficie documentada del proyecto.

use repair_nn_wasm::RepairModel;

#[test]
fn promoted_payload_loads_and_predicts() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../model/current.txt");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", path.display()));
    let weights: Vec<f32> = raw
        .split_whitespace()
        .map(|token| token.parse::<f32>().expect("peso f32"))
        .collect();
    assert_eq!(
        weights.len(),
        RepairModel::weight_count(),
        "el payload promovido debe tener exactamente WEIGHT_COUNT pesos"
    );
    let model = RepairModel::from_weights(&weights).expect("payload valido para from_weights");
    let features = vec![0.0f32; RepairModel::feature_dim()];
    let action = model
        .predict_from_features(&features)
        .expect("prediccion sobre features zeros");
    assert!((0.0..=1.0).contains(&action.confidence));
    assert!((0.0..=1.0).contains(&action.risk));
}
