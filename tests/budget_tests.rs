use spawn_lite::{
    budget::{Budget, BudgetLimits},
    llm::TokenUsage,
};

#[test]
fn charges_reported_usage_and_stops_before_caps() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let limits = BudgetLimits {
        max_tokens: Some(1000),
        max_iterations: Some(2),
        max_cost_usd: Some(0.0009),
        input_cost_per_1m: Some(1.0),
        output_cost_per_1m: Some(2.0),
    };
    let mut budget = Budget::open(dir.path(), limits.clone())?;
    assert_eq!(budget.output_allowance(400, 450), 250);
    assert!(budget.reserve(400, 251).is_err());
    budget.reserve(400, 250)?;
    budget.settle(Some(TokenUsage {
        input: 350,
        output: 150,
    }))?;
    let usage = budget.snapshot();
    assert_eq!(usage.total_tokens, 500);
    assert_eq!(usage.remaining_iterations, Some(1));
    assert!((usage.cost_usd.unwrap() - 0.00065).abs() < 1e-10);
    assert_eq!(budget.output_allowance(200, 100), 25);
    budget.reserve(200, 24)?;
    budget.settle(None)?;
    assert!(budget.snapshot().exhausted);
    assert!(budget.reserve(1, 1).is_err());
    let restored = Budget::open(dir.path(), limits)?;
    assert!(restored.snapshot().exhausted);
    assert_eq!(restored.snapshot().total_tokens, 724);
    assert_eq!(restored.snapshot().estimated_requests, 1);
    let persisted_limits = Budget::saved_limits(dir.path())?.unwrap();
    assert_eq!(persisted_limits.max_iterations, Some(2));
    assert_eq!(persisted_limits.max_tokens, Some(1000));
    assert_eq!(persisted_limits.max_cost_usd, Some(0.0009));
    Ok(())
}

#[test]
fn recovery_charges_the_persisted_reservation_once() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let limits = BudgetLimits {
        max_tokens: Some(120),
        ..BudgetLimits::default()
    };
    let mut original = Budget::open(dir.path(), limits.clone())?;
    original.reserve(100, 20)?;
    drop(original);
    for _ in 0..2 {
        let mut budget = Budget::open(dir.path(), limits.clone())?;
        assert_eq!(budget.snapshot().requests, 1);
        assert_eq!(budget.snapshot().total_tokens, 120);
        assert_eq!(budget.snapshot().estimated_requests, 1);
        assert_eq!(budget.snapshot().cost_usd, None);
        assert!(budget.reserve(1, 1).is_err());
    }
    let mut raised = Budget::open(
        dir.path(),
        BudgetLimits {
            max_tokens: Some(121),
            ..limits
        },
    )?;
    assert_eq!(raised.output_allowance(0, 100), 1);
    raised.reserve(0, 1)?;
    raised.settle(None)?;
    assert!(raised.snapshot().exhausted);
    Ok(())
}

#[test]
fn missing_ledger_is_reported_without_replacing_initialized_save() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    std::fs::create_dir(dir.path().join("agent"))?;
    std::fs::write(dir.path().join("agent/state.json"), "{\"version\":1}")?;
    assert!(Budget::open(dir.path(), BudgetLimits::default()).is_err());
    assert!(!dir.path().join("budget.json").exists());
    Ok(())
}

#[test]
fn requires_prices_for_dollar_caps_and_accounts_for_previously_unpriced_tokens(
) -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let defaults = BudgetLimits::default();
    let cap_without_prices = BudgetLimits {
        max_cost_usd: Some(1.0),
        ..defaults.clone()
    };
    assert!(Budget::open(dir.path(), cap_without_prices).is_err());
    let mut budget = Budget::open(dir.path(), defaults.clone())?;
    budget.reserve(800, 200)?;
    budget.settle(None)?;
    assert_eq!(budget.snapshot().cost_usd, None);
    let with_prices = Budget::open(
        dir.path(),
        BudgetLimits {
            max_cost_usd: Some(0.001),
            input_cost_per_1m: Some(1.0),
            output_cost_per_1m: Some(1.0),
            ..defaults
        },
    )?;
    assert_eq!(with_prices.snapshot().cost_usd, Some(0.001));
    assert!(with_prices.snapshot().exhausted);
    Ok(())
}
