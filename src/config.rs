use crate::{
    agent::AgentSettings,
    budget::{Budget, BudgetLimits},
    llm::{HttpLlm, LlmClient, LlmMode, MockLlm, TokenLimitField},
    observer::ContextLimits,
};
use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use std::{env, net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Cli {
    #[arg(long, default_value = "default", value_parser = parse_run_id)]
    pub run_id: String,
    #[arg(long, default_value = "saves")]
    pub saves_dir: PathBuf,
    #[arg(
        long,
        value_name = "SESSION_DIR",
        help = "Replay a recorded session and print its final world snapshot"
    )]
    pub replay: Option<PathBuf>,
    #[arg(long, default_value_t = 42)]
    pub seed: u64,
    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..=60))]
    pub tick_rate: u32,
    #[arg(long)]
    pub mock_llm: bool,
    #[arg(
        long,
        help = "Total token cap. Existing saved cap or 250000 for a new run"
    )]
    pub max_tokens: Option<u64>,
    #[arg(long)]
    pub max_cost_usd: Option<f64>,
    #[arg(
        long,
        help = "Model iteration cap. Existing saved cap or 250 for a new run"
    )]
    pub max_iterations: Option<u64>,
    #[arg(long, default_value_t = 1_000, value_parser = clap::value_parser!(u64).range(100..=3_600_000))]
    pub cooldown_ms: u64,
    #[arg(long, default_value_t = 16_384, value_parser = parse::<4096, 131072>)]
    pub max_context_tokens: usize,
    #[arg(long, default_value_t = 2_048, value_parser = clap::value_parser!(u32).range(256..=16384))]
    pub max_output_tokens: u32,
    #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u32).range(1..=25))]
    pub max_repair_attempts: u32,
    #[arg(long, default_value_t = 40, value_parser = parse::<1, 100>)]
    pub max_log_entries: usize,
    #[arg(long, default_value_t = 25, value_parser = parse::<1, 100>)]
    pub max_errors: usize,
    #[arg(long, default_value_t = 20, value_parser = parse::<1, 100>)]
    pub max_journal_entries: usize,
    #[arg(long, default_value_t = 20_000, value_parser = parse::<1, 65536>)]
    pub max_source_bytes: usize,
    #[arg(long, default_value_t = 20, value_parser = parse::<1, 100>)]
    pub max_pending_tasks: usize,
    #[arg(long, help = "Enable the optional WebSocket interface")]
    pub http: bool,
    #[arg(long, default_value = "127.0.0.1:3210")]
    pub http_addr: SocketAddr,
}

impl Cli {
    pub fn run_dir(&self) -> PathBuf {
        self.saves_dir.join(&self.run_id)
    }

    pub fn agent_settings(&self) -> AgentSettings {
        AgentSettings {
            cooldown: Duration::from_millis(self.cooldown_ms),
            max_repair_attempts: self.max_repair_attempts,
            max_output_tokens: self.max_output_tokens,
            context: ContextLimits {
                max_context_tokens: self.max_context_tokens,
                max_log_entries: self.max_log_entries,
                max_errors: self.max_errors,
                max_journal_entries: self.max_journal_entries,
                max_source_bytes: self.max_source_bytes,
                max_pending_tasks: self.max_pending_tasks,
            },
        }
    }

    pub fn budget_limits(&self) -> Result<BudgetLimits> {
        let saved = Budget::saved_limits(&self.run_dir())?;
        let (input, output) = if self.mock_llm {
            (Some(0.0), Some(0.0))
        } else {
            (
                price("LLM_INPUT_COST_PER_1M")?,
                price("LLM_OUTPUT_COST_PER_1M")?,
            )
        };
        Ok(BudgetLimits {
            max_tokens: Some(
                self.max_tokens
                    .or_else(|| saved.as_ref().and_then(|limits| limits.max_tokens))
                    .unwrap_or(250_000),
            ),
            max_cost_usd: self
                .max_cost_usd
                .or_else(|| saved.as_ref().and_then(|limits| limits.max_cost_usd)),
            max_iterations: Some(
                self.max_iterations
                    .or_else(|| saved.as_ref().and_then(|limits| limits.max_iterations))
                    .unwrap_or(250),
            ),
            input_cost_per_1m: input,
            output_cost_per_1m: output,
        })
    }

    pub fn llm(&self) -> Result<Option<Arc<dyn LlmClient>>> {
        if self.mock_llm {
            return Ok(Some(Arc::new(MockLlm)));
        }
        let (Some(base), Some(model)) = (nonempty("LLM_BASE_URL")?, nonempty("LLM_MODEL")?) else {
            return Ok(None);
        };
        let key = nonempty("LLM_API_KEY")?;
        let mode = match nonempty("LLM_MODE")?.as_deref() {
            None | Some("tools") => LlmMode::Tools,
            Some("json") => LlmMode::Json,
            _ => bail!("LLM_MODE must be tools or json"),
        };
        let limit = match nonempty("LLM_TOKEN_LIMIT_FIELD")?.as_deref() {
            None | Some("max_completion_tokens") => TokenLimitField::MaxCompletionTokens,
            Some("max_tokens") => TokenLimitField::MaxTokens,
            _ => bail!("LLM_TOKEN_LIMIT_FIELD must be max_completion_tokens or max_tokens"),
        };
        let client = HttpLlm::new(&base, key.as_deref(), &model, mode, Duration::from_secs(45))?
            .with_token_limit_field(limit);
        Ok(Some(Arc::new(client)))
    }
}

fn nonempty(var: &str) -> Result<Option<String>> {
    match env::var(var) {
        Ok(value) => Ok(Some(value.trim().to_owned()).filter(|value| !value.is_empty())),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error).with_context(|| format!("reading {var}")),
    }
}

fn price(var: &str) -> Result<Option<f64>> {
    let Some(value) = nonempty(var)? else {
        return Ok(None);
    };
    let parsed: f64 = value
        .parse()
        .map_err(|_| anyhow!("{var} must be a nonnegative number"))?;
    if !parsed.is_finite() || parsed < 0.0 {
        bail!("{var} must be a finite nonnegative number");
    }
    Ok(Some(parsed))
}

fn parse<const MIN: usize, const MAX: usize>(value: &str) -> std::result::Result<usize, String> {
    value
        .parse()
        .ok()
        .filter(|n| (MIN..=MAX).contains(n))
        .ok_or_else(|| format!("expected an integer from {MIN} to {MAX}"))
}

fn parse_run_id(value: &str) -> std::result::Result<String, String> {
    let canonical = crate::world::validate_mod_name(value).map_err(|error| error.to_string())?;
    if canonical != value {
        return Err("run ID must be a single directory name".into());
    }
    Ok(canonical.into())
}
