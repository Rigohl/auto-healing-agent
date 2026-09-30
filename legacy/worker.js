/**
 * LEGACY Cloudflare Worker (JS). Prefer worker/ (Rust workers-rs).
 * Moved from root to match Notion structure.
 */
const DISPATCH_EVENT = "auto-repair";

export default {
  async fetch(request, env) {
    if (request.method === "GET") {
      return json({ ok: true, service: "auto-healing-agent-worker-legacy" });
    }
    if (request.method !== "POST") {
      return json({ error: "method_not_allowed" }, 405);
    }
    const raw = await request.text();
    let body;
    try {
      body = raw ? JSON.parse(raw) : {};
    } catch {
      return json({ error: "invalid_json" }, 400);
    }
    const source = detectSource(request, body);
    if (!source) {
      return json({ error: "unknown_source" }, 400);
    }
    if (env.WEBHOOK_SECRET) {
      const hdr = request.headers.get("x-webhook-secret") || "";
      if (hdr !== env.WEBHOOK_SECRET) {
        return json({ error: "unauthorized" }, 401);
      }
    }
    const payload = normalizePayload(source, body);
    if (!payload.shouldRun) {
      return json({ ok: true, skipped: true, reason: payload.reason || "not_actionable" });
    }
    const owner = env.GITHUB_OWNER || "Rigohl";
    const repo = env.GITHUB_REPO || "auto-healing-agent";
    const token = env.GH_PAT;
    if (!token) return json({ error: "missing_GH_PAT" }, 500);
    const res = await fetch(`https://api.github.com/repos/${owner}/${repo}/dispatches`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        Accept: "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "Content-Type": "application/json",
        "User-Agent": "auto-healing-agent-worker",
      },
      body: JSON.stringify({
        event_type: DISPATCH_EVENT,
        client_payload: {
          source,
          ...payload.clientPayload,
          receivedAt: new Date().toISOString(),
        },
      }),
    });
    if (!res.ok && res.status !== 204) {
      const errText = await res.text();
      return json({ error: "github_dispatch_failed", status: res.status, detail: errText }, 502);
    }
    return json({ ok: true, dispatched: DISPATCH_EVENT, source });
  },
};

function detectSource(request, body) {
  const ua = (request.headers.get("user-agent") || "").toLowerCase();
  if (request.headers.get("x-vercel-signature") || body.deployment || body.project || ua.includes("vercel")) {
    return "vercel";
  }
  if (request.headers.get("linear-signature") || body.type?.startsWith?.("Issue") || body.data?.issue) {
    return "linear";
  }
  if (body.source === "vercel" || body.source === "linear") return body.source;
  return null;
}

function normalizePayload(source, body) {
  if (source === "vercel") {
    const state = (body.deployment?.state || body.state || body.type || "").toString().toLowerCase();
    const errorLike =
      state.includes("error") ||
      state.includes("fail") ||
      body.type === "deployment.error" ||
      body.type === "deployment.failed";
    if (!errorLike) return { shouldRun: false, reason: `vercel_state_${state || "unknown"}` };
    return {
      shouldRun: true,
      clientPayload: {
        vercel: {
          deploymentId: body.deployment?.id || body.id,
          url: body.deployment?.url || body.url,
          projectId: body.project?.id || body.projectId,
          projectName: body.project?.name || body.projectName,
          state,
        },
      },
    };
  }
  const issue = body.data?.issue || body.issue || body.data || {};
  const labels = (issue.labels || []).map((l) =>
    (typeof l === "string" ? l : l.name || "").toLowerCase()
  );
  const hasAutoRepair =
    labels.includes("auto-repair") || JSON.stringify(body).toLowerCase().includes("auto-repair");
  if (!hasAutoRepair) return { shouldRun: false, reason: "no_auto-repair_label" };
  return {
    shouldRun: true,
    clientPayload: {
      linear: {
        issueId: issue.id,
        issueIdentifier: issue.identifier,
        title: issue.title,
        description: issue.description,
        url: issue.url,
        labels,
      },
    },
  };
}

function json(data, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}
