//! Aprendizaje desde casos REALES (CONTRATO PART4): el worker persiste un
//! TrainingExample por reparacion (queue_consumer::persist_case, clave
//! training_example:{correlation_id} en REPAIR_CASES_KV) con las features
//! congeladas del momento del incidente; /github/callback lo marca verified
//! y le asigna el reward REAL de Actions. Este modulo consume ese JSONL
//! exportado (promote-model.yml, flag export_examples): carga fail-closed,
//! entrena mezclando sinteticos + reales con el MISMO nucleo SGD (crate::sgd)
//! y evalua sobre los reales con el RepairNet::predict de produccion.
//! Sin LLM: los ejemplos solo agregan senal verificada, nunca inventan labels.

use feature_engine::extract;
use feature_engine::synthetic::generate_synthetic_dataset;
use repair_nn_core::RepairNet;
use repair_types::{FeatureVector, TrainingExample, OPERATOR_COUNT};

use crate::{sgd, TrainConfig};

/// Veces que repite cada ejemplo verificado en el dataset mixto: la senal
/// real pesa mas que un sample sintetico, pero sin ahogar al sintetico.
pub const EXAMPLE_REPEAT: usize = 3;

/// Carga un JSONL de TrainingExample (export de REPAIR_CASES_KV).
/// Fail-closed por linea: dimension != 64, valor no finito u operador fuera
/// de rango invalidan el archivo entero (misma regla que load_payload).
pub fn load_examples(jsonl: &str) -> Result<Vec<TrainingExample>, String> {
    let mut out = Vec::new();
    for (i, line) in jsonl.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let ex: TrainingExample = serde_json::from_str(trimmed)
            .map_err(|e| format!("linea {}: JSON invalido: {}", i + 1, e))?;
        if ex.features.len() != FeatureVector::DIM {
            return Err(format!(
                "linea {}: features.len() = {}, se esperaban {}",
                i + 1,
                ex.features.len(),
                FeatureVector::DIM
            ));
        }
        if ex.features.iter().any(|v| !v.is_finite()) {
            return Err(format!("linea {}: feature no finita", i + 1));
        }
        if ex.operator as usize >= OPERATOR_COUNT {
            return Err(format!(
                "linea {}: operador {} fuera de rango",
                i + 1,
                ex.operator
            ));
        }
        out.push(ex);
    }
    Ok(out)
}

/// Solo la senal VERIFICADA positiva entrena (verified + reward > 0): un
/// caso FAIL o pendiente de VERIFY nunca ensena su etiqueta como buena.
pub fn usable(ex: &TrainingExample) -> Option<(FeatureVector, usize)> {
    if !ex.verified || ex.reward <= 0.0 {
        return None;
    }
    let mut fv = FeatureVector::zeros();
    fv.values.copy_from_slice(&ex.features);
    Some((fv, ex.operator as usize))
}

/// Entrena con el dataset sintetico de config MAS los ejemplos reales
/// verificables (repetidos EXAMPLE_REPEAT veces). Determinista: misma
/// config + mismos ejemplos => misma red.
pub fn train_with_examples(
    config: &TrainConfig,
    examples: &[TrainingExample],
) -> Result<Vec<f32>, String> {
    assert!(
        config.samples > 0,
        "train_with_examples necesita samples > 0"
    );
    let dataset = generate_synthetic_dataset(config.samples, config.dataset_seed);
    let mut features: Vec<FeatureVector> = dataset
        .iter()
        .map(|s| extract(&s.incident, &s.signature))
        .collect();
    let mut labels: Vec<usize> = dataset
        .iter()
        .map(|s| s.ground_truth_operator as usize)
        .collect();
    let mut usable_count = 0usize;
    for ex in examples {
        if let Some((fv, op)) = usable(ex) {
            for _ in 0..EXAMPLE_REPEAT {
                features.push(fv);
                labels.push(op);
            }
            usable_count += 1;
        }
    }
    if usable_count == 0 {
        return Err(String::from(
            "ningun ejemplo verificado utilizable (verified=true y reward>0)",
        ));
    }
    Ok(sgd(config, features, labels))
}

/// Accuracy sobre los ejemplos reales usando el predictor de produccion
/// (RepairNet::predict), no una copia del forward.
pub fn evaluate_examples(weights: &[f32], examples: &[TrainingExample]) -> Result<f32, &'static str> {
    let net = RepairNet::from_weights(weights)?;
    let mut correct = 0usize;
    let mut total = 0usize;
    for ex in examples {
        if let Some((fv, op)) = usable(ex) {
            total += 1;
            let action = net.predict(&fv);
            if action.repair_operator as usize == op {
                correct += 1;
            }
        }
    }
    if total == 0 {
        return Err("sin ejemplos verificables para evaluar");
    }
    Ok(correct as f32 / total as f32)
}
