// src/snb/snb_algorithm.rs
// _light_platform (Kumulativ)

#[derive(Debug, Clone)]
pub struct BugEvent {
    pub module_name: String,
}

#[derive(Debug, Clone)]
pub struct ShadowBugReport {
    pub module_name: String,
}

pub struct SnbAlgorithm {
    events: Vec<BugEvent>,
    has_bug: bool,
}

impl SnbAlgorithm {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            has_bug: false,
        }
    }

    // The POC caller records a local readiness probe; this is not Shadow transport.
    pub fn report_bug(&mut self, module_name: &str) {
        let event = BugEvent {
            module_name: module_name.to_string(),
        };

        self.events.push(event);
        self.has_bug = true;
    }

    pub fn prepare_for_shadow_snb(&self) -> Option<ShadowBugReport> {
        match self.has_bug && !self.events.is_empty() {
            false => return None,
            true => {}
        }
        let last = &self.events[self.events.len() - 1];
        Some(ShadowBugReport {
            module_name: last.module_name.clone(),
        })
    }
}

impl Default for SnbAlgorithm {
    fn default() -> Self {
        Self::new()
    }
}
