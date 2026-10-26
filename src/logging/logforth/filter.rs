use std::collections::HashMap;

use logforth::{Filter, filter::FilterResult, kv, record::Metadata};

use crate::{component_configs::ComponentConfig, utils::unlikely};

use super::{COMPONENT_DIAG_KEY, Config, LogLevel};

#[derive(Debug)]
struct DiagVisitor<'a> {
    max_level: Option<LogLevel>,
    map: &'a HashMap<String, LogLevel>,
    has_component: bool,
}

impl kv::Visitor for DiagVisitor<'_> {
    fn visit(&mut self, key: kv::Key, value: kv::Value) -> Result<(), logforth::Error> {
        if key == kv::Key::new(COMPONENT_DIAG_KEY) {
            self.has_component = true;
            if let Some(component) = value.to_str() {
                self.max_level = self.map.get(&*component).copied();
            }
        }

        Ok(())
    }
}

/// Filters records against [`Config`].
///
/// Precedence, from most to least specific:
/// 1. **Target match** in `cfg.targets` — if the record's target is
///    listed, its level acts as a hard ceiling. Records exceeding the
///    configured level are rejected outright; `LogLevel::Disabled`
///    rejects every record for that target. Component scope cannot
///    relax a target ceiling.
/// 2. **Component scope** carried via the
///    [`COMPONENT_DIAG_KEY`](super::COMPONENT_DIAG_KEY) diagnostic. If a
///    matching entry exists in `cfg.components`, its level is used; if
///    a component is in scope but has no explicit entry,
///    `cfg.default_level` is used.
/// 3. **`cfg.default_level`** for everything else (no target match, no
///    component scope).
///
/// `cfg.enabled = false` short-circuits to reject before any of the above.
#[derive(Debug)]
pub(super) struct LogFilter {
    pub(super) cfg: ComponentConfig<Config>,
}

impl Filter for LogFilter {
    fn enabled(
        &self,
        metadata: &Metadata,
        diags: &[Box<dyn logforth::Diagnostic>],
    ) -> FilterResult {
        if unlikely(!self.cfg.enabled) {
            return FilterResult::Reject;
        }

        // Look up target level once.
        let target_level = self
            .cfg
            .targets
            .get(metadata.target())
            .map(|lvl| lvl.into_logforth());

        // Early reject if target is explicitly configured and level exceeds it (or is disabled).
        if let Some(max_level) = target_level {
            match max_level {
                Some(max) if metadata.level() > max => return FilterResult::Reject,
                None => return FilterResult::Reject,
                _ => {}
            }
        }

        // Check diagnostics for component scope.
        let mut visitor = DiagVisitor {
            max_level: None,
            has_component: false,
            map: &self.cfg.components,
        };
        for diag in diags {
            _ = diag.visit(&mut visitor);
            if visitor.has_component {
                break;
            }
        }

        // Determine effective max level.
        let max_level = if let Some(component_level) = visitor.max_level {
            // Component found with explicit level configured.
            component_level.into_logforth()
        } else if visitor.has_component {
            // Component found, but no explicit level — use default.
            self.cfg.default_level.into_logforth()
        } else if let Some(target) = target_level {
            // Target was matched and we already passed the early-reject
            // check, so this is the effective ceiling.
            target
        } else {
            // No target match, no component scope — fall back to default.
            self.cfg.default_level.into_logforth()
        };

        if max_level.is_some_and(|lvl| metadata.level() <= lvl) {
            FilterResult::Accept
        } else {
            FilterResult::Reject
        }
    }
}
