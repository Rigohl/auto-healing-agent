/**
 * LEGACY — LLM HuggingFace path. No ampliar.
 * Núcleo canónico: crates/ + worker/ (Rust/WASM).
 * Moved from repo root to match Notion structure.
 */

import { MongoClient } from "mongodb";
import { writeFileSync, mkdirSync } from "fs";
import { execSync } from "child_process";

type ClientPayload = {
  source?: string;
  vercel?: {
    deploymentId?: string;
    url?: string;
    projectId?: string;
    projectName?: string;
    state?: string;
  };
  linear?: {
    issueId?: string;
    issueIdentifier?: string;
    title?: string;
    description?: string;
    url?: string;
    labels?: string[];
  };
};

const env = (k: string, fallback = "") => process.env[k] || fallback;

async function main() {
  const payload: ClientPayload = safeJson(env("CLIENT_PAYLOAD", "{}"));
  const source = env("EVENT_SOURCE", payload.source || "manual");
  console.log("[agent] source=", source, "payload keys=", Object.keys(payload));

  const linearIssue = await fetchLinearIssue(payload);
  const vercelLogs = await fetchVercelErrorContext(payload);
  const past = await loadPastPostmortems(payload);
  const research = await searchExa(
    [linearIssue?.title, linearIssue?.description, vercelLogs?.snippet]
      .filter(Boolean)
      .join(" ")
      .slice(0, 500)
  );
  const plan = await askHuggingFace({ source, linearIssue, vercelLogs, past, research });
  const branch = `fix/auto-repair-${Date.now()}`;
  const pr = await createFixBranchAndPr(branch, plan);
  if (linearIssue?.id || env("LINEAR_ISSUE_ID")) {
    await commentLinear(linearIssue?.id || env("LINEAR_ISSUE_ID"), plan, pr);
  }
  await savePostmortem({
    source,
    linearIssue,
    vercelLogs,
    plan,
    pr,
    createdAt: new Date(),
  });
  console.log("[agent] done", { branch, pr: pr?.html_url });
}

function safeJson(s: string) {
  try {
    return JSON.parse(s);
  } catch {
    return {};
  }
}

async function fetchLinearIssue(payload: ClientPayload) {
  const key = env("LINEAR_API_KEY");
  const id = payload.linear?.issueId || env("LINEAR_ISSUE_ID");
  if (!key || !id) {
    return {
      id,
      title: payload.linear?.title,
      description: payload.linear?.description,
      url: payload.linear?.url,
    };
  }
  const query = `query($id: String!) {
    issue(id: $id) { id identifier title description url state { name } labels { nodes { name } } }
  }`;
  const res = await fetch("https://api.linear.app/graphql", {
    method: "POST",
    headers: { Authorization: key, "Content-Type": "application/json" },
    body: JSON.stringify({ query, variables: { id } }),
  });
  const data = await res.json();
  return data?.data?.issue || payload.linear;
}

async function fetchVercelErrorContext(payload: ClientPayload) {
  const token = env("VERCEL_TOKEN");
  const teamId = env("VERCEL_ORG_ID");
  const projectId = payload.vercel?.projectId || env("VERCEL_PROJECT_ID");
  if (!token || !projectId) {
    return { snippet: payload.vercel ? JSON.stringify(payload.vercel) : "" };
  }
  const q = new URLSearchParams({
    projectId,
    limit: "5",
    state: "ERROR",
    ...(teamId ? { teamId } : {}),
  });
  const res = await fetch(`https://api.vercel.com/v6/deployments?${q}`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!res.ok) return { snippet: await res.text() };
  const data = await res.json();
  const dep = data.deployments?.[0];
  return {
    snippet: JSON.stringify(dep || data).slice(0, 4000),
    deploymentId: dep?.uid,
    url: dep?.url,
  };
}

async function loadPastPostmortems(payload: ClientPayload) {
  const uri = env("MONGODB_URI");
  if (!uri) return [];
  const client = new MongoClient(uri);
  try {
    await client.connect();
    const col = client.db("auto_healing_agent").collection("postmortems");
    const q = payload.linear?.title
      ? { "linearIssue.title": { $regex: String(payload.linear.title).slice(0, 40), $options: "i" } }
      : {};
    return await col.find(q).sort({ createdAt: -1 }).limit(5).toArray();
  } catch (e) {
    console.warn("[mongo] read failed", e);
    return [];
  } finally {
    await client.close().catch(() => {});
  }
}

async function searchExa(query: string) {
  const key = env("EXA_API_KEY");
  if (!key || !query.trim()) return "";
  try {
    const res = await fetch("https://api.exa.ai/search", {
      method: "POST",
      headers: { "x-api-key": key, "Content-Type": "application/json" },
      body: JSON.stringify({
        query: `fix software error: ${query}`,
        num_results: 5,
        contents: { text: true },
      }),
    });
    if (!res.ok) return "";
    const data = await res.json();
    return (data.results || [])
      .map((r: any) => `- ${r.title}: ${r.url}\n${(r.text || "").slice(0, 300)}`)
      .join("\n");
  } catch {
    return "";
  }
}

async function askHuggingFace(ctx: any) {
  const token = env("HF_TOKEN");
  const model = env("HF_MODEL", "Qwen/Qwen2.5-Coder-7B-Instruct");
  const base = env("HF_BASE_URL", "https://router.huggingface.co/v1").replace(/\/$/, "");
  const system = `You are a senior DevSecOps engineer. Propose a minimal safe code or workflow fix.
Return STRICT JSON with keys: analysis (string), solution (string), files (array of {path, content}), commit_message (string).`;
  const user = JSON.stringify(
    {
      source: ctx.source,
      issue: ctx.linearIssue,
      vercel: ctx.vercelLogs,
      past_postmortems: ctx.past,
      research: ctx.research,
    },
    null,
    2
  ).slice(0, 12000);
  if (!token) return fallbackPlan(ctx);
  try {
    const res = await fetch(`${base}/chat/completions`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        model,
        messages: [
          { role: "system", content: system },
          { role: "user", content: user },
        ],
        temperature: 0.2,
        max_tokens: 2048,
      }),
    });
    const data = await res.json();
    const text = data.choices?.[0]?.message?.content || "";
    const parsed = extractJson(text);
    if (parsed) return parsed;
    return {
      analysis: text.slice(0, 2000),
      solution: "See analysis",
      files: [],
      commit_message: "fix: auto-repair attempt",
    };
  } catch (e) {
    console.warn("[hf] failed", e);
    return fallbackPlan(ctx);
  }
}

function fallbackPlan(ctx: any) {
  return {
    analysis: `Automated triage for source=${ctx.source}. Issue: ${ctx.linearIssue?.title || "n/a"}`,
    solution:
      "No HF token or model response. Created tracking PR stub. Add HF_TOKEN and re-run.",
    files: [
      {
        path: "AUTO_REPAIR_REPORT.md",
        content: `# Auto-Repair Report\n\n${JSON.stringify(ctx, null, 2).slice(0, 3000)}\n`,
      },
    ],
    commit_message: "chore: auto-repair report stub",
  };
}

function extractJson(text: string) {
  const m = text.match(/\{[\s\S]*\}/);
  if (!m) return null;
  try {
    return JSON.parse(m[0]);
  } catch {
    return null;
  }
}

async function createFixBranchAndPr(branch: string, plan: any) {
  const token = env("GITHUB_TOKEN");
  const repoFull = env("GITHUB_REPOSITORY");
  if (!token || !repoFull) throw new Error("missing GITHUB_TOKEN or GITHUB_REPOSITORY");
  const [owner, repo] = repoFull.split("/");
  execSync(`git config user.email "auto-healing-agent[bot]@users.noreply.github.com"`);
  execSync(`git config user.name "auto-healing-agent"`);
  execSync(`git checkout -b ${branch}`);
  const files: { path: string; content: string }[] = plan.files || [];
  if (files.length === 0) {
    files.push({
      path: "AUTO_REPAIR_REPORT.md",
      content: `# Auto-Repair\n\n## Analysis\n${plan.analysis}\n\n## Solution\n${plan.solution}\n`,
    });
  }
  for (const f of files) {
    const parts = f.path.split("/");
    if (parts.length > 1) mkdirSync(parts.slice(0, -1).join("/"), { recursive: true });
    writeFileSync(f.path, f.content);
  }
  execSync(`git add -A`);
  try {
    execSync(`git commit -m ${JSON.stringify(plan.commit_message || "fix: auto-repair")}`);
  } catch {
    console.log("[git] nothing to commit");
  }
  execSync(`git push origin ${branch}`, {
    env: { ...process.env, GIT_TERMINAL_PROMPT: "0" },
  });
  const res = await fetch(`https://api.github.com/repos/${owner}/${repo}/pulls`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${token}`,
      Accept: "application/vnd.github+json",
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      title: `[auto-repair] ${String(plan.commit_message || "fix").slice(0, 70)}`,
      head: branch,
      base: "main",
      body: `## Analysis\n${plan.analysis}\n\n## Solution\n${plan.solution}\n\n_Generated by Auto-Healing Agent (legacy)_`,
    }),
  });
  if (!res.ok) {
    console.warn("[pr] failed", await res.text());
    return { html_url: `https://github.com/${owner}/${repo}/tree/${branch}` };
  }
  return await res.json();
}

async function commentLinear(issueId: string, plan: any, pr: any) {
  const key = env("LINEAR_API_KEY");
  if (!key || !issueId) return;
  const body = `## Auto-Healing Agent (legacy)\n\n**Analysis**\n${plan.analysis}\n\n**Solution**\n${plan.solution}\n\n**PR:** ${pr?.html_url || "n/a"}`;
  const mutation = `mutation($id: String!, $body: String!) {
    commentCreate(input: { issueId: $id, body: $body }) { success }
  }`;
  await fetch("https://api.linear.app/graphql", {
    method: "POST",
    headers: { Authorization: key, "Content-Type": "application/json" },
    body: JSON.stringify({ query: mutation, variables: { id: issueId, body } }),
  });
}

async function savePostmortem(doc: any) {
  const uri = env("MONGODB_URI");
  if (!uri) return;
  const client = new MongoClient(uri);
  try {
    await client.connect();
    await client.db("auto_healing_agent").collection("postmortems").insertOne(doc);
  } catch (e) {
    console.warn("[mongo] write failed", e);
  } finally {
    await client.close().catch(() => {});
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
