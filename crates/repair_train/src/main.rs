//! CLI del trainer V1: entrena y escribe el payload KV.
//!
//! cargo run -p repair_train --release -- --out model/current.txt
//!
//! Aprendizaje real (CONTRATO PART4): con --examples <ruta.jsonl> mezcla el
//! dataset sintetico con los TrainingExample verificados exportados de
//! REPAIR_CASES_KV (promote-model.yml, flag export_examples). Solo la
//! senal verificada positiva (verified + reward > 0) entra al entrenamiento.
//!
//! El archivo producido es el formato EXACTO que worker/src/worker/model.rs
//! espera en MODEL_KV (clave model/current). Escribirlo en el repo NO lo
//! promueve: subir los pesos a KV es una accion humana (docs/GOVERNANCE.md).

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

use repair_train::{evaluate, export_payload, train, TrainConfig};

/// Umbral V1: un payload que no clasifica al 90% no se promueve.
const MIN_ARTIFACT_ACCURACY: f32 = 0.90;

fn main() {
    let mut out = PathBuf::from("model/current.txt");
    let mut examples: Option<PathBuf> = None;
    let args: Vec<String> = env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--out" && i + 1 < args.len() {
            out = PathBuf::from(&args[i + 1]);
            i += 2;
        } else if args[i] == "--examples" && i + 1 < args.len() {
            examples = Some(PathBuf::from(&args[i + 1]));
            i += 2;
        } else if args[i] == "--help" || args[i] == "-h" {
            print_usage();
            return;
        } else {
            eprintln!("argumento no reconocido: {}", args[i]);
            print_usage();
            process::exit(2);
        }
    }

    let config = TrainConfig::default();
    eprintln!(
        "entrenando V1: {} muestras (seed {}), {} epocas, batch {}, lr {} -> {}",
        config.samples,
        config.dataset_seed,
        config.epochs,
        config.batch,
        config.lr,
        config.lr_final
    );

    // Aprendizaje real: con --examples se mezcla el dataset sintetico con
    // los casos reales verificados; sin el flag, entrenamiento sintetico
    // puro (V1). El nucleo SGD es el mismo en ambos caminos.
    let (weights, source) = match &examples {
        Some(path) => {
            let raw = fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("no se pudo leer {}: {e}", path.display());
                process::exit(1);
            });
            let loaded = repair_train::examples::load_examples(&raw).unwrap_or_else(|e| {
                eprintln!("ejemplos invalidos en {}: {e}", path.display());
                process::exit(1);
            });
            let usable = loaded
                .iter()
                .filter(|ex| repair_train::examples::usable(ex).is_some())
                .count();
            eprintln!(
                "ejemplos reales: {} cargados, {} verificables",
                loaded.len(),
                usable
            );
            let w = repair_train::examples::train_with_examples(&config, &loaded)
                .expect("entrenamiento mixto produce WEIGHT_COUNT pesos");
            (w, String::from("mixed: synthetic + real verified examples"))
        }
        None => (train(&config), String::from("synthetic only")),
    };
    eprintln!("dataset: {source}");
    let payload = export_payload(&weights).expect("train produce WEIGHT_COUNT pesos");

    let train_m =
        evaluate(&weights, config.samples, config.dataset_seed).expect("pesos recien entrenados");
    let holdout_m = evaluate(&weights, 500, 43).expect("pesos recien entrenados");

    println!(
        "train    acc={:.4} actionable={:.4} conf={:.4} risk={:.5}",
        train_m.accuracy, train_m.actionable, train_m.mean_confidence, train_m.mean_risk
    );
    println!(
        "holdout  acc={:.4} actionable={:.4} conf={:.4} risk={:.5}",
        holdout_m.accuracy, holdout_m.actionable, holdout_m.mean_confidence, holdout_m.mean_risk
    );

    // Senal real: accuracy sobre los propios ejemplos verificables, con el
    // predictor de produccion. Solo informativo: el umbral de promocion
    // sigue siendo el sintetico (canary de regression.yml).
    if let Some(path) = &examples {
        if let Ok(raw) = fs::read_to_string(path) {
            if let Ok(loaded) = repair_train::examples::load_examples(&raw) {
                if let Ok(acc) = repair_train::examples::evaluate_examples(&weights, &loaded) {
                    println!("examples acc={:.4} (senal verificada)", acc);
                }
            }
        }
    }

    // BUG-05: validar ANTES de escribir. El orden anterior escribia el
    // artefacto en model/current.txt y solo entonces comprobaba el umbral:
    // un re-entrenamiento degradado dejaba en disco (listo para un git add)
    // un payload que el propio comando declaraba no promovible.
    if train_m.accuracy < MIN_ARTIFACT_ACCURACY || holdout_m.accuracy < MIN_ARTIFACT_ACCURACY {
        eprintln!(
            "accuracy por debajo del umbral V1 ({MIN_ARTIFACT_ACCURACY}): no se escribe {}",
            out.display()
        );
        process::exit(1);
    }

    fs::write(&out, &payload).unwrap_or_else(|e| {
        eprintln!("no se pudo escribir {}: {e}", out.display());
        process::exit(1);
    });
    println!(
        "payload KV escrito en {} ({} pesos, dataset: {source})",
        out.display(),
        weights.len()
    );
}

fn print_usage() {
    eprintln!("uso: repair-train [--out <ruta>] [--examples <training_examples.jsonl>]");
    eprintln!("  entrena con la config V1 y escribe el payload KV (WEIGHT_COUNT f32 en texto)");
    eprintln!("  --examples: mezcla casos reales verificados (export de REPAIR_CASES_KV) con el sintetico");
}
