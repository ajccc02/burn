use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::{llm::TokenUsage, persist};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BudgetLimits {
    pub max_tokens: Option<u64>,
    pub max_cost_usd: Option<f64>,
    pub max_iterations: Option<u64>,
    pub input_cost_per_1m: Option<f64>,
    pub output_cost_per_1m: Option<f64>,
}

impl Default for BudgetLimits {
    fn default() -> Self {
        Self {
            max_tokens: Some(150_000),
            max_cost_usd: None,
            max_iterations: Some(50),
            input_cost_per_1m: None,
            output_cost_per_1m: None,
        }
    }
}

impl BudgetLimits {
    pub fn validate(&self) -> Result<()> {
        for (name, number) in [
            ("maximum cost", self.max_cost_usd),
            ("input price", self.input_cost_per_1m),
            ("output price", self.output_cost_per_1m),
        ] {
            if let Some(number) = number {
                ensure!(
                    number.is_finite() && number >= 0.0,
                    "{name} must be finite and nonnegative"
                );
            }
        }
        ensure!(
            self.max_cost_usd.is_none() || self.prices().is_some(),
            "a USD spending cap requires LLM_INPUT_COST_PER_1M and LLM_OUTPUT_COST_PER_1M"
        );
        Ok(())
    }

    fn prices(&self) -> Option<Prices> {
        Some(Prices {
            input: self.input_cost_per_1m?,
            output: self.output_cost_per_1m?,
        })
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Prices {
    input: f64,
    output: f64,
}

impl Prices {
    fn cost(self, input: u64, output: u64) -> f64 {
        input as f64 / 1_000_000.0 * self.input + output as f64 / 1_000_000.0 * self.output
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Reservation {
    input: u64,
    output: u64,
    prices: Option<Prices>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Ledger {
    version: u32,
    #[serde(default)]
    limits: Option<BudgetLimits>,
    input_tokens: u64,
    output_tokens: u64,
    unpriced_input_tokens: u64,
    unpriced_output_tokens: u64,
    priced_cost_usd: f64,
    requests: u64,
    iterations: u64,
    estimated_requests: u64,
    pending: Option<Reservation>,
}

impl Default for Ledger {
    fn default() -> Self {
        Self {
            version: 1,
            limits: None,
            input_tokens: 0,
            output_tokens: 0,
            unpriced_input_tokens: 0,
            unpriced_output_tokens: 0,
            priced_cost_usd: 0.0,
            requests: 0,
            iterations: 0,
            estimated_requests: 0,
            pending: None,
        }
    }
}

impl Ledger {
    fn charge(&mut self, reservation: Reservation, usage: Option<TokenUsage>) -> Result<()> {
        let (input, output) = match usage {
            Some(usage) => (usage.input, usage.output),
            None => {
                self.estimated_requests = self.estimated_requests.saturating_add(1);
                (reservation.input, reservation.output)
            }
        };
        self.input_tokens = self.input_tokens.saturating_add(input);
        self.output_tokens = self.output_tokens.saturating_add(output);
        if let Some(prices) = reservation.prices {
            self.priced_cost_usd += prices.cost(input, output);
            ensure!(self.priced_cost_usd.is_finite(), "budget cost overflow");
        } else {
            self.unpriced_input_tokens = self.unpriced_input_tokens.saturating_add(input);
            self.unpriced_output_tokens = self.unpriced_output_tokens.saturating_add(output);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct BudgetSnapshot {
    pub limits: BudgetLimits,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub requests: u64,
    pub iterations: u64,
    pub estimated_requests: u64,
    pub cost_usd: Option<f64>,
    pub remaining_tokens: Option<u64>,
    pub remaining_cost_usd: Option<f64>,
    pub remaining_iterations: Option<u64>,
    pub exhausted: bool,
}

#[derive(Debug)]
pub struct Budget {
    path: PathBuf,
    ledger: Ledger,
    limits: BudgetLimits,
}

impl Budget {
    pub fn saved_limits(run_dir: &Path) -> Result<Option<BudgetLimits>> {
        let ledger: Option<Ledger> = persist::read_json(&run_dir.join("budget.json"))?;
        if let Some(ledger) = ledger {
            ensure!(
                ledger.version == 1,
                "unsupported budget schema version {}",
                ledger.version
            );
            if let Some(limits) = &ledger.limits {
                limits.validate()?;
            }
            return Ok(ledger.limits);
        }
        Ok(None)
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.ledger.pending.is_some()
    }

    pub fn open(run_dir: &Path, limits: BudgetLimits) -> Result<Self> {
        limits.validate()?;
        let path = run_dir.join("budget.json");
        ensure!(
            path.try_exists()? || !run_dir.join("agent/state.json").try_exists()?,
            "budget.json is missing from an initialized save. Restore the budget ledger before restarting the agent."
        );
        let mut ledger: Ledger = persist::read_json(&path)?.unwrap_or_default();
        ensure!(
            ledger.version == 1,
            "unsupported budget schema version {}",
            ledger.version
        );
        ensure!(
            ledger.priced_cost_usd.is_finite() && ledger.priced_cost_usd >= 0.0,
            "invalid persisted budget cost"
        );
        ledger.limits = Some(limits.clone());
        if let Some(pending) = ledger.pending.take() {
            if let Some(prices) = pending.prices {
                ensure!(
                    prices.input.is_finite()
                        && prices.input >= 0.0
                        && prices.output.is_finite()
                        && prices.output >= 0.0,
                    "invalid persisted reservation prices"
                );
            }
            ledger.charge(pending, None)?;
        }
        if let Some(prices) = limits.prices() {
            ledger.priced_cost_usd +=
                prices.cost(ledger.unpriced_input_tokens, ledger.unpriced_output_tokens);
            ensure!(ledger.priced_cost_usd.is_finite(), "budget cost overflow");
            ledger.unpriced_input_tokens = 0;
            ledger.unpriced_output_tokens = 0;
        }
        persist::write_json(&path, &ledger)?;
        Ok(Self {
            path,
            ledger,
            limits,
        })
    }

    pub fn snapshot(&self) -> BudgetSnapshot {
        let (pending_input, pending_output, pending_cost) =
            self.ledger
                .pending
                .as_ref()
                .map_or((0, 0, Some(0.0)), |pending| {
                    (
                        pending.input,
                        pending.output,
                        pending
                            .prices
                            .map(|rate| rate.cost(pending.input, pending.output)),
                    )
                });
        let input_tokens = self.ledger.input_tokens.saturating_add(pending_input);
        let output_tokens = self.ledger.output_tokens.saturating_add(pending_output);
        let total_tokens = input_tokens.saturating_add(output_tokens);
        let cost_usd = pending_cost
            .filter(|_| {
                self.ledger.unpriced_input_tokens == 0 && self.ledger.unpriced_output_tokens == 0
            })
            .map(|cost| cost + self.ledger.priced_cost_usd);
        let remaining_tokens = self
            .limits
            .max_tokens
            .map(|cap| cap.saturating_sub(total_tokens));
        let remaining_cost_usd = self
            .limits
            .max_cost_usd
            .zip(cost_usd)
            .map(|(cap, cost)| (cap - cost).max(0.0));
        let remaining_iterations = self
            .limits
            .max_iterations
            .map(|cap| cap.saturating_sub(self.ledger.iterations));
        let exhausted = remaining_tokens == Some(0)
            || remaining_iterations == Some(0)
            || (remaining_cost_usd == Some(0.0)
                && self
                    .limits
                    .prices()
                    .is_some_and(|prices| prices.input > 0.0 || prices.output > 0.0));
        BudgetSnapshot {
            limits: self.limits.clone(),
            input_tokens,
            output_tokens,
            total_tokens,
            requests: self.ledger.requests,
            iterations: self.ledger.iterations,
            estimated_requests: self.ledger.estimated_requests,
            cost_usd,
            remaining_tokens,
            remaining_cost_usd,
            remaining_iterations,
            exhausted,
        }
    }

    pub fn output_allowance(&self, input: u64, requested: u32) -> u32 {
        let snapshot = self.snapshot();
        if self.ledger.pending.is_some() || snapshot.exhausted {
            return 0;
        }
        let mut maximum = requested.min(snapshot.remaining_tokens.map_or(u32::MAX, |left| {
            u32::try_from(left.saturating_sub(input)).unwrap_or(u32::MAX)
        }));
        if let Some((cost_left, prices)) = snapshot.remaining_cost_usd.zip(self.limits.prices()) {
            let mut lower = 0;
            while lower < maximum {
                let middle = lower + (maximum - lower).div_ceil(2);
                if prices.cost(input, u64::from(middle)) <= cost_left {
                    lower = middle;
                } else {
                    maximum = middle - 1;
                }
            }
            maximum = lower;
        }
        maximum
    }

    pub fn reserve(&mut self, input: u64, output: u32) -> Result<()> {
        if output == 0 || self.output_allowance(input, output) != output {
            bail!("the next model request exceeds the available budget");
        }
        let mut next = self.ledger.clone();
        next.requests = next.requests.saturating_add(1);
        next.iterations = next.iterations.saturating_add(1);
        next.pending = Some(Reservation {
            input,
            output: u64::from(output),
            prices: self.limits.prices(),
        });
        self.commit(next)
    }

    pub fn settle(&mut self, usage: Option<TokenUsage>) -> Result<()> {
        let mut next = self.ledger.clone();
        let reservation = next
            .pending
            .take()
            .ok_or_else(|| anyhow::anyhow!("no model request is reserved"))?;
        next.charge(reservation, usage)?;
        self.commit(next)
    }

    fn commit(&mut self, next: Ledger) -> Result<()> {
        persist::write_json(&self.path, &next)?;
        self.ledger = next;
        Ok(())
    }
}
