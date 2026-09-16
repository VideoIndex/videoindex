//! `GET /v1/openapi.json`: a compact OpenAPI 3.1 description of the routes.

use axum::Json;
use serde_json::{json, Value};

/// The document.
pub async fn document() -> Json<Value> {
    Json(spec())
}

fn op(summary: &str, tag: &str) -> Value {
    json!({"summary": summary, "tags": [tag], "responses": {"200": {"description": "OK"}}})
}

fn json_body(schema: Value) -> Value {
    json!({"required": true, "content": {"application/json": {"schema": schema}}})
}

/// Build the spec.
pub fn spec() -> Value {
    let index_param =
        json!({"name": "index", "in": "path", "required": true, "schema": {"type": "string"}});
    let video_param =
        json!({"name": "video", "in": "path", "required": true, "schema": {"type": "string"}});
    let job_param =
        json!({"name": "job", "in": "path", "required": true, "schema": {"type": "string"}});
    let mut paths = serde_json::Map::new();
    paths.insert(
        "/healthz".into(),
        json!({"get": op("Liveness, version, known indexes", "system")}),
    );
    paths.insert(
        "/metrics".into(),
        json!({"get": op("Prometheus metrics", "system")}),
    );
    paths.insert(
        "/v1/openapi.json".into(),
        json!({"get": op("This document", "system")}),
    );
    paths.insert("/v1/models".into(), json!({
        "get": {"summary": "Chat models `ask` accepts as `model`: provider name, adapter, model id, and which one the agent role uses by default", "tags": ["query"],
                "responses": {"200": {"description": "models", "content": {"application/json": {"schema": {"type": "object", "properties": {"models": {"type": "array", "items": {"type": "object", "properties": {
                    "provider": {"type": "string"}, "adapter": {"type": "string"}, "model": {"type": "string"}, "default": {"type": "boolean"}}}}}}}}}}}
    }));
    paths.insert("/v1/indexes".into(), json!({
        "get": op("List indexes under the index root", "indexes"),
        "post": {"summary": "Create an empty index", "tags": ["indexes"],
                 "requestBody": json_body(json!({"type": "object", "required": ["id"], "properties": {"id": {"type": "string"}}})),
                 "responses": {"201": {"description": "Created"}, "409": {"description": "Exists"}}}
    }));
    paths.insert("/v1/indexes/{index}".into(), json!({"get": {"summary": "Index status: videos, counts, sizes", "tags": ["indexes"], "parameters": [index_param], "responses": {"200": {"description": "OK"}}}}));
    paths.insert("/v1/indexes/{index}/videos".into(), json!({
        "get": {"summary": "Videos in the index", "tags": ["videos"], "parameters": [index_param], "responses": {"200": {"description": "OK"}}},
        "post": {"summary": "Add sources; returns a job id (202)", "tags": ["jobs"], "parameters": [index_param],
                 "requestBody": json_body(json!({"type": "object", "required": ["sources"], "properties": {
                     "sources": {"type": "array", "items": {"type": "string"}, "description": "paths, directories, https://, s3://, YouTube URLs"},
                     "policy": {"type": "string"}, "force": {"type": "boolean", "default": false}}})),
                 "responses": {"202": {"description": "Accepted", "content": {"application/json": {"schema": {"type": "object", "properties": {"job_id": {"type": "string"}}}}}}}}
    }));
    paths.insert("/v1/indexes/{index}/videos/{video}".into(), json!({"get": {"summary": "One video with its tracks", "tags": ["videos"], "parameters": [index_param, video_param], "responses": {"200": {"description": "OK"}}}}));
    paths.insert("/v1/indexes/{index}/videos/{video}/timeline".into(), json!({"get": {"summary": "Segments at a level", "tags": ["videos"], "parameters": [index_param, video_param,
        {"name": "level", "in": "query", "schema": {"type": "string", "enum": ["chapter", "scene", "shot"], "default": "chapter"}}], "responses": {"200": {"description": "OK"}}}}));
    paths.insert("/v1/indexes/{index}/videos/{video}/transcript".into(), json!({"get": {"summary": "Transcript, OCR or descriptions in a time range", "tags": ["videos"], "parameters": [index_param, video_param,
        {"name": "t0", "in": "query", "schema": {"type": "number", "default": 0}}, {"name": "t1", "in": "query", "schema": {"type": "number"}},
        {"name": "kind", "in": "query", "schema": {"type": "string", "enum": ["transcript", "ocr", "description"], "default": "transcript"}}], "responses": {"200": {"description": "OK"}}}}));
    paths.insert("/v1/indexes/{index}/search".into(), json!({"post": {"summary": "Hybrid search", "tags": ["query"], "parameters": [index_param],
        "requestBody": json_body(json!({"type": "object", "required": ["query"], "properties": {
            "query": {"type": "string"}, "k": {"type": "integer", "default": 10},
            "videos": {"type": "array", "items": {"type": "string"}},
            "kinds": {"type": "array", "items": {"type": "string", "enum": ["transcript", "ocr", "description", "frame"]}},
            "text_only": {"type": "boolean", "default": false}}})),
        "responses": {"200": {"description": "hits, lists, grouping"}}}}));
    paths.insert("/v1/indexes/{index}/ask".into(), json!({"post": {"summary": "Agentic answer; SSE with Accept: text/event-stream (events: status, tool_call, tool_result, token, citation, done), else JSON", "tags": ["query"], "parameters": [index_param],
        "requestBody": json_body(json!({"type": "object", "required": ["question"], "properties": {
            "question": {"type": "string"}, "videos": {"type": "array", "items": {"type": "string"}},
            "budget": {"type": "object", "properties": {"max_tokens": {"type": "integer"}, "max_cost_usd": {"type": "number"}, "max_wallclock_secs": {"type": "number"}, "max_tool_calls": {"type": "integer"}}},
            "session_id": {"type": "string"}, "policy": {"type": "string", "enum": ["agent", "retrieval-only"], "default": "agent"},
            "model": {"type": "string", "description": "provider name or model id from GET /v1/models; default: the agent_llm role"}}})),
        "responses": {"200": {"description": "answer"}, "429": {"description": "daily spend cap reached"}}}}));
    paths.insert("/v1/indexes/{index}/view".into(), json!({"post": {"summary": "Labelled frame grid as PNG", "tags": ["query"], "parameters": [index_param],
        "requestBody": json_body(json!({"type": "object", "required": ["video_id", "t0", "t1"], "properties": {
            "video_id": {"type": "string"}, "t0": {"type": "number"}, "t1": {"type": "number"}, "fps": {"type": "number", "default": 1}, "cols": {"type": "integer", "default": 3}, "max_dim": {"type": "integer"}}})),
        "responses": {"200": {"description": "image/png"}}}}));
    paths.insert("/v1/indexes/{index}/blobs/{key}".into(), json!({"get": {"summary": "Thumbnail or grid bytes", "tags": ["blobs"], "parameters": [index_param, {"name": "key", "in": "path", "required": true, "schema": {"type": "string"}}], "responses": {"200": {"description": "image"}}}}));
    paths.insert(
        "/v1/jobs".into(),
        json!({"get": op("Jobs of this process", "jobs")}),
    );
    paths.insert("/v1/jobs/{job}".into(), json!({
        "get": {"summary": "Job status; SSE progress with Accept: text/event-stream", "tags": ["jobs"], "parameters": [job_param], "responses": {"200": {"description": "OK"}}},
        "delete": {"summary": "Cancel", "tags": ["jobs"], "parameters": [job_param], "responses": {"200": {"description": "OK"}}}
    }));
    paths.insert("/v1/mcp".into(), json!({"post": {"summary": "MCP (streamable HTTP, JSON-RPC 2.0) over the default index", "tags": ["mcp"], "responses": {"200": {"description": "JSON-RPC response"}, "202": {"description": "notification accepted"}}}}));
    paths.insert("/v1/indexes/{index}/mcp".into(), json!({"post": {"summary": "MCP over one index", "tags": ["mcp"], "parameters": [index_param], "responses": {"200": {"description": "JSON-RPC response"}}}}));
    json!({
        "openapi": "3.1.0",
        "info": {"title": "VideoIndex API", "version": env!("CARGO_PKG_VERSION"),
                 "description": "Index long videos and query them. Timestamps are seconds. Authentication: `Authorization: Bearer <key>` when keys are configured."},
        "servers": [{"url": "/"}],
        "components": {"securitySchemes": {"bearer": {"type": "http", "scheme": "bearer"}}},
        "security": [{"bearer": []}],
        "paths": Value::Object(paths),
    })
}
