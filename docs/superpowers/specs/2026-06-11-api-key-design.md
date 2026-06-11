# Design: Optional API Key for LLM Chat Panel

## Summary
Add an optional `llm_api_key` configuration field so users can authenticate with OpenAI-compatible API endpoints that require an `Authorization: Bearer <key>` header.

## Context
The `buff` TUI currently supports configurable `llm_base_url`, `llm_model`, and `llm_system_prompt`, but it sends requests without any authentication header. Many hosted OpenAI-compatible services (e.g., OpenRouter, Groq, Together, custom proxies) require an API key.

## Design

### 1. Config field
In `src/config.rs`, add a new field to `Config`:

```rust
pub llm_api_key: Option<String>,
```

- Default: `None`.
- Parsed from TOML as `llm_api_key = "sk-..."`.
- Existing configs without this field continue to work (defaults to `None`).

### 2. ChatRequest field
In `src/app/llm.rs`, add to `ChatRequest`:

```rust
pub api_key: Option<String>,
```

This carries the key from config down to the worker thread that makes the HTTP request.

### 3. HTTP header injection
In `llm.rs`'s `spawn()` function, when building the `ureq` request:

- If `req.api_key` is `Some(key)` and `!key.is_empty()`, call `.set("Authorization", &format!("Bearer {}", key))` on the request builder before `.send_json(body)`.
- If `None` or empty, skip the header (existing behavior).

### 4. Wiring in actions.rs
All three sites that construct `ChatRequest` populate `api_key` from `state.config.llm_api_key.clone()`:

- `handle_ask` — standard daily chat path.
- `handle_ask` — meeting assistant path.
- `fire_meeting_llm_call` — summary generation path.

### 5. Testing

#### Config tests
- Parse `llm_api_key = "sk-test"` from TOML and assert `Some("sk-test")`.
- Assert default config has `llm_api_key == None`.

#### LLM tests
- Verify `build_body` does not include the key in the JSON body.
- Add a test around `ureq` request construction showing the `Authorization` header is set when `api_key` is `Some`.

## Example config

```toml
llm_base_url = "https://api.openai.com/v1"
llm_model = "gpt-4o"
llm_api_key = "sk-..."
```

## Backward compatibility
Fully backward-compatible. Existing users with no `llm_api_key` see zero change in behavior.
