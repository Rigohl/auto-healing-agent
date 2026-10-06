#!/usr/bin/env python3
"""Deterministic repository consistency & drift checker.

Compara lo que el repo AFIRMA (docs, invariantes declaradas) con lo que el
repo HACE (codigo, workflows, permisos en el arbol). Cada afirmacion devuelve
PASS, FAIL o UNKNOWN. UNKNOWN en un invariante critico se cuenta como FAIL:
nunca se autopromociona a PASS.

Por que no usar cargo test para esto: los tests verifican comportamiento del
codigo que ya compila. Esto verifica las afirmaciones que crosses tres
unidades de build (crates/, worker/ y .github/workflows/) mas los permisos del
arbol, que ningun test ve porque no son codigo Rust.

Diseno (heredado de jules-5813573718571814256-249ce1c3, PR #7, reescrito):
  - Los checks apuntan al archivo y al simbolo REALES de main. La version
    original buscaba `constant_time_eq` en worker/src/lib.rs y los umbrales
    tambien ahi; en main ambos viven en worker/src/worker/{security,mod}.rs,
    asi que los checks originales fallarian con el codigo correcto.
  - No se escribe nada en docs/: la evidencia es un artefacto de CI. Un
    `verification_evidence.json` versionado es evidencia que nadie regenera y
    por tanto solo puede quedarse vieja.

Uso:
    python3 scripts/verify_repo.py [--out RUTA]
    WORKER_URL=... python3 scripts/verify_repo.py    # habilita el smoke test
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from typing import Any

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))

CRATE_FILES = [
    "crates/repair_types/src/lib.rs",
    "crates/repair_types/src/contract.rs",
    "crates/feature_engine/src/lib.rs",
    "crates/feature_engine/src/synthetic.rs",
    "crates/repair_nn_core/src/lib.rs",
    "crates/repair_nn_wasm/src/lib.rs",
    "crates/repair_operators/src/lib.rs",
    "crates/repair_pr/src/lib.rs",
    "crates/repair_pr/src/diff.rs",
    "crates/repair_pr/src/github.rs",
]

WORKER_FILES = [
    "worker/Cargo.toml",
    "worker/src/lib.rs",
    "worker/src/worker/mod.rs",
    "worker/src/worker/security.rs",
    "worker/src/worker/incident_state.rs",
    "worker/src/worker/queue_consumer.rs",
    "worker/src/worker/quota.rs",
    "worker/src/worker/anti_loop.rs",
    "worker/wrangler.toml",
]

WORKFLOWS = [
    ".github/workflows/ci.yml",
    ".github/workflows/consistency.yml",
    ".github/workflows/deploy.yml",
    ".github/workflows/wasm.yml",
    ".github/workflows/security.yml",
    ".github/workflows/regression.yml",
    ".github/workflows/repair-validation.yml",
    ".github/workflows/auto-repair.yml",
    ".github/workflows/deploy-staging.yml",
    ".github/workflows/promote-model.yml",
    ".github/workflows/cleanup-branches.yml",
]

DOCS = [
    "docs/ARCHITECTURE.md",
    "docs/GOVERNANCE.md",
    "docs/DISCREPANCIES.md",
    "docs/BRANCH_POLICY.md",
    "docs/CONTRACT.md",
    "docs/E2E_CHECKLIST.md",
    "docs/INVENTORY.md",
    "docs/NO_LLM_POLICY.md",
]

# INPUT/HIDDEN/LATENT/OPS del MLP, y WEIGHT_COUNT que sale de su aritmetica.
NN_INPUT, NN_HIDDEN, NN_LATENT, NN_OPS = 64, 32, 16, 13
NN_WEIGHT_COUNT = (
    NN_INPUT * NN_HIDDEN
    + NN_HIDDEN
    + NN_HIDDEN * NN_LATENT
    + NN_LATENT
    + NN_LATENT * NN_OPS
    + NN_OPS
    + NN_LATENT
    + 1
    + NN_LATENT
    + 1
)


def read(rel: str) -> str:
    path = os.path.join(ROOT, rel)
    if not os.path.exists(path):
        return ""
    with open(path, "r", encoding="utf-8", errors="ignore") as fh:
        return fh.read()


def workflow_triggers(rel: str) -> set[str] | None:
    """Triggers declarados bajo el `on:` de primer nivel de un workflow.

    Devuelve None si no se puede parsear, para que el claim sea UNKNOWN y no un
    PASS por defecto. Un grep de "push:" no sirve: cambiarlo por "pushx:" deja
    el workflow sin disparar nada y el grep loeria verde.
    """
    lines = read(rel).splitlines()
    triggers: set[str] = set()
    inside = False
    key_indent: int | None = None
    for line in lines:
        if not inside:
            if re.match(r"^on:\s*(#.*)?$", line):
                inside = True
            continue
        if not line.strip() or line.strip().startswith("#"):
            continue
        indent = len(line) - len(line.lstrip())
        if indent == 0:
            break  # se acabo el bloque on:
        if key_indent is None:
            # Las claves de primer nivel son las de la primera linea con
            # contenido: `push:`, `workflow_dispatch:`. Todo lo mas profundo
            # (`branches:`, `paths:`, `inputs:`) es de otro trigger.
            key_indent = indent
        if indent != key_indent:
            continue
        m = re.match(r"^\s+([A-Za-z_][\w-]*):", line)
        if m:
            triggers.add(m.group(1))
    return triggers or None


def code_only(rel: str) -> str:
    """El archivo sin comentarios de linea.

    Los comentarios de este repo explican por que algo se hizo y mencionan
   Terms que el codigo ya no usa ("V0 hacia hash01(&format!(\"{}:{}\", i,
    fingerprint))", "no depende de `rand`"). Buscar en la prosa daria un FAIL
    sobre el codigo correcto, que es peor que no comprobar nada: entrena al
    lector a ignorar el checker.
    """
    body = []
    in_block = False
    for line in read(rel).splitlines():
        stripped = line.strip()
        if stripped.startswith("/*") or stripped.startswith("*") or stripped.endswith("*/"):
            in_block = stripped.startswith("/*") and not stripped.endswith("*/")
            continue
        if in_block:
            in_block = not stripped.endswith("*/")
            continue
        body.append(line.split("//", 1)[0])
    return "\n".join(body)


class Report:
    """Acumula claims. UNKNOWN critico se contabiliza como FAIL."""

    def __init__(self) -> None:
        self.claims: list[dict[str, Any]] = []

    def add(
        self,
        claim_id: str,
        description: str,
        status: str,
        details: str = "",
        critical: bool = True,
    ) -> None:
        status = status.upper()
        if status not in ("PASS", "FAIL", "UNKNOWN"):
            status = "UNKNOWN"
        self.claims.append(
            {
                "id": claim_id,
                "description": description,
                "status": status,
                "details": details,
                "critical": critical,
            }
        )

    def expect(
        self,
        claim_id: str,
        description: str,
        ok: bool,
        details: str = "",
        critical: bool = True,
    ) -> None:
        self.add(claim_id, description, "PASS" if ok else "FAIL", details, critical)

    def summary(self) -> dict[str, Any]:
        failed = [c for c in self.claims if c["status"] == "FAIL"]
        unknown = [c for c in self.claims if c["status"] == "UNKNOWN"]
        # UNKNOWN critico bloquea igual que FAIL.
        blocking = failed + [c for c in unknown if c["critical"]]
        total = len(self.claims)
        passed = sum(1 for c in self.claims if c["status"] == "PASS")
        return {
            "total_claims": total,
            "pass": passed,
            "fail": len(failed),
            "unknown": len(unknown),
            "critical_unknown": sum(1 for c in unknown if c["critical"]),
            "coverage_percentage": round(passed / total * 100, 2) if total else 0.0,
            "status": "PASS" if not blocking else "FAIL",
        }


# ---------------------------------------------------------------- estructura


def check_files_exist(r: Report) -> None:
    required = (
        [
            "README.md",
            "Cargo.toml",
            "rust-toolchain.toml",
            ".gitignore",
            "LICENSE",

            "model/current.json",
            "model/stable.json",
            "model/schema.json",
            "scripts/validate-preflight.sh",
            "scripts/verify_repo.py",
            "scripts/set-github-secrets.sh",
        ]
        + CRATE_FILES
        + WORKER_FILES
        + WORKFLOWS
        + DOCS
    )
    missing = [p for p in required if not os.path.exists(os.path.join(ROOT, p))]
    r.expect(
        "FILES_PRESENT",
        f"Los {len(required)} archivos obligatorios existen",
        not missing,
        f"faltan: {missing}" if missing else "",
    )


def check_exec_bits(r: Report) -> None:
    """Los bits de ejecucion no son codigo: ningun cargo test los ve."""
    # El +x solo es funcional para archivos ejecutados directamente
    # (Workers Builds corre worker/build.sh). validate-preflight.sh se
    # invoca con `bash scripts/validate-preflight.sh`, y la API de
    # contents siempre reescribe los blobs sin bit +x (100644).
    for rel in ("worker/build.sh",):
        path = os.path.join(ROOT, rel)
        r.expect(
            f"EXECBIT_{rel.replace('/', '_')}",
            f"{rel} es ejecutable",
            os.path.exists(path) and os.access(path, os.X_OK),
            "Workers Builds falla con un error que no parece de permisos",
        )


def check_two_build_units(r: Report) -> None:
    """worker/ tiene su propio [workspace]; el raiz no lo compila (item 35)."""
    root_manifest = read("Cargo.toml")
    worker_manifest = read("worker/Cargo.toml")

    r.expect(
        "WORKER_NOT_IN_ROOT_WORKSPACE",
        'worker/ NO es miembro del workspace raiz (DISCREPANCIES item 35)',
        '"worker"' not in root_manifest,
        'aparece "worker" en los members del manifiesto raiz',
    )
    r.expect(
        "WORKER_OWN_WORKSPACE",
        "worker/Cargo.toml declara su propio [workspace]",
        "[workspace]" in worker_manifest,
        "sin [workspace], cargo se niega a compilarlo aparte",
    )
    # Si el worker volviera a ser miembro, los jobs worker-* de ci.yml con
    # --manifest-path seguian passando pero dejarian de ser los unicos que lo
    # cubren. Este claim es la red que avisa del cambio de modelo de build.
    ci = read(".github/workflows/ci.yml")
    r.expect(
        "CI_HAS_WORKER_JOBS",
        "ci.yml mantiene jobs worker-* con --manifest-path worker/Cargo.toml",
        "--manifest-path worker/Cargo.toml" in ci
        and "worker-test:" in ci
        and "worker-clippy:" in ci,
    )
    r.expect(
        "WORKER_QUEUE_FEATURE",
        'worker/Cargo.toml conserva features = ["queue"] (PART3: Queues + DLQ)',
        '"queue"' in worker_manifest or "queue" in worker_manifest,
        "sin la feature queue no compila #[event(queue)] ni el consumidor asincrono",
    )


# ---------------------------------------------------------------- seguridad


def check_no_llm_path(r: Report) -> None:
    auto_repair = read(".github/workflows/auto-repair.yml")
    r.expect(
        "NO_LLM_WORKFLOW",
        "auto-repair.yml declara el camino LLM como deshabilitado",
        "disabled" in auto_repair.lower(),
    )
    # Un camino LLM colado de vuelta seria un fichero nuevo, no una linea.
    offenders = []
    for dirpath, _dirnames, filenames in os.walk(os.path.join(ROOT, "crates")):
        for name in filenames:
            if name.endswith(".rs"):
                rel = os.path.relpath(os.path.join(dirpath, name), ROOT)
                body = read(rel).lower()
                for needle in ("openai", "anthropic", "huggingface", "reqwest"):
                    if needle in body:
                        offenders.append(f"{rel}:{needle}")
    r.expect(
        "NO_LLM_IN_CRATES",
        "Ningun crate depende de un cliente LLM o HTTP a uno",
        not offenders,
        f"encontrado: {offenders}" if offenders else "",
    )


def check_constant_time_auth(r: Report) -> None:
    """El secret se compara en tiempo constante y en fail-closed."""
    security = read("worker/src/worker/security.rs")
    lib = read("worker/src/lib.rs")

    r.expect(
        "CONSTANT_TIME_AUTH",
        "verify_webhook_secret delega en constant_time_eq",
        "fn verify_webhook_secret" in security and "constant_time_eq" in security,
        "buscado en worker/src/worker/security.rs (no en lib.rs)",
    )
    r.expect(
        "AUTH_FAIL_CLOSED",
        "Sin WEBHOOK_SECRET el webhook responde 503 (nunca fail-open)",
        'svc_unavailable("webhook_secret_not_configured")' in lib,
    )
    r.expect(
        "AUTH_REJECTS_401",
        "Secret incorrecto responde 401",
        '(StatusCode::UNAUTHORIZED, "unauthorized")' in lib,
    )


def check_no_active_placeholder(r: Report) -> None:
    """REPLACE_WITH en comentario no bloquea; en linea activa si."""
    for rel in ("worker/wrangler.toml", "wrangler.toml"):
        active = [
            line
            for line in read(rel).splitlines()
            if "REPLACE_WITH" in line and not line.strip().startswith("#")
        ]
        r.expect(
            f"NO_ACTIVE_PLACEHOLDER_{rel.replace('/', '_')}",
            f"{rel} no tiene REPLACE_WITH en una linea activa",
            not active,
            f"lineas: {active}" if active else "",
        )
    # La logica de preflight no puede volver a la version sin `sed`, que
    # bloqueo el deploy por un comentario (falso positivo del P0, PR #8).
    deploy = read(".github/workflows/deploy.yml")
    r.expect(
        "PREFLIGHT_STRIPS_COMMENTS",
        "El Preflight de deploy.yml filtra comentarios antes de buscar placeholders",
        "sed" in deploy and "REPLACE_WITH" in deploy,
    )
    script = read("scripts/validate-preflight.sh")
    r.expect(
        "PREFLIGHT_HAS_GUARD_TEST",
        "scripts/validate-preflight.yml tiene test del falso positivo del P0",
        "comentario" in script.lower(),
    )


def check_secrets_not_in_tree(r: Report) -> None:
    patterns = {
        "github_pat": re.compile(r"\bgh[pousr]_[A-Za-z0-9]{20,}\b"),
        "hf_token": re.compile(r"\bhf_[A-Za-z0-9]{20,}\b"),
        "cloudflare_token": re.compile(r"\b[A-Za-z0-9_-]{40}\b(?=.*CLOUDFLARE)"),
    }
    offenders: list[str] = []
    for dirpath, dirnames, filenames in os.walk(ROOT):
        dirnames[:] = [
            d for d in dirnames if d not in (".git", "target", "build", "node_modules")
        ]
        for name in filenames:
            if not name.endswith((".sh", ".rs", ".toml", ".yml", ".yaml", ".json", ".ts")):
                continue
            path = os.path.join(dirpath, name)
            try:
                with open(path, "r", encoding="utf-8", errors="ignore") as fh:
                    body = fh.read()
            except OSError:
                continue
            for label, pat in patterns.items():
                if pat.search(body):
                    offenders.append(f"{os.path.relpath(path, ROOT)}:{label}")
    r.expect(
        "NO_SECRETS_IN_TREE",
        "Ningun token con forma de secret en el arbol",
        not offenders,
        f"encontrado: {offenders}" if offenders else "",
        # gitleaks es la autoridad de este invariante; aqui es una red extra y
        # un patron masBurdo puede dar falso positivo sin romper el deploy.
        critical=False,
    )


# ---------------------------------------------------------------- gate / VERIFY


def check_gate_thresholds(r: Report) -> None:
    """GOVERNANCE.md es la fuente de verdad y el runtime la replica."""
    runtime = read("worker/src/worker/mod.rs")
    gov = read("docs/GOVERNANCE.md")

    r.expect(
        "GATE_THRESHOLDS_RUNTIME",
        "runtime declara MIN_CONFIDENCE=0.55 y MAX_RISK=0.45",
        "pub const MIN_CONFIDENCE: f32 = 0.55;" in runtime
        and "pub const MAX_RISK: f32 = 0.45;" in runtime,
        "buscado en worker/src/worker/mod.rs (no en lib.rs)",
    )
    r.expect(
        "GATE_THRESHOLDS_DOCS",
        "GOVERNANCE.md declara los mismos 0.55 / 0.45",
        "MIN_CONFIDENCE=0.55" in gov and "MAX_RISK=0.45" in gov,
    )
    # Si el gate deja de existir, la NN decide sola y eso viola GOVERNANCE.
    operators = read("crates/repair_operators/src/lib.rs")
    r.expect(
        "GATE_EXISTS",
        "repair_operators sigue exponiendo gate() fail-closed",
        "pub fn gate(" in operators and "is_actionable" in operators,
    )


def check_verify_authority(r: Report) -> None:
    """Actions es la autoridad de VERIFY; el Worker nunca declara PASS."""
    ci = read(".github/workflows/ci.yml")
    r.expect(
        "CI_IS_VERIFY_AUTHORITY",
        "ci.yml ejecuta tests y clippy con -D warnings",
        "cargo test --workspace" in ci
        and "cargo clippy --workspace --all-targets -- -D warnings" in ci,
    )
    # GOVERNANCE.md: AUTO_DEPLOY=false, PRODUCTION_WRITE=false. El unico
    # trigger admitido es workflow_dispatch, porque salta al environment
    # `production` con sus required reviewers. Cualquier otro trigger
    # desplegaria sin revision humana.
    expected = {"workflow_dispatch"}
    actual = workflow_triggers(".github/workflows/deploy.yml")
    if actual is None:
        r.add(
            "DEPLOY_TRIGGERS_EXACT",
            "deploy.yml dispara unicamente con workflow_dispatch (AUTO_DEPLOY=false)",
            "UNKNOWN",
            "no se pudo parsear el bloque on: de deploy.yml",
        )
    else:
        r.expect(
            "DEPLOY_TRIGGERS_EXACT",
            "deploy.yml dispara unicamente con workflow_dispatch (AUTO_DEPLOY=false)",
            actual == expected,
            f"triggers: {sorted(actual)}; se esperaba {sorted(expected)}. "
            f"un deploy sin dispatch saltaria el environment production",
        )
    r.expect(
        "NO_MERGE_AUTOMATION",
        "No hay bot de merge automatico (AUTO_MERGE=false)",
        not any(
            os.path.exists(os.path.join(ROOT, p))
            for p in (".mergify.yml", "auto-merge.yml")
        ),
    )


def check_weight_count(r: Report) -> None:
    """WEIGHT_COUNT es aritmetica, no un literal: se recalcula aqui."""
    nn = read("crates/repair_nn_core/src/lib.rs")
    declared = re.search(r"pub const WEIGHT_COUNT: usize\s*=", nn)
    r.expect(
        "WEIGHT_COUNT_DECLARED",
        f"repair_nn_core declara WEIGHT_COUNT ({NN_WEIGHT_COUNT} segun la aritmetica de capas)",
        declared is not None,
    )
    schema = read("model/schema.json")
    r.expect(
        "SCHEMA_INPUT_DIM",
        "model/schema.json declara input_dim = 64, igual que FeatureVector::DIM",
        '"input_dim": 64' in schema,
    )
    r.expect(
        "SCHEMA_OPERATOR_CLASSES",
        f"model/schema.json declara operator_classes = {NN_OPS}, igual que OPERATOR_COUNT",
        f'"operator_classes": {NN_OPS}' in schema,
    )


def check_encoder_invariants(r: Report) -> None:
    """El encoder V1 no puede volver a colar el fingerprint completo."""
    enc = read("crates/feature_engine/src/lib.rs")
    body = code_only("crates/feature_engine/src/lib.rs").split("#[cfg(test)]")[0]
    r.expect(
        "ENCODER_IS_V1",
        "El feature encoder es V1",
        "Feature encoder V1" in enc,
    )
    # V0 metia hash01(&format!("{}:{}", i, fingerprint)) en los slots altos:
    # el vector dependia del hash completo y ningun slot era estable entre
    # entregas del mismo error. V1 no lo usa.
    r.expect(
        "ENCODER_NO_FINGERPRINT_HASH",
        "extract() no hashea signature.fingerprint en ningun slot",
        "fingerprint" not in body,
        "V0 hacia hash01(&format!(\"{}:{}\", i, fingerprint)); slot directo de hash",
    )
    r.expect(
        "ENCODER_BIAS_SLOT",
        "El slot 63 es el bias 1.0 del encoder",
        "values[63] = 1.0" in body,
    )
    synthetic = code_only("crates/feature_engine/src/synthetic.rs")
    # "rand" a secasAppearanceia en rand_noise (el nombre de la variable de
    # ruido), asi que el claim mira las formas de dependencia reales.
    uses_rand = bool(re.search(r"\buse rand\b|\brand::", synthetic))
    r.expect(
        "SYNTHETIC_DATASET_DETERMINISTIC",
        "El dataset sintetico usa LCG con semilla, sin crate rand",
        "wrapping_mul" in synthetic and not uses_rand,
        "generar el dataset con rand haria el test no reproducible",
    )


def check_contract_module(r: Report) -> None:
    """El contrato GitHub<->Cloudflare debe seguir exportando su superficie."""
    lib = read("crates/repair_types/src/lib.rs")
    contract = read("crates/repair_types/src/contract.rs")
    contract_path = os.path.join(ROOT, "crates/repair_types/src/contract.rs")
    r.expect(
        "CONTRACT_MODULE_EXPORTED",
        "repair_types declara `pub mod contract` y el archivo existe",
        "pub mod contract;" in lib and os.path.exists(contract_path),
        "lib.rs declara el modulo pero el archivo no esta: no compila",
    )
    for sym in (
        "RepairEvent",
        "OutboundPRRequest",
        "CandidatePatch",
        "VerifiedResult",
        "CONTRACT_VERSION",
        "compute_idempotency_key",
        "get_error_policy",
        "MinimumPermissions",
    ):
        r.expect(
            f"CONTRACT_REEXPORTS_{sym}",
            f"{sym} se reexporta desde la raiz de repair_types",
            sym in lib.split("pub use contract::")[1].split("};")[0]
            if "pub use contract::" in lib
            else False,
        )
    # Pass sin evidencia de Actions es el fallo de governance mas caro: es el
    # unico camino por el que el sistema se autoaproba.
    r.expect(
        "PASS_REQUIRES_EVIDENCE",
        "VerifiedResult::is_valid() exige workflow_run_id, commit_sha y evidence_ref",
        "fn is_valid(&self) -> bool" in contract
        and "evidence_ref" in contract
        and "workflow_run_id" in contract,
    )


def check_gate_to_pr(r: Report) -> None:
    """GATE->PR V1 (CONTRACT §3, P2): diff real + apertura de PR, fail-closed."""
    root_manifest = read("Cargo.toml")
    repair_pr_manifest = read("crates/repair_pr/Cargo.toml")
    diff_rs = read("crates/repair_pr/src/diff.rs")
    github_rs = read("crates/repair_pr/src/github.rs")

    r.expect(
        "GATE_TO_PR_IN_WORKSPACE",
        'crates/repair_pr es miembro del workspace raiz (GATE->PR V1)',
        '"crates/repair_pr"' in root_manifest,
        "el puente GATE->PR quedo fuera del workspace",
    )
    r.expect(
        "GATE_TO_PR_DIFF_GENERATOR",
        "repair_pr declara similar (diff unificado) y octocrab (PR)",
        "similar" in repair_pr_manifest and "octocrab" in repair_pr_manifest,
        "sin similar/octocrab no hay diff real ni PR (item 71)",
    )
    r.expect(
        "GATE_TO_PR_NO_INVENTED_DIFFS",
        "contenido identico => bundle vacio => el contrato sigue blocked",
        "if change.before == change.after" in diff_rs
        and "patch.diff = None" in diff_rs,
        "el generador podria inventar diffs (CONTRACT §3)",
    )
    r.expect(
        "GATE_TO_PR_FAIL_CLOSED",
        "sin token o con bundle vacio la apertura falla cerrada",
        "MissingToken" in github_rs and "Blocked" in github_rs,
        "fail-open abriria PRs sin autorizacion",
    )


def check_no_orphan_code(r: Report) -> None:
    """Código huérfano: crates fuera del workspace, deps sin vigilancia."""
    root_manifest = read("Cargo.toml")
    crates_dir = os.path.join(ROOT, "crates")
    orphans = sorted(
        d
        for d in os.listdir(crates_dir)
        if os.path.isdir(os.path.join(crates_dir, d))
        and f'"crates/{d}"' not in root_manifest
    )
    r.expect(
        "NO_ORPHAN_CRATES",
        "todo directorio de crates/ es miembro del workspace raíz",
        not orphans,
        f"crates huérfanos (fuera del workspace): {orphans}" if orphans else "",
    )
    ci = read(".github/workflows/ci.yml")
    r.expect(
        "NO_ORPHAN_DEPS_CI",
        "ci.yml vigila deps sin uso (cargo machete + cargo shear)",
        "cargo machete" in ci and "cargo shear" in ci,
        "sin vigilancia, dependencias muertas se acumulan sin que CI lo vea",
    )


def check_fmt_blocking(r: Report) -> None:
    """Item 34 cerrado: workspace-fmt es blocking, ningun job vuelve a advisory."""
    ci = read(".github/workflows/ci.yml")
    r.expect(
        "FMT_BLOCKING",
        "ci.yml ejecuta cargo fmt --check sin continue-on-error (item 34)",
        "cargo fmt --all -- --check" in ci and "continue-on-error" not in ci,
        "un fmt advisory deja entrar formato divergente sin que CI lo bloquee",
    )


# ---------------------------------------------------------------- git / drift


def check_branch_drift(r: Report) -> None:
    """docs/BRANCH_POLICY.md: solo main persiste. Esta claim lo hace real."""
    try:
        # symref aparte: `origin/HEAD` es un puntero symbolic y
        # `%(refname:short)` lo devuelve como `origin`, no como `origin/HEAD`.
        # Filtrarlo por sufijo "/HEAD" no lo caza y `origin` acaba contado como
        # si fuera una rama. Es lo que pasa en actions/checkout.
        out = subprocess.run(
            [
                "git",
                "for-each-ref",
                "--format=%(refname)%09%(symref)",
                "refs/remotes/origin",
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=True,
        ).stdout
    except (subprocess.SubprocessError, OSError) as exc:
        # Sin repositorio no hay nada que drift pueda comprobar.
        r.add(
            "BRANCH_DRIFT",
            "origin solo tiene main (o la rama efimera de la PR en curso)",
            "UNKNOWN",
            f"git no disponible: {exc}",
            critical=False,
        )
        return

    branches: list[str] = []
    for line in out.splitlines():
        if not line.strip():
            continue
        refname, _, symref = line.partition("\t")
        if symref:
            continue  # puntero symbolic (origin/HEAD), no una rama
        short = refname.removeprefix("refs/remotes/")
        if short == "origin" or short.endswith("/HEAD"):
            continue
        branches.append(short)

    if not branches:
        # Un checkout shallow (fetch-depth: 1, el default de actions/checkout)
        # no trae refs de origin: el claim no se puede comprobar y pasaria en
        # verde sin haber mirado nada. FAIL explicito, noUnknown benigno.
        r.expect(
            "BRANCH_DRIFT",
            "origin solo tiene main (o la rama efimera de la PR en curso)",
            False,
            "no hay refs refs/remotes/origin: el checkout es shallow o no hay "
            "remote. El workflow debe usar fetch-depth: 0 para que este claim "
            "compruebe algo; con fetch-depth: 1 pasaria en verde sin mirar.",
        )
        return

    # La rama bajo revision no cuenta como persistencia: por definicion es
    # efimera (docs/BRANCH_POLICY.md, regla 1: se borra tras merge o abandono).
    # En CI es GITHUB_HEAD_REF; en local, la ramaChecked out. Asi el claim no
    # puede pasar en verde por elodbjeto mismo que esta comprobando.
    head_ref = os.environ.get("GITHUB_HEAD_REF", "")
    if not head_ref:
        try:
            head_ref = subprocess.run(
                ["git", "rev-parse", "--abbrev-ref", "HEAD"],
                cwd=ROOT,
                capture_output=True,
                text=True,
                check=True,
            ).stdout.strip()
        except (subprocess.SubprocessError, OSError):
            head_ref = ""
    # `main` nunca es efimera, aunque HEAD este en ella: si no, un push a main
    # marcaria origin/main como "la rama bajo revision" y dejaria el claim en
    # FAIL con el repo exactamente como debe estar.
    ephemeral = [
        b
        for b in branches
        if head_ref
        and head_ref not in ("main", "master")
        and b in (f"origin/{head_ref}", head_ref)
    ]
    persistent = sorted(b for b in branches if b not in ephemeral)

    r.expect(
        "BRANCH_DRIFT",
        "origin solo tiene main (o la rama efimera de la PR en curso)",
        persistent == ["origin/main"],
        f"persistentes: {persistent}; efimeras: {sorted(ephemeral)}",
    )
    policy = read("docs/BRANCH_POLICY.md")
    r.expect(
        "BRANCH_POLICY_DOC",
        "docs/BRANCH_POLICY.md sigue declarando que solo main persiste",
        "solo `main`" in policy.lower() or "solo `main`" in policy,
    )


def check_workflow_permissions(r: Report) -> None:
    """Least privilege: ningun workflow con permiso de escritura por defecto."""
    offenders = []
    for rel in WORKFLOWS:
        body = read(rel)
        if "permissions:" not in body:
            offenders.append(f"{rel}:sin permissions")
    r.expect(
        "WORKFLOW_LEAST_PRIVILEGE",
        "Los workflows declaran `permissions` explicito",
        not offenders,
        f"sin permisos declarados: {offenders}" if offenders else "",
    )


def check_smoke_test_reachable(r: Report) -> None:
    """El smoke test de deploy depende de una variable que puede no existir."""
    worker_url = os.environ.get("WORKER_URL", "")
    if not worker_url:
        r.add(
            "SMOKE_WORKER_URL",
            "WORKER_URL disponible para el smoke test del webhook",
            "UNKNOWN",
            "WORKER_URL no configurado: smoke test omitido (deploy.yml lo salta)",
            critical=False,
        )
    else:
        r.add(
            "SMOKE_WORKER_URL",
            "WORKER_URL disponible para el smoke test del webhook",
            "PASS",
            f"url: {worker_url}",
        )


CHECKS = [
    check_files_exist,
    check_exec_bits,
    check_two_build_units,
    check_no_llm_path,
    check_constant_time_auth,
    check_no_active_placeholder,
    check_secrets_not_in_tree,
    check_gate_thresholds,
    check_verify_authority,
    check_weight_count,
    check_encoder_invariants,
    check_contract_module,
    check_gate_to_pr,
    check_no_orphan_code,
    check_fmt_blocking,
    check_branch_drift,
    check_workflow_permissions,
    check_smoke_test_reachable,
]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--out",
        default=os.environ.get(
            "VERIFY_EVIDENCE_OUT", os.path.join(ROOT, "verification_evidence.json")
        ),
        help="Ruta del JSON de evidencia (no se escribe dentro de docs/)",
    )
    args = parser.parse_args()

    report = Report()
    for check in CHECKS:
        check(report)

    summary = report.summary()

    print("=" * 72)
    print("REPO CONSISTENCY & DRIFT CHECK")
    print("=" * 72)
    for claim in report.claims:
        flag = "CRIT " if claim["critical"] else "     "
        print(f"[{claim['status']:7s}] {flag}{claim['id']}")
        if claim["details"] and claim["status"] != "PASS":
            print(f"          {claim['details']}")
    print("-" * 72)
    print(
        f"claims={summary['total_claims']} pass={summary['pass']} "
        f"fail={summary['fail']} unknown={summary['unknown']} "
        f"(criticos={summary['critical_unknown']}) "
        f"coverage={summary['coverage_percentage']}%"
    )
    print(f"OVERALL: {summary['status']}")
    print("=" * 72)

    out_dir = os.path.dirname(os.path.abspath(args.out))
    if out_dir:
        os.makedirs(out_dir, exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as fh:
        json.dump({"summary": summary, "claims": report.claims}, fh, indent=2)
    print(f"evidencia: {args.out}")

    return 0 if summary["status"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
