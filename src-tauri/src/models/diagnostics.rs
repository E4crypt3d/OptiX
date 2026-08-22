//! Diagnostics models (Phase 12): a single rule-based finding with a
//! confidence score and an actionable recommendation.

use serde::Serialize;

/// A ranked, evidence-backed diagnostic finding. Rule-based only — nothing is
/// changed automatically; each finding is advisory with a confidence score.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    /// Stable rule identifier (e.g. `memory_pressure`).
    pub id: String,
    /// info | warning | critical
    pub severity: String,
    /// cpu | gpu | memory | storage | background | update | driver | thermal
    pub category: String,
    pub title: String,
    /// The specific evidence behind the finding (never a black box).
    pub detail: String,
    pub recommendation: String,
    /// 0–100 confidence.
    pub confidence: u8,
}

/// Full result of a diagnostics run: ranked findings plus an overall health
/// score derived from them. Still purely advisory — nothing is applied.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsReport {
    pub findings: Vec<Diagnostic>,
    /// 0–100 system health score (100 = no findings).
    pub score: u8,
    /// Short verdict matching the score band.
    pub verdict: String,
    /// How many rules were evaluated, so "no issues" is verifiable.
    pub checks_run: u32,
}
