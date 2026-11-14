//! One computational scale, retained in IR for every subsequent derivation.
use super::*;

pub const MAX_LIMIT_SCALE: usize = 1000;
pub fn parse_limit_scale(value: &str) -> std::result::Result<usize, &'static str> {
    if value.is_empty() || !value.bytes().all(|c| c.is_ascii_digit()) {
        return Err("--agency-limit-scale requires an ASCII integer");
    }
    let n = value
        .parse::<usize>()
        .map_err(|_| "--agency-limit-scale integer overflow")?;
    if !(1..=MAX_LIMIT_SCALE).contains(&n) {
        return Err("--agency-limit-scale is outside 1..=1000");
    }
    Ok(n)
}
pub(super) const DEGRADATION_WORK: usize = 1_000_000;

impl Limits {
    pub(super) fn scaled(self, scale: usize) -> Result<Self> {
        if scale == 0 {
            return Err(invalid());
        }
        Ok(Self {
            work: self.work.checked_mul(scale).ok_or_else(invalid)?,
            cells: self.cells.checked_mul(scale).ok_or_else(invalid)?,
            ..self
        })
    }
}

#[derive(Serialize)]
pub(super) struct Budgets {
    pub main: Limits,
    pub observation: Limits,
    pub framing: Limits,
    pub retention: Limits,
    pub effects: Limits,
    pub common_cause: Limits,
}

pub(super) fn budgets_for_model(m: &ir::Model) -> Result<Budgets> {
    let normal = DEFAULT.scaled(m.agency_limit_scale)?;
    Ok(Budgets {
        main: normal,
        observation: normal,
        framing: normal,
        retention: normal,
        effects: normal,
        common_cause: common_cause::LIMITS.scaled(m.agency_limit_scale)?,
    })
}
