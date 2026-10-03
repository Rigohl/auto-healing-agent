//! CLI del trainer V1: entrena y escribe el payload KV.
//!
//! ```bash
//! cargo run -p repair_train --release -- --out model/current.txt
//! ```
//!
//! El archivo producido es el formato EXACTO que `worker/src/worker/model.rs`
//! espera en `MODEL_KV` (clave `model/current`). Escribirlo en el repo NO lo
//! promueve: subir los pesos a KV es una accion humana (`docs/GOVERNANCE.md`).

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

use repair_train::{evaluate, export_payload, train, TrainConfig};

/// Umbral V1: un payload que no clasifica al 90% no se promueve.
const MIN_ARTIFACT_ACCURACY: f32 = 0.90;

fn main() {
    let mut out = PathBuf::from("model/current.txt");
    let args: Vec<String> = env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--out" && i + 1 < args.len() {
            out = PathBuf::from(&args[i + 1]);
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

    let weights = train(&config);
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

    fs::write(&out, &payload).unwrap_or_else(|e| {
        eprintln!("no se pudo escribir {}: {e}", out.display());
        process::exit(1);
    });
    println!(
        "payload KV escrito en {} ({} pesos)",
        out.display(),
        weights.len()
    );

    if train_m.accuracy < MIN_ARTIFACT_ACCURACY || holdout_m.accuracy < MIN_ARTIFACT_ACCURACY {
        eprintln!(
            "accuracy por debajo del umbral V1 ({MIN_ARTIFACT_ACCURACY}): no promover este payload"
        );
        process::exit(1);
    }
}

fn print_usage() {
    eprintln!("uso: repair-train [--out <ruta>]");
    eprintln!("  entrena con la config V1 y escribe el payload KV (WEIGHT_COUNT f32 en texto)");
}
