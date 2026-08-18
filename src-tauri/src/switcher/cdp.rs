use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::switcher::{ApiError, ApiResult};

pub async fn inject_model_catalog(port: u16, models: &[Value]) -> ApiResult<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|error| {
            ApiError::detailed(
                "cdp_client_failed",
                "创建 CDP 客户端失败。",
                error.to_string(),
            )
        })?;
    let endpoint = format!("http://127.0.0.1:{port}/json");
    let targets = client
        .get(&endpoint)
        .send()
        .await
        .map_err(|error| {
            ApiError::detailed(
                "cdp_target_query_failed",
                "无法连接 Codex 调试端口。",
                error.to_string(),
            )
        })?
        .json::<Vec<Value>>()
        .await
        .map_err(|error| {
            ApiError::detailed(
                "cdp_target_parse_failed",
                "解析 Codex 调试目标失败。",
                error.to_string(),
            )
        })?;

    let websocket_url = targets
        .iter()
        .filter(|target| target.get("type").and_then(Value::as_str) == Some("page"))
        .find_map(|target| target.get("webSocketDebuggerUrl").and_then(Value::as_str))
        .or_else(|| {
            targets
                .iter()
                .find_map(|target| target.get("webSocketDebuggerUrl").and_then(Value::as_str))
        })
        .ok_or_else(|| ApiError::new("cdp_target_missing", "Codex 没有暴露可注入的页面目标。"))?;

    let (mut socket, _) = connect_async(websocket_url).await.map_err(|error| {
        ApiError::detailed(
            "cdp_websocket_failed",
            "连接 Codex CDP WebSocket 失败。",
            error.to_string(),
        )
    })?;
    let script = build_injection_script(models)?;

    send_command(&mut socket, 1, "Page.enable", json!({})).await?;
    send_command(
        &mut socket,
        2,
        "Page.addScriptToEvaluateOnNewDocument",
        json!({ "source": script }),
    )
    .await?;
    send_command(
        &mut socket,
        3,
        "Runtime.evaluate",
        json!({
            "expression": script,
            "awaitPromise": false,
            "returnByValue": true
        }),
    )
    .await?;
    let _ = socket.close(None).await;
    Ok(())
}

async fn send_command<S>(
    socket: &mut tokio_tungstenite::WebSocketStream<S>,
    id: u64,
    method: &str,
    params: Value,
) -> ApiResult<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    socket
        .send(Message::Text(
            json!({ "id": id, "method": method, "params": params })
                .to_string()
                .into(),
        ))
        .await
        .map_err(|error| {
            ApiError::detailed(
                "cdp_command_send_failed",
                format!("发送 CDP 命令 {method} 失败。"),
                error.to_string(),
            )
        })?;

    while let Some(message) = socket.next().await {
        let message = message.map_err(|error| {
            ApiError::detailed(
                "cdp_command_read_failed",
                format!("读取 CDP 命令 {method} 结果失败。"),
                error.to_string(),
            )
        })?;
        let Message::Text(text) = message else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        if value.get("id").and_then(Value::as_u64) != Some(id) {
            continue;
        }
        if let Some(error) = value.get("error") {
            return Err(ApiError::detailed(
                "cdp_command_rejected",
                format!("Codex 拒绝 CDP 命令 {method}。"),
                error.to_string(),
            ));
        }
        return Ok(());
    }

    Err(ApiError::new(
        "cdp_connection_closed",
        format!("等待 CDP 命令 {method} 时连接已关闭。"),
    ))
}

fn build_injection_script(models: &[Value]) -> ApiResult<String> {
    let ui_models: Vec<Value> = models
        .iter()
        .filter_map(|model| {
            let mut model = model.clone();
            let object = model.as_object_mut()?;
            let slug = object.get("slug")?.as_str()?.to_string();
            let display_name = object
                .get("display_name")
                .and_then(Value::as_str)
                .unwrap_or(&slug)
                .to_string();
            object.insert("id".to_string(), json!(slug));
            object.insert("name".to_string(), json!(display_name));
            Some(model)
        })
        .collect();
    let serialized = serde_json::to_string(&ui_models).map_err(|error| {
        ApiError::detailed(
            "cdp_models_serialize_failed",
            "序列化模型注入数据失败。",
            error.to_string(),
        )
    })?;

    Ok(format!(
        r#"
(() => {{
  const customModels = {serialized};
  const state = window.__CODEX_API_SWITCHER__ || {{}};
  state.models = customModels;
  state.installedAt = Date.now();
  window.__CODEX_API_SWITCHER__ = state;

  const mergeModels = (value) => {{
    if (!value || typeof value !== "object") return value;
    const mergeArray = (items) => {{
      const existing = new Set(items.map((item) => item && (item.slug || item.id || item.model)));
      for (const model of customModels) {{
        if (!existing.has(model.slug)) items.push({{ ...model }});
      }}
      return items;
    }};
    if (Array.isArray(value)) return mergeArray(value);
    for (const key of ["models", "data", "items", "available_models", "availableModels"]) {{
      if (Array.isArray(value[key])) mergeArray(value[key]);
    }}
    return value;
  }};

  if (!state.responsePatched) {{
    const originalJson = Response.prototype.json;
    Response.prototype.json = async function (...args) {{
      const value = await originalJson.apply(this, args);
      try {{
        const url = String(this.url || "");
        if (/model|catalog/i.test(url)) mergeModels(value);
      }} catch (_) {{}}
      return value;
    }};
    state.responsePatched = true;
  }}

  window.dispatchEvent(new CustomEvent("codex-api-switcher:models", {{
    detail: {{ models: customModels }}
  }}));
}})();
"#
    ))
}
