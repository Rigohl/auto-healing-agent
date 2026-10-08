//! Trainer offline V1 para la MLP de `repair_nn_core` (64→32→16).
//!
//! Cierra el gap "offline train / export" de `docs/E2E_CHECKLIST.md`: entrena
//! la red sobre el dataset sintetico determinista (`feature_engine::synthetic`)
//! y exporta los pesos en el formato EXACTO que `worker/src/worker/model.rs`
//! carga desde `MODEL_KV`: `WEIGHT_COUNT` valores `f32` legibles separados por
//! whitespace. Sin LLM y sin crate `rand`: el dataset ya es determinista y el
//! trainer solo anade aritmetica pura (LCG propio para init y shuffle).
//!
//! Objetivo V1 (λ queda FIJADA aqui; cierra el gap de λ de
//! `docs/INVENTORY.md`):
//!
//! ```text
//! loss = CE(logits, operador_gt)
//!      + λ_conf · BCE(sigmoid(critic_conf), pred==gt)
//!      + λ_risk · BCE(sigmoid(critic_risk), pred!=gt)
//! ```
//!
//! con `λ_conf = λ_risk = 0.5`: la cabeza de confianza aprende a reconocer sus
//! aciertos y la de riesgo sus fallos, en vez de declarar confianza constante.
//! El gate de produccion (`0.55 / 0.45`) NO se toca: GitHub Actions sigue
//! siendo la unica autoridad de VERIFY (`docs/NO_LLM_POLICY.md`).
//!
//! El artefacto comprometido en `model/current.txt` se valida en CI con el
//! `extract` y `RepairNet::predict` reales (ver `tests/train_tests.rs`).

use feature_engine::extract;
use feature_engine::synthetic::generate_synthetic_dataset;
use repair_nn_core::RepairNet;
use repair_types::{FeatureVector, OperatorId, TrainingExample, OPERATOR_COUNT};

/// Re-export: los callers del trainer no deberian depender del core solo
/// para conocer la longitud del payload.
pub use repair_nn_core::WEIGHT_COUNT;

/// Peso de la BCE de la cabeza de confianza (fijado en V1).
pub const LAMBDA_CONF: f32 = 0.5;
/// Peso de la BCE de la cabeza de riesgo (fijado en V1).
pub const LAMBDA_RISK: f32 = 0.5;
/// Umbral de gate que replica el worker (`worker/src/worker/mod.rs`).
pub const MIN_CONFIDENCE: f32 = 0.55;
/// Umbral de gate que replica el worker (`worker/src/worker/mod.rs`).
pub const MAX_RISK: f32 = 0.45;

const INPUT: usize = 64;
const HIDDEN: usize = 32;
const LATENT: usize = 16;
const OPS: usize = OPERATOR_COUNT;

// Layout plano, espejo exacto de `repair_nn_core::RepairNet::predict`.
const OFF_B1: usize = INPUT * HIDDEN;
const OFF_W2: usize = OFF_B1 + HIDDEN;
const OFF_B2: usize = OFF_W2 + HIDDEN * LATENT;
const OFF_WO: usize = OFF_B2 + LATENT;
const OFF_BO: usize = OFF_WO + LATENT * OPS;
const OFF_WC: usize = OFF_BO + OPS;
const OFF_BC: usize = OFF_WC + LATENT;
const OFF_WR: usize = OFF_BC + 1;
const OFF_BR: usize = OFF_WR + LATENT;

// El layout del trainer tiene que ser el del core, no "parecido": la misma
// regla de `weight_count_stable` en repair_nn_core, exacta y no una cota.
const _: () = assert!(OFF_BR + 1 == WEIGHT_COUNT);

/// LCG determinista u32 para el init de pesos y el shuffle de batches.
struct Rng(u32);

impl Rng {
    /// Uniforme en [0, 1): 24 bits de mantisa, sin redondeo hacia 1.0.
    fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Hiperparametros V1. Los defaults son los de la corrida que produjo
/// `model/current.txt` (dataset 1000 / seed 42, 120 epocas, batch 16,
/// lr 0.1 -> 0.03 al 70% de las epocas, init seed 7). Re-entrenar con ellos
/// produce una red equivalente, no bytes identicos: la aritmetica intermedia
/// del runtime puede divergir en los ultimos bits. El contrato real es el
/// artefacto comprometido, validado directo en CI (`tests/train_tests.rs`).
#[derive(Debug, Clone, Copy)]
pub struct TrainConfig {
    pub samples: usize,
    pub dataset_seed: u64,
    pub epochs: usize,
    pub batch: usize,
    pub lr: f32,
    pub lr_final: f32,
    pub init_seed: u32,
}

impl Default for TrainConfig {
    fn default() -> Self {
        Self {
            samples: 1000,
            dataset_seed: 42,
            epochs: 120,
            batch: 16,
            lr: 0.1,
            lr_final: 0.03,
            init_seed: 7,
        }
    }
}

/// Metricas de evaluacion sobre el dataset sintetico.
#[derive(Debug, Clone, Copy)]
pub struct Metrics {
    pub samples: usize,
    pub accuracy: f32,
    pub actionable: f32,
    pub mean_confidence: f32,
    pub mean_risk: f32,
}

/// Intermedios del forward que el backward necesita (mascaras ReLU incluidas).
struct Forward {
    h: [f32; HIDDEN],
    z: [f32; LATENT],
    probs: [f32; OPS],
    pred: usize,
    conf: f32,
    risk: f32,
}

fn relu(x: f32) -> f32 {
    if x > 0.0 {
        x
    } else {
        0.0
    }
}

fn forward(w: &[f32], x: &FeatureVector) -> Forward {
    let mut s1 = [0.0f32; HIDDEN];
    s1.copy_from_slice(&w[OFF_B1..OFF_B1 + HIDDEN]);
    for (i, &xv) in x.values.iter().enumerate() {
        if xv == 0.0 {
            continue;
        }
        for (j, s) in s1.iter_mut().enumerate() {
            *s += xv * w[i * HIDDEN + j];
        }
    }
    let h = s1.map(relu);

    let mut s2 = [0.0f32; LATENT];
    s2.copy_from_slice(&w[OFF_B2..OFF_B2 + LATENT]);
    for (j, &hv) in h.iter().enumerate() {
        if hv == 0.0 {
            continue;
        }
        for (k, s) in s2.iter_mut().enumerate() {
            *s += hv * w[OFF_W2 + j * LATENT + k];
        }
    }
    let z = s2.map(relu);

    let mut logits = [0.0f32; OPS];
    logits.copy_from_slice(&w[OFF_BO..OFF_BO + OPS]);
    for (j, &zv) in z.iter().enumerate() {
        if zv == 0.0 {
            continue;
        }
        for (c, l) in logits.iter_mut().enumerate() {
            *l += zv * w[OFF_WO + j * OPS + c];
        }
    }

    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let ex = logits.map(|l| (l - max).exp());
    let sum: f32 = ex.iter().sum();
    let probs = ex.map(|e| e / sum);

    let mut pred = 0;
    let mut best = 0.0f32;
    for (c, &pv) in probs.iter().enumerate() {
        if pv > best {
            best = pv;
            pred = c;
        }
    }

    let cr = w[OFF_BC]
        + z.iter()
            .enumerate()
            .map(|(j, &zv)| zv * w[OFF_WC + j])
            .sum::<f32>();
    let conf = 1.0 / (1.0 + (-cr).exp());
    let rr = w[OFF_BR]
        + z.iter()
            .enumerate()
            .map(|(j, &zv)| zv * w[OFF_WR + j])
            .sum::<f32>();
    let risk = 1.0 / (1.0 + (-rr).exp());

    Forward {
        h,
        z,
        probs,
        pred,
        conf,
        risk,
    }
}

/// Acumula en `g` el gradiente de una muestra (loss V1, ver doc del crate).
fn backward(fwd: &Forward, gt: usize, w: &[f32], g: &mut [f32], x: &FeatureVector) {
    let mut dl = [0.0f32; OPS];
    for (c, d) in dl.iter_mut().enumerate() {
        *d = fwd.probs[c] - if c == gt { 1.0 } else { 0.0 };
    }
    let correct = if fwd.pred == gt { 1.0 } else { 0.0 };
    let dcr = LAMBDA_CONF * (fwd.conf - correct);
    let drr = LAMBDA_RISK * (fwd.risk - (1.0 - correct));

    let mut dz = [0.0f32; LATENT];
    for (j, dzv) in dz.iter_mut().enumerate() {
        let mut acc = 0.0;
        for (c, &d) in dl.iter().enumerate() {
            acc += w[OFF_WO + j * OPS + c] * d;
        }
        *dzv = acc + w[OFF_WC + j] * dcr + w[OFF_WR + j] * drr;
    }

    let mut ds2 = [0.0f32; LATENT];
    for ((ds, &dzv), &zv) in ds2.iter_mut().zip(dz.iter()).zip(fwd.z.iter()) {
        *ds = if zv > 0.0 { dzv } else { 0.0 };
    }

    let mut dh = [0.0f32; HIDDEN];
    for (j, dhv) in dh.iter_mut().enumerate() {
        let mut acc = 0.0;
        for (k, &ds) in ds2.iter().enumerate() {
            acc += w[OFF_W2 + j * LATENT + k] * ds;
        }
        *dhv = acc;
    }
    let mut ds1 = [0.0f32; HIDDEN];
    for ((ds, &dhv), &hv) in ds1.iter_mut().zip(dh.iter()).zip(fwd.h.iter()) {
        *ds = if hv > 0.0 { dhv } else { 0.0 };
    }

    for (i, &xv) in x.values.iter().enumerate() {
        if xv == 0.0 {
            continue;
        }
        for (j, &ds) in ds1.iter().enumerate() {
            g[i * HIDDEN + j] += xv * ds;
        }
    }
    for (j, &ds) in ds1.iter().enumerate() {
        g[OFF_B1 + j] += ds;
    }
    for (j, &hv) in fwd.h.iter().enumerate() {
        if hv == 0.0 {
            continue;
        }
        for (k, &ds) in ds2.iter().enumerate() {
            g[OFF_W2 + j * LATENT + k] += hv * ds;
        }
    }
    for (k, &ds) in ds2.iter().enumerate() {
        g[OFF_B2 + k] += ds;
    }
    for (j, &zv) in fwd.z.iter().enumerate() {
        if zv == 0.0 {
            continue;
        }
        for (c, &d) in dl.iter().enumerate() {
            g[OFF_WO + j * OPS + c] += zv * d;
        }
    }
    for (c, &d) in dl.iter().enumerate() {
        g[OFF_BO + c] += d;
    }
    for (j, &zv) in fwd.z.iter().enumerate() {
        if zv == 0.0 {
            continue;
        }
        g[OFF_WC + j] += zv * dcr;
        g[OFF_WR + j] += zv * drr;
    }
    g[OFF_BC] += dcr;
    g[OFF_BR] += drr;
}

/// Init uniforme en [-1/sqrt(fan_in), 1/sqrt(fan_in)], biases a 0.
fn init_weights(seed: u32) -> Vec<f32> {
    let mut rng = Rng(seed);
    let mut w = vec![0.0f32; WEIGHT_COUNT];
    let uniform =
        |rng: &mut Rng, fan_in: usize| (rng.next_f32() * 2.0 - 1.0) / (fan_in as f32).sqrt();

    for i in 0..INPUT {
        for j in 0..HIDDEN {
            w[i * HIDDEN + j] = uniform(&mut rng, INPUT);
        }
    }
    for j in 0..HIDDEN {
        for k in 0..LATENT {
            w[OFF_W2 + j * LATENT + k] = uniform(&mut rng, HIDDEN);
        }
    }
    for j in 0..LATENT {
        for c in 0..OPS {
            w[OFF_WO + j * OPS + c] = uniform(&mut rng, LATENT);
        }
    }
    for j in 0..LATENT {
        w[OFF_WC + j] = uniform(&mut rng, LATENT);
        w[OFF_WR + j] = uniform(&mut rng, LATENT);
    }
    w
}

/// Entrena y devuelve los `WEIGHT_COUNT` pesos (layout de `repair_nn_core`).
///
/// SGD por mini-batches sobre el dataset sintetico determinista. El shuffle
/// usa el mismo LCG que el init: una misma `TrainConfig` produce siempre la
/// misma red.
pub fn train(config: &TrainConfig) -> Vec<f32> {
    assert!(config.samples > 0, "train necesita samples > 0");
    assert!(config.batch > 0, "train necesita batch > 0");

    let dataset = generate_synthetic_dataset(config.samples, config.dataset_seed);
    let features: Vec<FeatureVector> = dataset
        .iter()
        .map(|s| extract(&s.incident, &s.signature))
        .collect();
    let labels: Vec<usize> = dataset
        .iter()
        .map(|s| s.ground_truth_operator as usize)
        .collect();
    sgd(&features, &labels, config)
}

/// Loop SGD compartido por train (sintetico) y train_mixed (sintetico MAS
/// ejemplos reales verificados): mismo LCG, mismo shuffle y mismo decay
/// de lr.
fn sgd(features: &[FeatureVector], labels: &[usize], config: &TrainConfig) -> Vec<f32> {
    let samples = features.len();
    assert!(samples > 0, "sgd necesita samples > 0");
    assert!(config.batch > 0, "sgd necesita batch > 0");

    let mut w = init_weights(config.init_seed);
    let mut g = vec![0.0f32; WEIGHT_COUNT];
    let mut order: Vec<usize> = (0..samples).collect();
    let mut rng = Rng(config.init_seed);
    let decay_epoch = (config.epochs as f32 * 0.7) as usize;

    for epoch in 0..config.epochs {
        // Fisher-Yates: el orden de cada epoca depende solo del LCG.
        for i in (1..samples).rev() {
            let j = ((rng.next_f32() * (i + 1) as f32) as usize).min(i);
            order.swap(i, j);
        }
        let lr = if epoch < decay_epoch {
            config.lr
        } else {
            config.lr_final
        };

        let mut start = 0;
        while start < samples {
            let bs = config.batch.min(samples - start);
            g.fill(0.0);
            for &si in &order[start..start + bs] {
                let fwd = forward(&w, &features[si]);
                backward(&fwd, labels[si], &w, &mut g, &features[si]);
            }
            let scale = lr / bs as f32;
            for (wv, &gv) in w.iter_mut().zip(g.iter()) {
                *wv -= gv * scale;
            }
            start += bs;
        }
    }
    w
}

/// Cuantas veces se replica cada ejemplo real dentro del mixto: para que
/// la senal verificada no quede ahogada por el dataset sintetico.
pub const REAL_EXAMPLE_BOOST: usize = 8;

/// Entrena con el dataset sintetico MAS los ejemplos reales verificados
/// (TrainingExample exportados de REPAIR_CASES_KV): cierra el loop de
/// aprendizaje; la NN aprende de reparaciones que Actions verifico PASS.
/// Fail-closed: ejemplos con dimension incorrecta, operador fuera de
/// rango o reward no positivo se ignoran (nunca entrenan con senal
/// dudosa).
pub fn train_mixed(config: &TrainConfig, examples: &[TrainingExample]) -> Vec<f32> {
    let dataset = generate_synthetic_dataset(config.samples, config.dataset_seed);
    let mut features: Vec<FeatureVector> = dataset
        .iter()
        .map(|s| extract(&s.incident, &s.signature))
        .collect();
    let mut labels: Vec<usize> = dataset
        .iter()
        .map(|s| s.ground_truth_operator as usize)
        .collect();
    for ex in examples {
        let op = ex.operator as usize;
        if ex.features.len() != FeatureVector::DIM || op >= OPS || ex.reward <= 0.0 {
            continue;
        }
        let mut fv = FeatureVector::zeros();
        fv.values.copy_from_slice(&ex.features);
        for _ in 0..REAL_EXAMPLE_BOOST {
            features.push(fv);
            labels.push(op);
        }
    }
    sgd(&features, &labels, config)
}

/// Carga TrainingExample desde un dump JSONL (una linea por ejemplo).
/// Una linea corrupta invalida el archivo entero (fail-closed): mejor no
/// entrenar que entrenar con filas a medias.
pub fn load_examples_jsonl(raw: &str) -> Result<Vec<TrainingExample>, String> {
    let mut out = Vec::new();
    for (i, line) in raw.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        let ex: TrainingExample =
            serde_json::from_str(line).map_err(|e| format!("linea {i}: {e}"))?;
        out.push(ex);
    }
    Ok(out)
}

/// Evalua pesos contra el dataset sintetico (semilla propia) usando el
/// `RepairNet::predict` REAL de produccion, no una copia del forward.
pub fn evaluate(weights: &[f32], samples: usize, seed: u64) -> Result<Metrics, &'static str> {
    if samples == 0 {
        return Err("evaluate necesita samples > 0");
    }
    let net = RepairNet::from_weights(weights)?;
    let dataset = generate_synthetic_dataset(samples, seed);
    let mut correct = 0usize;
    let mut actionable = 0usize;
    let mut conf_sum = 0.0f32;
    let mut risk_sum = 0.0f32;

    for item in &dataset {
        let fv = extract(&item.incident, &item.signature);
        let action = net.predict(&fv);
        if action.repair_operator == OperatorId::from_u8(item.ground_truth_operator) {
            correct += 1;
        }
        if action.is_actionable(MIN_CONFIDENCE, MAX_RISK) {
            actionable += 1;
        }
        conf_sum += action.confidence;
        risk_sum += action.risk;
    }

    let n = samples as f32;
    Ok(Metrics {
        samples,
        accuracy: correct as f32 / n,
        actionable: actionable as f32 / n,
        mean_confidence: conf_sum / n,
        mean_risk: risk_sum / n,
    })
}

/// Exporta el payload KV: `WEIGHT_COUNT` f32 en texto, uno por linea.
///
/// `Display` de `f32` emite la representacion mas corta que parsea al mismo
/// valor (round-trip exacto): no hay perdida de precision en el export.
pub fn export_payload(weights: &[f32]) -> Result<String, &'static str> {
    if weights.len() != WEIGHT_COUNT {
        return Err("weight length mismatch");
    }
    let mut out = String::new();
    for &v in weights {
        out.push_str(&v.to_string());
        out.push('\n');
    }
    Ok(out)
}

/// Espejo del loader del worker (`worker/src/worker/model.rs`): cualquier
/// token no finito, o un recuento distinto de `WEIGHT_COUNT`, invalida el
/// payload completo. Mismas reglas: aqui `Err`, alla `None` ->
/// `blocked_no_model`. Nunca se cae a pesos desplazados.
pub fn load_payload(raw: &str) -> Result<Vec<f32>, &'static str> {
    if raw.trim().is_empty() {
        return Err("payload vacio");
    }
    let mut values: Vec<f32> = Vec::with_capacity(WEIGHT_COUNT);
    for token in raw.split_whitespace() {
        let v: f32 = token.parse().map_err(|_| "token no numerico")?;
        if !v.is_finite() {
            return Err("token no finito");
        }
        values.push(v);
        if values.len() > WEIGHT_COUNT {
            return Err("demasiados tokens");
        }
    }
    if values.len() != WEIGHT_COUNT {
        return Err("weight length mismatch");
    }
    Ok(values)
}
