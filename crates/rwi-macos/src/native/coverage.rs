//! Observation coverage is evidence, never input authority or proof of absence.
#[derive(Default)]
pub(super) struct Coverage {
    pub visited: usize,
    pub described: usize,
    pub maximum_depth_seen: usize,
    pub unexpanded_depth_nodes: usize,
    pub dropped_queue_nodes: usize,
    pub child_reads_failed: usize,
    pub descriptions_failed: usize,
    pub node_budget_reached: bool,
    pub time_budget_reached: bool,
}

impl Coverage {
    pub fn evidence(&self, remaining: usize) -> serde_json::Value {
        let mut reasons = Vec::new();
        for (present, reason) in [
            (self.unexpanded_depth_nodes > 0, "depth_budget"),
            (self.dropped_queue_nodes > 0, "queue_budget"),
            (self.node_budget_reached, "node_budget"),
            (self.time_budget_reached, "time_budget"),
            (self.child_reads_failed > 0, "children_read_unavailable"),
            (self.descriptions_failed > 0, "element_read_unavailable"),
            (remaining > 0, "pending_nodes"),
        ] {
            if present {
                reasons.push(reason);
            }
        }
        serde_json::json!({
            "scope":"selected_window_AXChildren_at_read_time",
            "complete":reasons.is_empty(), "reasons":reasons,
            "visited_nodes":self.visited, "described_nodes":self.described,
            "maximum_depth_seen":self.maximum_depth_seen,
            "unexpanded_depth_nodes":self.unexpanded_depth_nodes,
            "dropped_queue_nodes":self.dropped_queue_nodes, "pending_nodes":remaining,
            "child_reads_failed":self.child_reads_failed,
            "descriptions_failed":self.descriptions_failed,
            "absence_proven":false, "input_authority":false,
            "limitations":"no continuous tree/hidden-content attestation; depth-boundary children not read"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_does_not_silently_convert_limits_or_native_failures_to_absence() {
        let complete = Coverage::default().evidence(0);
        assert_eq!(complete["complete"], true);
        assert_eq!(complete["absence_proven"], false);
        assert_eq!(complete["input_authority"], false);
        for report in [
            Coverage {
                unexpanded_depth_nodes: 1,
                ..Coverage::default()
            },
            Coverage {
                dropped_queue_nodes: 1,
                ..Coverage::default()
            },
            Coverage {
                node_budget_reached: true,
                ..Coverage::default()
            },
            Coverage {
                time_budget_reached: true,
                ..Coverage::default()
            },
            Coverage {
                child_reads_failed: 1,
                ..Coverage::default()
            },
            Coverage {
                descriptions_failed: 1,
                ..Coverage::default()
            },
        ] {
            let evidence = report.evidence(0);
            assert_eq!(evidence["complete"], false);
            assert_eq!(evidence["reasons"].as_array().unwrap().len(), 1);
            assert_eq!(evidence["input_authority"], false);
        }
        assert_eq!(Coverage::default().evidence(1)["complete"], false);
    }
}
