use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::time::Duration;

use reqwest::header::{HeaderValue, AUTHORIZATION};
use reqwest::{Client, Response, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const MAX_RESPONSE_BYTES: usize = 512 * 1024;
const MAX_ARGUMENT_BYTES: usize = 64 * 1024;
const MAX_TOOL_CALLS: usize = 8;
const MAX_TEXT_BYTES: usize = 4096;
const REQUEST_TOKEN_OVERHEAD: u64 = 128;

pub type LlmFuture<'a> = Pin<Box<dyn Future<Output = Result<LlmReply, LlmError>> + Send + 'a>>;

pub trait LlmClient: Send + Sync {
    fn complete<'a>(&'a self, request: &'a LlmRequest) -> LlmFuture<'a>;

    fn estimated_input_tokens(&self, request: &LlmRequest) -> u64 {
        conservative_tokens(&request)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct LlmRequest {
    pub system: String,
    pub observation: Value,
    pub tools: Vec<ToolDefinition>,
    pub max_output_tokens: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolCall {
    pub name: String,
    pub arguments: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LlmReply {
    pub calls: Vec<ToolCall>,
    pub text: Option<String>,
    pub usage: Option<TokenUsage>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LlmErrorKind {
    Temporary,
    InvalidResponse,
    Permanent,
}

#[derive(Clone, Debug, thiserror::Error)]
#[error("{message}")]
pub struct LlmError {
    pub kind: LlmErrorKind,
    pub message: String,
    pub usage: Option<TokenUsage>,
}

impl LlmError {
    pub fn temporary(message: impl Into<String>) -> Self {
        Self::new(LlmErrorKind::Temporary, message)
    }

    pub fn invalid_response(message: impl Into<String>) -> Self {
        Self::new(LlmErrorKind::InvalidResponse, message)
    }

    pub fn permanent(message: impl Into<String>) -> Self {
        Self::new(LlmErrorKind::Permanent, message)
    }

    pub fn is_temporary(&self) -> bool {
        self.kind == LlmErrorKind::Temporary
    }

    pub fn is_invalid_response(&self) -> bool {
        self.kind == LlmErrorKind::InvalidResponse
    }

    fn new(kind: LlmErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            usage: None,
        }
    }

    fn with_usage(mut self, usage: Option<TokenUsage>) -> Self {
        self.usage = usage;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LlmMode {
    Tools,
    Json,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenLimitField {
    MaxCompletionTokens,
    MaxTokens,
}

pub struct HttpLlm {
    client: Client,
    endpoint: Url,
    authorization: Option<HeaderValue>,
    model: String,
    mode: LlmMode,
    token_limit_field: TokenLimitField,
}

impl HttpLlm {
    pub fn new(
        base_url: &str,
        api_key: Option<&str>,
        model: &str,
        mode: LlmMode,
        timeout: Duration,
    ) -> Result<Self, LlmError> {
        let endpoint = chat_endpoint(base_url)?;
        let model = model.trim();
        if model.is_empty() || model.len() > 256 {
            return Err(LlmError::permanent(
                "Set LLM_MODEL to the configured model name",
            ));
        }
        if timeout.is_zero() {
            return Err(LlmError::permanent("Model timeout must be positive"));
        }
        let authorization = api_key
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map(|key| {
                let mut header = HeaderValue::from_str(&format!("Bearer {key}")).map_err(|_| {
                    LlmError::permanent("LLM_API_KEY contains invalid header characters")
                })?;
                header.set_sensitive(true);
                Ok::<_, LlmError>(header)
            })
            .transpose()?;
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(timeout.min(Duration::from_secs(10)))
            .timeout(timeout)
            .build()
            .map_err(|_| LlmError::permanent("Could not initialize the model HTTP client"))?;
        Ok(Self {
            client,
            endpoint,
            authorization,
            model: model.to_owned(),
            mode,
            token_limit_field: TokenLimitField::MaxCompletionTokens,
        })
    }

    pub fn with_token_limit_field(mut self, field: TokenLimitField) -> Self {
        self.token_limit_field = field;
        self
    }

    fn request_body(&self, request: &LlmRequest) -> Value {
        let mut system = request.system.clone();
        if self.mode == LlmMode::Json {
            system.push_str(
                "\nReturn exactly one JSON object with fields \"calls\" (an array of up to eight objects, each with \"name\" and object-valued \"arguments\") and \"text\" (a string or null). JSON only, without Markdown. Use the following tool definitions:\n",
            );
            system.push_str(&serde_json::to_string(&request.tools).unwrap_or_default());
        }
        let mut body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": request.observation.to_string()}
            ],
            "stream": false
        });
        let limit_key = match self.token_limit_field {
            TokenLimitField::MaxCompletionTokens => "max_completion_tokens",
            TokenLimitField::MaxTokens => "max_tokens",
        };
        body[limit_key] = json!(request.max_output_tokens);
        if self.mode == LlmMode::Tools && !request.tools.is_empty() {
            let functions = request
                .tools
                .iter()
                .map(|tool| json!({"type": "function", "function": tool}));
            body["tools"] = Value::Array(functions.collect());
            body["tool_choice"] = json!("auto");
        }
        body
    }

    async fn send(&self, request: &LlmRequest) -> Result<LlmReply, LlmError> {
        if request.max_output_tokens == 0 {
            return Err(LlmError::permanent(
                "Model request requires a positive output token limit",
            ));
        }
        let mut builder = self
            .client
            .post(self.endpoint.clone())
            .json(&self.request_body(request));
        if let Some(authorization) = &self.authorization {
            builder = builder.header(AUTHORIZATION, authorization.clone());
        }
        let response = builder.send().await.map_err(transport_error)?;
        let status = response.status();
        if !status.is_success() {
            return Err(status_error(status));
        }
        decode_response(&read_limited(response).await?, self.mode)
    }
}

impl LlmClient for HttpLlm {
    fn complete<'a>(&'a self, request: &'a LlmRequest) -> LlmFuture<'a> {
        Box::pin(self.send(request))
    }

    fn estimated_input_tokens(&self, request: &LlmRequest) -> u64 {
        conservative_tokens(&self.request_body(request))
    }
}

fn conservative_tokens(value: &impl Serialize) -> u64 {
    serde_json::to_vec(value)
        .map(|bytes| (bytes.len() as u64).saturating_add(REQUEST_TOKEN_OVERHEAD))
        .unwrap_or(u64::MAX)
}

fn chat_endpoint(base: &str) -> Result<Url, LlmError> {
    let invalid = || {
        LlmError::permanent(
            "Set LLM_BASE_URL to an HTTP(S) API base URL with no credentials, query, or fragment",
        )
    };
    let mut url = Url::parse(base.trim()).map_err(|_| invalid())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    let already_full = url
        .path()
        .trim_end_matches('/')
        .ends_with("/chat/completions");
    let mut segments = url.path_segments_mut().map_err(|_| invalid())?;
    segments.pop_if_empty();
    if !already_full {
        segments.extend(["chat", "completions"]);
    }
    drop(segments);
    Ok(url)
}

fn transport_error(error: reqwest::Error) -> LlmError {
    if error.is_timeout() {
        LlmError::temporary("Model request timed out")
    } else if error.is_builder() {
        LlmError::permanent("Could not construct the model request")
    } else {
        LlmError::temporary("Model endpoint connection or response failed")
    }
}

fn status_error(status: StatusCode) -> LlmError {
    let code = status.as_u16();
    match code {
        408 | 409 | 425 | 429 | 500..=599 => {
            LlmError::temporary(format!("Model endpoint returned HTTP {code}"))
        }
        401 | 403 => LlmError::permanent(format!(
            "Model endpoint returned HTTP {code}. Check LLM_API_KEY and model access."
        )),
        400 | 404 | 422 => LlmError::permanent(format!(
            "Model endpoint returned HTTP {code}. Check LLM_BASE_URL, LLM_MODEL, LLM_MODE, and LLM_TOKEN_LIMIT_FIELD."
        )),
        _ => LlmError::permanent(format!("Model endpoint returned HTTP {code}")),
    }
}

async fn read_limited(mut response: Response) -> Result<Vec<u8>, LlmError> {
    let too_large = || LlmError::invalid_response("Model response exceeds the response size limit");
    if response
        .content_length()
        .is_some_and(|len| len > MAX_RESPONSE_BYTES as u64)
    {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(transport_error)? {
        if chunk.len() > MAX_RESPONSE_BYTES - bytes.len() {
            return Err(too_large());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[derive(Deserialize)]
struct WireUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
}

#[derive(Deserialize)]
struct WireChoice {
    message: WireMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct WireMessage {
    content: Option<String>,
    tool_calls: Option<Vec<WireToolCall>>,
}

#[derive(Deserialize)]
struct WireToolCall {
    #[serde(rename = "type")]
    call_type: String,
    function: WireFunction,
}

#[derive(Deserialize)]
struct WireFunction {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonEnvelope {
    calls: Vec<ToolCall>,
    text: Option<String>,
}

fn decode_response(bytes: &[u8], mode: LlmMode) -> Result<LlmReply, LlmError> {
    let raw: Value = serde_json::from_slice(bytes)
        .map_err(|_| LlmError::invalid_response("Model HTTP response is invalid JSON"))?;
    let usage = serde_json::from_value::<WireUsage>(raw["usage"].clone())
        .ok()
        .map(|wire| TokenUsage {
            input: wire.prompt_tokens,
            output: wire.completion_tokens,
        });
    decode_choice(&raw, mode)
        .map(|(calls, text)| LlmReply { calls, text, usage })
        .map_err(|error| error.with_usage(usage))
}

fn decode_choice(raw: &Value, mode: LlmMode) -> Result<(Vec<ToolCall>, Option<String>), LlmError> {
    let choices = raw["choices"]
        .as_array()
        .filter(|choices| choices.len() == 1)
        .ok_or_else(|| {
            LlmError::invalid_response("Model returned an unexpected number of choices")
        })?;
    let choice: WireChoice = serde_json::from_value(choices[0].clone())
        .map_err(|_| LlmError::invalid_response("Model choice has an invalid message shape"))?;
    if matches!(
        choice.finish_reason.as_deref(),
        Some("length" | "content_filter")
    ) {
        return Err(LlmError::invalid_response(
            "Model did not finish producing a usable action",
        ));
    }
    let (calls, text) = match mode {
        LlmMode::Tools => {
            let wire_calls = choice.message.tool_calls.unwrap_or_default();
            if wire_calls.len() > MAX_TOOL_CALLS {
                return Err(LlmError::invalid_response(
                    "Model exceeded the tool call limit",
                ));
            }
            let calls = wire_calls
                .into_iter()
                .map(decode_wire_tool)
                .collect::<Result<Vec<_>, _>>()?;
            (calls, choice.message.content)
        }
        LlmMode::Json => {
            if choice
                .message
                .tool_calls
                .is_some_and(|calls| !calls.is_empty())
            {
                return Err(LlmError::invalid_response(
                    "JSON mode requires an action envelope in message content",
                ));
            }
            let content = choice.message.content.ok_or_else(|| {
                LlmError::invalid_response("JSON mode requires a message content string")
            })?;
            let envelope: JsonEnvelope = serde_json::from_str(&content).map_err(|_| {
                LlmError::invalid_response(
                    "Return a JSON object containing calls and optional text",
                )
            })?;
            (envelope.calls, envelope.text)
        }
    };
    validate_actions(&calls, text.as_deref())?;
    Ok((calls, text))
}

fn decode_wire_tool(call: WireToolCall) -> Result<ToolCall, LlmError> {
    if call.call_type != "function" || call.function.arguments.len() > MAX_ARGUMENT_BYTES {
        return Err(LlmError::invalid_response(
            "Model emitted an invalid function call type or size",
        ));
    }
    let arguments = serde_json::from_str(&call.function.arguments)
        .map_err(|_| LlmError::invalid_response("Function arguments must encode a JSON object"))?;
    Ok(ToolCall {
        name: call.function.name,
        arguments,
    })
}

fn validate_actions(calls: &[ToolCall], text: Option<&str>) -> Result<(), LlmError> {
    if calls.len() > MAX_TOOL_CALLS || text.is_some_and(|text| text.len() > MAX_TEXT_BYTES) {
        return Err(LlmError::invalid_response(
            "Model exceeded the tool call or text limit",
        ));
    }
    for call in calls {
        if call.name.is_empty()
            || call.name.len() > 64
            || !call
                .name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            || !call.arguments.is_object()
            || call.arguments.to_string().len() > MAX_ARGUMENT_BYTES
        {
            return Err(LlmError::invalid_response(
                "Each call requires a valid tool name and bounded object-valued arguments",
            ));
        }
    }
    Ok(())
}

pub struct ScriptedLlm {
    replies: Mutex<VecDeque<Result<LlmReply, LlmError>>>,
}

impl ScriptedLlm {
    pub fn new(replies: impl IntoIterator<Item = Result<LlmReply, LlmError>>) -> Self {
        Self {
            replies: Mutex::new(replies.into_iter().collect()),
        }
    }
}

impl LlmClient for ScriptedLlm {
    fn complete<'a>(&'a self, _request: &'a LlmRequest) -> LlmFuture<'a> {
        Box::pin(async {
            self.replies
                .lock()
                .map_err(|_| LlmError::permanent("Mock model queue is unavailable"))?
                .pop_front()
                .unwrap_or_else(|| Err(LlmError::permanent("Mock model reply queue exhausted")))
        })
    }
}

const MERCHANT_SCRIPT: &str = include_str!("../examples/mods/merchant.rhai");
const VOLCANO_SCRIPT: &str = include_str!("../examples/mods/volcano.rhai");

#[derive(Default)]
pub struct MockLlm;

impl LlmClient for MockLlm {
    fn complete<'a>(&'a self, request: &'a LlmRequest) -> LlmFuture<'a> {
        Box::pin(async move {
            let observation = &request.observation;
            let mut reply =
                mock_call(observation).map_or_else(LlmReply::default, |call| LlmReply {
                    calls: vec![call],
                    ..LlmReply::default()
                });
            let input = (request.observation.to_string().len() as u64).div_ceil(4) + 32;
            let output = (serde_json::to_vec(&reply.calls).unwrap_or_default().len() as u64)
                .div_ceil(4)
                + 16;
            if output > u64::from(request.max_output_tokens) {
                return Err(LlmError::invalid_response(
                    "Mock reply exceeded its output token limit",
                )
                .with_usage(Some(TokenUsage {
                    input,
                    output: u64::from(request.max_output_tokens),
                })));
            }
            reply.usage = Some(TokenUsage { input, output });
            Ok(reply)
        })
    }
}

fn mock_call(observation: &Value) -> Option<ToolCall> {
    let task = observation.get("task")?.as_object()?;
    let instruction = task
        .get("instruction")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let instruction = instruction.strip_prefix("please ").unwrap_or(&instruction);
    let last_action = task.get("last_action").and_then(Value::as_str);
    let repairing = task.get("kind").and_then(Value::as_str) == Some("repair")
        || instruction.starts_with("fix ")
        || instruction.starts_with("repair ");
    let demo = match task.get("target_mod").and_then(Value::as_str) {
        Some("merchant" | "merchant.rhai") => Some(Demo::Merchant),
        Some("volcano" | "volcano.rhai") => Some(Demo::Volcano),
        Some(_) => None,
        None => Demo::from_instruction(instruction),
    };
    let world_summary = instruction.starts_with("show ")
        || instruction.starts_with("list ")
        || instruction.starts_with("inspect ")
        || instruction.starts_with("status");
    let action = if world_summary {
        mock_read_only(
            observation,
            last_action,
            "The offline model can inspect this world and build the merchant and volcano examples.",
        )
    } else if let Some(demo) = demo {
        let mutation_tick = task.get("mutation_tick").and_then(Value::as_u64);
        let tested_ticks = task
            .get("tested_ticks")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let unchanged = observation["results"].as_array().is_some_and(|results| {
            results.iter().any(|result| {
                result["tool"] == "write_mod"
                    && result["ok"] == true
                    && result["data"]["changed"] == false
            })
        });
        if !repairing && mutation_tick.is_none() && unchanged {
            mock_read_only(
                observation,
                last_action,
                "The requested example is already installed with the same source.",
            )
        } else {
            demo.next_action(repairing, last_action, mutation_tick, tested_ticks)
        }
    } else {
        mock_read_only(
            observation,
            last_action,
            "The offline model handles merchant and volcano examples. Configure a live model for this request.",
        )
    };
    Some(action)
}

fn mock_read_only(observation: &Value, last_action: Option<&str>, message: &str) -> ToolCall {
    if last_action != Some("get_world_snapshot") {
        return call("get_world_snapshot", json!({}));
    }
    let world = &observation["world"];
    let tick = world["tick"].as_u64().unwrap_or(0);
    let count = world["entity_count"]
        .as_u64()
        .or_else(|| world["entities"].as_array().map(|items| items.len() as u64))
        .unwrap_or(0);
    let text = format!("The world is at tick {tick} with {count} observed entities. {message}");
    call("send_message_to_player", json!({ "text": text }))
}

#[derive(Clone, Copy)]
enum Demo {
    Merchant,
    Volcano,
}

impl Demo {
    fn from_instruction(instruction: &str) -> Option<Self> {
        let (verb, subject) = instruction.split_once(' ')?;
        if !matches!(verb, "add" | "create" | "make" | "build" | "fix" | "repair") {
            return None;
        }
        let subject = subject
            .strip_prefix("the ")
            .or_else(|| subject.strip_prefix("a "))
            .unwrap_or(subject);
        let subject = subject
            .strip_prefix("wandering ")
            .or_else(|| subject.strip_prefix("broken "))
            .unwrap_or(subject);
        match subject
            .split(|char: char| !char.is_ascii_alphabetic())
            .next()?
        {
            "merchant" => Some(Self::Merchant),
            "volcano"
                if instruction
                    .split_once("every ")
                    .is_none_or(|(_, cadence)| cadence.split_whitespace().next() == Some("60")) =>
            {
                Some(Self::Volcano)
            }
            _ => None,
        }
    }

    fn next_action(
        self,
        repairing: bool,
        last_action: Option<&str>,
        mutation_tick: Option<u64>,
        tested_ticks: u64,
    ) -> ToolCall {
        let (name, source, ticks, event, event_count, message) = match self {
            Self::Merchant => (
                "merchant",
                MERCHANT_SCRIPT,
                6,
                if repairing {
                    "merchant_patrol"
                } else {
                    "trade"
                },
                1,
                "The wandering merchant is active. I observed a bread-for-gold trade.",
            ),
            Self::Volcano => (
                "volcano",
                VOLCANO_SCRIPT,
                120,
                "eruption",
                2,
                "The volcano is active. I observed two eruptions while testing its 60-tick timer.",
            ),
        };
        if (mutation_tick.is_none() && last_action == Some("write_mod"))
            || last_action == Some("verify_task")
        {
            return call(
                "report_bug",
                json!({
                    "summary": format!("The offline {name} example did not pass its world checks."),
                    "repro": format!("Inspect /errors and try the {name} example in the current world.")
                }),
            );
        }
        if mutation_tick.is_none() {
            let read_tool = if repairing {
                "read_mod"
            } else {
                "get_world_snapshot"
            };
            if last_action.is_none() {
                return if repairing {
                    call(read_tool, json!({ "name": name }))
                } else {
                    call(read_tool, json!({}))
                };
            }
            return call("write_mod", json!({ "name": name, "source": source }));
        }
        if last_action == Some("write_mod") {
            return call("reload_mods", json!({}));
        }
        if tested_ticks < ticks {
            return call("run_ticks", json!({ "n": ticks - tested_ticks }));
        }
        let outcome = if repairing {
            format!(
                "I repaired the {name} mod and verified it while advancing {tested_ticks} ticks."
            )
        } else {
            message.to_owned()
        };
        let mut event_check = json!({ "kind": "event", "event": event, "min_count": event_count });
        if matches!(self, Self::Volcano) {
            event_check["every_ticks"] = json!(60);
        }
        call(
            "verify_task",
            json!({
                "checks": [
                    { "kind": "mod", "name": name },
                    { "kind": "entity", "entity_kind": name, "min_count": 1, "props": {"demo":name} },
                    event_check
                ],
                "message": outcome
            }),
        )
    }
}

fn call(name: &str, arguments: Value) -> ToolCall {
    ToolCall {
        name: name.to_owned(),
        arguments,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    #[test]
    fn normalizes_base_urls_and_rejects_embedded_credentials() {
        for base in [
            "http://127.0.0.1:1234/v1",
            "http://127.0.0.1:1234/v1/",
            "http://127.0.0.1:1234/v1/chat/completions/",
        ] {
            assert_eq!(
                chat_endpoint(base).unwrap().as_str(),
                "http://127.0.0.1:1234/v1/chat/completions"
            );
        }
        for base in [
            "ftp://host.test/v1",
            "https://user:private@host.test/v1",
            "https://host.test/v1?token=private",
            "https://host.test/v1#private",
        ] {
            let error = chat_endpoint(base).unwrap_err();
            assert_eq!(error.kind, LlmErrorKind::Permanent);
            assert!(!error.to_string().contains("private"));
        }
    }

    #[test]
    fn decodes_function_calls_and_retains_usage_on_malformed_actions() {
        let outcome = json!({
            "choices": [{
                "finish_reason": "tool_calls",
                "message": { "content": null, "tool_calls": [{
                    "type": "function",
                    "function": { "name": "run_ticks", "arguments": "{\"n\":2}" }
                }] }
            }],
            "usage": { "prompt_tokens": 93, "completion_tokens": 8 }
        });
        let bytes = serde_json::to_vec(&outcome).unwrap();
        let reply = decode_response(&bytes, LlmMode::Tools).unwrap();
        assert_eq!(reply.calls[0].name, "run_ticks");
        assert_eq!(reply.calls[0].arguments, json!({"n": 2}));
        assert_eq!(
            reply.usage,
            Some(TokenUsage {
                input: 93,
                output: 8
            })
        );

        let mut broken = outcome;
        broken["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"] = json!("[]");
        let bytes = serde_json::to_vec(&broken).unwrap();
        let error = decode_response(&bytes, LlmMode::Tools).unwrap_err();
        assert!(error.is_invalid_response());
        assert_eq!(error.usage, reply.usage);
    }

    #[test]
    fn enforces_json_envelope_and_output_limits() {
        let encode = |content: &str| {
            serde_json::to_vec(&json!({
                "choices": [{ "finish_reason": "stop", "message": { "content": content } }]
            }))
            .unwrap()
        };
        let reply =
            decode_response(&encode(r#"{"calls":[],"text":"Ready."}"#), LlmMode::Json).unwrap();
        assert_eq!(reply.text.as_deref(), Some("Ready."));
        for content in [
            r#"{"calls":[],"text":"x","surprise":true}"#,
            r#"{"calls":[{"name":"a","arguments":"b"}]}"#,
            "plain text",
        ] {
            assert!(decode_response(&encode(content), LlmMode::Json).is_err());
        }
        let oversized = json!({ "calls": [], "text": "x".repeat(MAX_TEXT_BYTES + 1) });
        assert!(decode_response(&encode(&oversized.to_string()), LlmMode::Json).is_err());
    }

    #[test]
    fn backs_off_only_retryable_http_statuses() {
        assert!(status_error(StatusCode::TOO_MANY_REQUESTS).is_temporary());
        assert!(status_error(StatusCode::SERVICE_UNAVAILABLE).is_temporary());
        assert!(!status_error(StatusCode::UNAUTHORIZED).is_temporary());
        assert!(!status_error(StatusCode::BAD_REQUEST).is_temporary());
    }

    #[test]
    fn unchanged_mock_example_finishes_with_a_fresh_observation() {
        let mut observation = json!({
            "task": {
                "kind":"player", "instruction":"Add a merchant.",
                "last_action":"write_mod", "mutation_tick":null
            },
            "results":[{"tool":"write_mod","ok":true,"data":{"changed":false}}],
            "world":{"tick":20,"entities":[{"kind":"merchant"}]}
        });
        assert_eq!(mock_call(&observation).unwrap().name, "get_world_snapshot");
        observation["task"]["last_action"] = json!("get_world_snapshot");
        let report = mock_call(&observation).unwrap();
        assert_eq!(report.name, "send_message_to_player");
        assert!(report.arguments["text"]
            .as_str()
            .unwrap()
            .contains("already installed"));
    }

    #[tokio::test]
    async fn http_client_sends_bounded_requests_in_both_modes() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            for mode in [LlmMode::Tools, LlmMode::Json] {
                let (socket, _) = listener.accept().await.unwrap();
                let mut reader = BufReader::new(socket);
                let mut request_line = String::new();
                reader.read_line(&mut request_line).await.unwrap();
                assert_eq!(request_line, "POST /v1/chat/completions HTTP/1.1\r\n");
                let mut length = None;
                let mut bearer_received = false;
                loop {
                    let mut header = String::new();
                    let received = reader.read_line(&mut header).await.unwrap();
                    assert_ne!(received, 0, "connection ended during HTTP headers");
                    if header == "\r\n" {
                        break;
                    }
                    let (name, value) = header.split_once(':').unwrap();
                    if name.eq_ignore_ascii_case("content-length") {
                        length = Some(value.trim().parse::<usize>().unwrap());
                    }
                    if name.eq_ignore_ascii_case("authorization") {
                        bearer_received = value.trim() == "Bearer test-token";
                    }
                }
                assert!(bearer_received);
                let length = length.unwrap();
                assert!(length <= 4096);
                let mut bytes = vec![0; length];
                reader.read_exact(&mut bytes).await.unwrap();
                let body: Value = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(body["model"], "local-demo");
                assert_eq!(body["stream"], false);
                assert_eq!(body["messages"][1]["content"], r#"{"tick":42}"#);
                let (message, finish_reason) = match mode {
                    LlmMode::Tools => {
                        assert_eq!(body["max_completion_tokens"], 128);
                        assert_eq!(body["tools"][0]["function"]["name"], "get_world_snapshot");
                        (
                            json!({
                                "tool_calls": [{
                                    "type": "function",
                                    "function": {"name": "get_world_snapshot", "arguments": "{}"}
                                }]
                            }),
                            "tool_calls",
                        )
                    }
                    LlmMode::Json => {
                        assert_eq!(body["max_tokens"], 128);
                        assert!(body.get("tools").is_none());
                        assert!(body["messages"][0]["content"]
                            .as_str()
                            .unwrap()
                            .contains("\"name\":\"get_world_snapshot\""));
                        let content = json!({
                            "calls": [{"name": "get_world_snapshot", "arguments": {}}],
                            "text": null
                        });
                        (json!({"content": content.to_string()}), "stop")
                    }
                };
                let response = json!({
                    "choices": [{"message": message, "finish_reason": finish_reason}],
                    "usage": {"prompt_tokens": 100, "completion_tokens": 10}
                })
                .to_string();
                let wire = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{response}",
                    response.len()
                );
                reader.get_mut().write_all(wire.as_bytes()).await.unwrap();
            }
        });
        let request = LlmRequest {
            system: "Use world tools.".to_owned(),
            observation: json!({"tick": 42}),
            tools: vec![ToolDefinition {
                name: "get_world_snapshot".to_owned(),
                description: "Read the current state.".to_owned(),
                parameters: json!({"type": "object", "properties": {}, "additionalProperties": false}),
            }],
            max_output_tokens: 128,
        };
        for (mode, field) in [
            (LlmMode::Tools, TokenLimitField::MaxCompletionTokens),
            (LlmMode::Json, TokenLimitField::MaxTokens),
        ] {
            let mut client = HttpLlm::new(
                &endpoint,
                Some("test-token"),
                "local-demo",
                mode,
                Duration::from_secs(5),
            )
            .unwrap()
            .with_token_limit_field(field);
            // The loopback test is independent of the workstation's proxy settings.
            client.client = Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap();
            let reply = client.complete(&request).await.unwrap();
            assert_eq!(reply.calls[0].name, "get_world_snapshot");
            assert_eq!(reply.calls[0].arguments, json!({}));
            assert_eq!(
                reply.usage,
                Some(TokenUsage {
                    input: 100,
                    output: 10
                })
            );
        }
        server.await.unwrap();
    }
}
