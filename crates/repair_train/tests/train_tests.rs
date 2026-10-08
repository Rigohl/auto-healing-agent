//! Tests del trainer V1: convergencia, artefacto comprometido y espejo del
//! loader del worker. El test del artefacto es el que mantiene honesto al
//! resto: si model/current.txt deja de cargar o de clasificar, CI lo dice.
//! Los tests de ejemplos reales validan el loop de aprendizaje PART4 con la
//! MISMA forma que persiste el worker (training_example:{id}).

use feature_engine::extract;
use feature_engine::synthetic::generate_synthetic_dataset;
use repair_nn_core::RepairNet;
use repair_train::{
    evaluate, export_payload, load_payload, train, TrainConfig, LAMBDA_CONF, LAMBDA_RISK, MAX_RISK,
    MIN_CONFIDENCE, WEIGHT_COUNT,
};
use repair_types::OperatorId;

/// El payload comprometido en el repo, tal como lo leeria el runner de CI.
const ARTIFACT: &str = include_str!("../../../model/current.txt");

/// Config corta: misma forma que la V1, escala de test (corre en debug).
fn short_config() -> TrainConfig {
    TrainConfig {
        samples: 240,
        dataset_seed: 42,
        epochs: 25,
        batch: 16,
        lr: 0.1,
        lr_final: 0.03,
        init_seed: 7,
    }
}

#[test]
fn short_train_converges() {
    let weights = train(&short_config());
    let metrics = evaluate(&weights, 240, 42).expect("pesos recien entrenados");
    assert!(
        metrics.accuracy >= 0.70,
        "accuracy {:.3} por debajo del umbral de convergencia",
        metrics.accuracy
    );
}

#[test]
fn lambda_is_pinned() {
    // El gap de INVENTORY era "lambda sin fijar": estos valores son parte del
    // contrato V1 y no pueden derivar en silencio.
    assert_eq!(LAMBDA_CONF, 0.5);
    assert_eq!(LAMBDA_RISK, 0.5);
}

#[test]
fn committed_artifact_loads_and_classifies() {
    let weights = load_payload(ARTIFACT).expect("model/current.txt debe ser un payload KV valido");
    assert_eq!(weights.len(), WEIGHT_COUNT);

    let train_m = evaluate(&weights, 1000, 42).expect("pesos validos");
    let holdout_m = evaluate(&weights, 500, 43).expect("pesos validos");
    assert!(
        train_m.accuracy >= 0.90,
        "accuracy train {:.3} por debajo del umbral V1",
        train_m.accuracy
    );
    assert!(
        holdout_m.accuracy >= 0.90,
        "accuracy holdout {:.3} por debajo del umbral V1",
        holdout_m.accuracy
    );
    assert!(
        train_m.actionable >= 0.85,
        "actionable {:.3} por debajo del umbral V1",
        train_m.actionable
    );
}

#[test]
fn committed_artifact_passes_production_gate() {
    // Camino REAL de produccion: RepairNet::predict + is_actionable con los
    // umbrales del worker (0.55 / 0.45). Sin esto, el artefacto clasificaria
    // bien en el test pero el worker lo rechazaria en el gate.
    let weights = load_payload(ARTIFACT).expect("payload valido");
    let net = RepairNet::from_weights(&weights).expect("longitud exacta");
    let dataset = generate_synthetic_dataset(50, 42);
    for item in &dataset {
        let fv = extract(&item.incident, &item.signature);
        let action = net.predict(&fv);
        assert_eq!(
            action.repair_operator,
            OperatorId::from_u8(item.ground_truth_operator),
            "operador incorrecto para {}",
            item.error_category
        );
        assert!(
            action.is_actionable(MIN_CONFIDENCE, MAX_RISK),
            "gate rechazado: confidence {} / risk {}",
            action.confidence,
            action.risk
        );
    }
}

#[test]
fn export_load_roundtrip_is_exact() {
    let weights = train(&short_config());
    let payload = export_payload(&weights).expect("el trainer produce WEIGHT_COUNT pesos");
    let reloaded = load_payload(&payload).expect("el payload exportado debe recargar");
    for (i, (a, b)) in weights.iter().zip(reloaded.iter()).enumerate() {
        assert!(
            a == b || (a.is_nan() && b.is_nan()),
            "roundtrip perdio el peso {i}: {a} != {b}"
        );
    }
}

#[test]
fn load_payload_rejects_malformed() {
    // Espejo de las reglas del worker: vacio, longitud incorrecta, token no
    // numerico y token no finito invalidan el payload entero.
    assert!(load_payload("").is_err());
    assert!(load_payload("   ").is_err());

    let too_few: String = (0..WEIGHT_COUNT - 1).map(|i| format!("{i}\n")).collect();
    assert!(load_payload(&too_few).is_err());

    let too_many: String = (0..WEIGHT_COUNT + 1).map(|i| format!("{i}\n")).collect();
    assert!(load_payload(&too_many).is_err());

    let zeros = "0.0\n".repeat(WEIGHT_COUNT);
    assert!(load_payload(&format!("{zeros}nan")).is_err());
    assert!(load_payload(&format!("{zeros}banana")).is_err());

    let valid = vec![0.5f32; WEIGHT_COUNT];
    let valid_payload = export_payload(&valid).expect("longitud exacta");
    assert!(load_payload(&valid_payload).is_ok());
}

#[test]
fn real_examples_close_the_learning_loop() {
    // Loop PART4 de punta a punta con la MISMA forma que persiste el worker
    // (queue_consumer::persist_case -> training_example:{id}): features del
    // momento del incidente + verificacion real (verified=true, reward=1).
    let dataset = generate_synthetic_dataset(60, 43);
    let examples: Vec<repair_types::TrainingExample> = dataset
        .iter()
        .map(|s| repair_types::TrainingExample {
            features: extract(&s.incident, &s.signature).values.to_vec(),
            operator: s.ground_truth_operator,
            node_id: String::from("node-test"),
            reward: 1.0,
            verified: true,
        })
        .collect();
    let jsonl = examples
        .iter()
        .map(|e| serde_json::to_string(e).expect("serializable"))
        .collect::<Vec<_>>()
        .join("\n");
    let loaded = repair_train::examples::load_examples(&jsonl).expect("jsonl valido");
    assert_eq!(loaded.len(), 60);

    let weights = repair_train::examples::train_with_examples(&short_config(), &loaded)
        .expect("entrenamiento mixto");
    let acc = repair_train::examples::evaluate_examples(&weights, &loaded)
        .expect("evaluar sobre los ejemplos reales");
    assert!(
        acc >= 0.70,
        "accuracy sobre ejemplos reales {:.3} por debajo de 0.70",
        acc
    );
}

#[test]
fn unverified_examples_never_train() {
    let dataset = generate_synthetic_dataset(10, 44);
    let unverified: Vec<repair_types::TrainingExample> = dataset
        .iter()
        .map(|s| repair_types::TrainingExample {
            features: extract(&s.incident, &s.signature).values.to_vec(),
            operator: s.ground_truth_operator,
            node_id: String::from("node-test"),
            reward: 0.0,
            verified: false,
        })
        .collect();
    // Sin senal verificada el entrenamiento mixto se niega: nunca se
    // aprende una etiqueta que Actions no respalde.
    assert!(repair_train::examples::train_with_examples(&short_config(), &unverified).is_err());
    // usable() tambien rechaza reward negativo (FAIL verificado).
    let mut fail = unverified[0].clone();
    fail.verified = true;
    fail.reward = -1.0;
    assert!(repair_train::examples::usable(&fail).is_none());
    assert!(repair_train::examples::usable(&unverified[0]).is_none());
}

#[test]
fn load_examples_is_fail_closed() {
    use repair_train::examples::load_examples;

    // Vacio = cero ejemplos (no es error: el export puede venir sin senal).
    assert!(load_examples("").expect("jsonl vacio").is_empty());

    // JSON corrupto.
    assert!(load_examples("no es json").is_err());

    // Dimension incorrecta: 63 features en vez de 64.
    let dim_bad = serde_json::json!({
        "features": vec![0.0f32; 63],
        "operator": 1,
        "node_id": "n",
        "reward": 1.0,
        "verified": true
    });
    assert!(load_examples(&dim_bad.to_string()).is_err());

    // Operador fuera de rango (>= OPERATOR_COUNT).
    let op_bad = serde_json::json!({
        "features": vec![0.0f32; 64],
        "operator": 13,
        "node_id": "n",
        "reward": 1.0,
        "verified": true
    });
    assert!(load_examples(&op_bad.to_string()).is_err());
}
