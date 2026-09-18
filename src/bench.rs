use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::decider::model_slug;
use crate::run::Report;

pub struct Summary {
    pub model: String,
    pub runs: usize,
    pub passes: usize,
    pub mean_decide_ms: f64,
    pub p50_decide_ms: u128,
    pub p95_decide_ms: u128,
    pub mean_total_ms: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost: f64,
}

impl Summary {
    pub fn of(model: &str, reports: &[Report]) -> Self {
        let spends: Vec<_> = reports
            .iter()
            .flat_map(|report| report.steps.iter().filter_map(|step| step.spend))
            .collect();
        let mut decide_ms: Vec<u128> = spends.iter().map(|spend| spend.decide_ms).collect();
        decide_ms.sort_unstable();

        Self {
            model: model.to_string(),
            runs: reports.len(),
            passes: reports.iter().filter(|report| report.passed()).count(),
            mean_decide_ms: mean(decide_ms.iter().copied()),
            p50_decide_ms: percentile(&decide_ms, 0.50),
            p95_decide_ms: percentile(&decide_ms, 0.95),
            mean_total_ms: mean(reports.iter().map(|report| report.total_ms)),
            input_tokens: spends.iter().map(|spend| spend.input_tokens).sum(),
            output_tokens: spends.iter().map(|spend| spend.output_tokens).sum(),
            cost: spends.iter().filter_map(|spend| spend.cost).sum(),
        }
    }

    pub fn header() -> String {
        "| model | runs | passes | mean decide ms | p50 | p95 | mean total ms | input tokens | output tokens | cost USD |\n|---|---|---|---|---|---|---|---|---|---|".to_string()
    }

    pub fn row(&self) -> String {
        format!(
            "| {} | {} | {} | {:.0} | {} | {} | {:.0} | {} | {} | {:.4} |",
            self.model,
            self.runs,
            self.passes,
            self.mean_decide_ms,
            self.p50_decide_ms,
            self.p95_decide_ms,
            self.mean_total_ms,
            self.input_tokens,
            self.output_tokens,
            self.cost
        )
    }
}

pub fn write_report(root: &Path, model: &str, stamp: u64, run: usize, report: &Report) -> Result<PathBuf> {
    let directory = root.join(model_slug(model));
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("{} could not be created", directory.display()))?;
    let path = directory.join(format!("{stamp}-{run}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(report)?)
        .with_context(|| format!("{} could not be written", path.display()))?;
    Ok(path)
}

pub fn stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0)
}

fn mean(values: impl Iterator<Item = u128>) -> f64 {
    let (total, count) = values.fold((0u128, 0usize), |(total, count), value| {
        (total + value, count + 1)
    });
    if count == 0 {
        return 0.0;
    }
    total as f64 / count as f64
}

fn percentile(sorted: &[u128], fraction: f64) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (fraction * (sorted.len() - 1) as f64).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}
