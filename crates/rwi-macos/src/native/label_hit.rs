//! A noninteractive text label may receive the hit for its original AXButton.
//! Exact bounded ancestry, never sibling geometry or a replacement control.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Role {
    Button,
    StaticText,
    Group,
    Other,
}
struct Facts {
    role: Role,
    same_provider: bool,
    same_window: bool,
    contains_point: bool,
    noninteractive: bool,
}
struct Node(Cf);
impl Clone for Node {
    fn clone(&self) -> Self {
        Self(self.0.retained())
    }
}
impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        unsafe { ffi::CFEqual(self.0.0, other.0.0) != 0 }
    }
}
impl Facts {
    fn scoped(&self) -> bool {
        self.same_provider && self.same_window && self.contains_point
    }
}

fn label_ancestor<K: Clone + PartialEq>(
    original: &K,
    hit: K,
    mut inspect: impl FnMut(&K) -> Result<Facts>,
    mut parent: impl FnMut(&K) -> Result<K>,
    mut checkpoint: impl FnMut() -> Result<()>,
) -> Result<bool> {
    checkpoint()?;
    let target = inspect(original)?;
    if target.role != Role::Button || !target.scoped() {
        return Ok(false);
    }
    let mut seen = Vec::with_capacity(8);
    let mut current = hit;
    for depth in 0..8 {
        checkpoint()?;
        if seen.contains(&current) {
            return Ok(false);
        }
        let facts = inspect(&current)?;
        if !facts.scoped() {
            return Ok(false);
        }
        if &current == original {
            return Ok(depth > 0 && facts.role == Role::Button);
        }
        if !facts.noninteractive
            || (depth == 0 && facts.role != Role::StaticText)
            || !matches!(facts.role, Role::StaticText | Role::Group)
        {
            return Ok(false);
        }
        seen.push(current.clone());
        current = parent(&current)?;
    }
    Ok(false)
}

fn passive_actions(element: &Cf) -> Result<bool> {
    let mut out = std::ptr::null();
    let code = unsafe { ffi::AXUIElementCopyActionNames(element.0, &mut out) };
    if code != 0 || out.is_null() {
        return Err(fault(
            "label_actions_unknown",
            "Cannot prove a label has no interactive native action; no equivalence.",
        ));
    }
    let array = unsafe { Cf::owned(out)? };
    let values = unsafe { ffi::array(array.0, 12)? };
    values
        .iter()
        .map(Cf::string)
        .try_fold(true, |passive, action| {
            Ok(passive && matches!(action?.as_str(), "AXShowMenu" | "AXScrollToVisible"))
        })
}

fn read_checkpoint(started: Instant, budget: Duration) -> Result<()> {
    operation::checkpoint("macos_button_label_read")?;
    if started.elapsed() >= budget {
        return Err(fault(
            "label_ancestry_deadline",
            "Bounded label ancestry read expired; no equivalence.",
        ));
    }
    Ok(())
}
fn bounded_read<T>(
    node: &Cf,
    started: Instant,
    budget: Duration,
    read: impl FnOnce(&Cf) -> Result<T>,
) -> Result<T> {
    read_checkpoint(started, budget)?;
    let timeout = budget
        .saturating_sub(started.elapsed())
        .min(Duration::from_millis(20));
    if timeout.is_zero() {
        return Err(fault(
            "label_ancestry_deadline",
            "No label read budget remains.",
        ));
    }
    let _call = operation::native_call("macos_button_label_read")?;
    let code = unsafe { ffi::AXUIElementSetMessagingTimeout(node.0, timeout.as_secs_f32()) };
    if code != 0 {
        return Err(fault(
            "label_timeout_configuration",
            "Cannot bound the exact label proxy; no equivalence.",
        ));
    }
    let result = read(node);
    // Each RWI proxy uses the regular 50 ms probe policy after this short read.
    // AX application timeout does not propagate to these per-object proxies.
    let restored = unsafe { ffi::AXUIElementSetMessagingTimeout(node.0, 0.05) };
    let value = match result {
        Ok(value) => value,
        Err(mut error) => {
            if restored != 0 {
                let mut diagnostic = error
                    .diagnostic
                    .take()
                    .map(|value| *value)
                    .unwrap_or_else(|| serde_json::json!({}));
                diagnostic["label_timeout_restore_code"] = serde_json::json!(restored);
                error.diagnostic = Some(Box::new(diagnostic));
            }
            return Err(error);
        }
    };
    if restored != 0 {
        return Err(fault(
            "label_timeout_restore",
            "Cannot restore the exact label proxy probe budget; no equivalence.",
        ));
    }
    read_checkpoint(started, budget)?;
    Ok(value)
}

pub(super) fn equivalent(
    original: &Cf,
    hit: &Cf,
    window: &Cf,
    point: [f64; 2],
    original_role: Option<&str>,
    budget: Duration,
) -> Result<bool> {
    if unsafe { ffi::CFEqual(original.0, hit.0) != 0 } {
        return Ok(true);
    }
    if original_role != Some("AXButton") {
        return Ok(false);
    }
    let started = Instant::now();
    let budget = budget.min(Duration::from_millis(40));
    if bounded_read(hit, started, budget, |node| node.attr("AXRole")?.string())? != "AXStaticText" {
        return Ok(false);
    }
    let owner = bounded_read(original, started, budget, Cf::pid)?;
    let epoch = process_epoch(owner)?;
    let checkpoint = || {
        read_checkpoint(started, budget)?;
        if process_epoch(owner)? != epoch {
            return Err(fault(
                "label_provider_changed",
                "The original label provider changed; no equivalence.",
            ));
        }
        Ok(())
    };
    label_ancestor(
        &Node(original.retained()),
        Node(hit.retained()),
        |node| {
            checkpoint()?;
            let node = &node.0;
            let role =
                match bounded_read(node, started, budget, |node| node.attr("AXRole")?.string())?
                    .as_str()
                {
                    "AXButton" => Role::Button,
                    "AXStaticText" => Role::StaticText,
                    "AXGroup" => Role::Group,
                    _ => Role::Other,
                };
            let container = bounded_read(node, started, budget, |node| node.attr("AXWindow"))?;
            let same_window = unsafe { ffi::CFEqual(container.0, window.0) != 0 };
            let position = bounded_read(node, started, budget, |node| {
                node.attr("AXPosition")?.point()
            })?;
            let size = bounded_read(node, started, budget, |node| node.attr("AXSize")?.size())?;
            let bounds = Rect {
                x: position.x,
                y: position.y,
                w: size.width,
                h: size.height,
            };
            let facts = Facts {
                role,
                same_provider: bounded_read(node, started, budget, Cf::pid)? == owner,
                same_window,
                contains_point: bounds.valid() && bounds.contains(point),
                noninteractive: if role == Role::Button {
                    false
                } else {
                    bounded_read(node, started, budget, passive_actions)?
                },
            };
            checkpoint()?;
            Ok(facts)
        },
        |node| {
            Ok(Node(bounded_read(&node.0, started, budget, |node| {
                node.attr("AXParent")
            })?))
        },
        checkpoint,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn facts(role: Role) -> Facts {
        Facts {
            role,
            same_provider: true,
            same_window: true,
            contains_point: true,
            noninteractive: role != Role::Button,
        }
    }
    fn graph(hit: u8, parents: &[(u8, u8)], roles: &[(u8, Role)]) -> bool {
        label_ancestor(
            &1,
            hit,
            |id| {
                Ok(facts(
                    roles
                        .iter()
                        .find(|(n, _)| n == id)
                        .map(|(_, r)| *r)
                        .unwrap_or(Role::Other),
                ))
            },
            |id| {
                parents
                    .iter()
                    .find(|(n, _)| n == id)
                    .map(|(_, p)| *p)
                    .ok_or_else(|| fault("parent_unknown", "unknown"))
            },
            || Ok(()),
        )
        .unwrap_or(false)
    }
    #[test]
    fn exact_button_label_and_passive_group_ancestor_are_proved() {
        assert!(graph(
            2,
            &[(2, 1)],
            &[(1, Role::Button), (2, Role::StaticText)]
        ));
        assert!(graph(
            3,
            &[(3, 2), (2, 1)],
            &[(1, Role::Button), (2, Role::Group), (3, Role::StaticText)]
        ));
    }
    #[test]
    fn wrong_button_sibling_interactive_child_cycle_and_depth_are_refused() {
        assert!(!graph(
            3,
            &[(3, 2), (2, 1)],
            &[(1, Role::Button), (2, Role::Button), (3, Role::StaticText)]
        ));
        assert!(!graph(
            3,
            &[(3, 2)],
            &[(1, Role::Button), (2, Role::Button), (3, Role::StaticText)]
        ));
        assert!(!graph(
            2,
            &[(2, 1)],
            &[(1, Role::Button), (2, Role::Button)]
        ));
        assert!(!graph(
            3,
            &[(3, 2), (2, 3)],
            &[(1, Role::Button), (2, Role::Group), (3, Role::StaticText)]
        ));
        let parents: Vec<_> = (2..=10).map(|n| (n, n - 1)).collect();
        let mut roles: Vec<_> = (2..10).map(|n| (n, Role::Group)).collect();
        roles.extend([(1, Role::Button), (10, Role::StaticText)]);
        assert!(!graph(10, &parents, &roles));
    }
    #[test]
    fn every_chain_node_requires_provider_window_geometry_and_passive_actions() {
        for bad in 0..4 {
            let result = label_ancestor(
                &1,
                2,
                |id| {
                    let mut f = facts(if *id == 1 {
                        Role::Button
                    } else {
                        Role::StaticText
                    });
                    if *id == 2 {
                        match bad {
                            0 => f.same_provider = false,
                            1 => f.same_window = false,
                            2 => f.contains_point = false,
                            _ => f.noninteractive = false,
                        }
                    }
                    Ok(f)
                },
                |_| Ok(1),
                || Ok(()),
            )
            .unwrap();
            assert!(!result);
        }
    }
    #[test]
    fn missing_parent_and_cancel_fail_closed() {
        let error = label_ancestor(
            &1,
            2,
            |id| {
                Ok(facts(if *id == 1 {
                    Role::Button
                } else {
                    Role::StaticText
                }))
            },
            |_| Err(fault("parent_unknown", "unknown")),
            || Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.code, "parent_unknown");
        let error = label_ancestor(
            &1,
            2,
            |id| {
                Ok(facts(if *id == 1 {
                    Role::Button
                } else {
                    Role::StaticText
                }))
            },
            |_| panic!("cancel must not read parent"),
            || Err(fault("operation_cancelled", "cancel")),
        )
        .unwrap_err();
        assert_eq!(error.code, "operation_cancelled");
    }
}
