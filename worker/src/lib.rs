//! Cloudflare Worker — workers-rs pattern (Context7).
//! Health + MODEL_KV pointer. No train / no full repo build.
use worker::*;

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();
    Router::new()
        .get("/", |_, _| Response::ok("AUTO-REPAIR LAB"))
        .get("/health", |_, _| Response::ok("ok"))
        .get_async("/model", |_, ctx| async move {
            match ctx.kv("MODEL_KV") {
                Ok(kv) => {
                    let ptr = kv.get("model/current").text().await.ok().flatten();
                    match ptr {
                        Some(p) => Response::ok(format!(r#"{{"model_ptr":"{}"}}"#, p)),
                        None => Response::ok(r#"{"model_ptr":null}"#),
                    }
                }
                Err(_) => Response::ok(r#"{"model_ptr":null,"kv":"unbound"}"#),
            }
        })
        .post_async("/webhook", |mut req, ctx| async move {
            if let Ok(secret) = ctx.secret("WEBHOOK_SECRET") {
                let hdr = req
                    .headers()
                    .get("x-webhook-secret")?
                    .unwrap_or_default();
                if hdr != secret.to_string() {
                    return Response::error("unauthorized", 401);
                }
            }
            let _body = req.text().await.unwrap_or_default();
            // Heavy path: dispatch to GitHub Actions (VERIFY authority).
            Response::ok(r#"{"accepted":true,"verify":"github_actions"}"#)
        })
        .run(req, env)
        .await
}
