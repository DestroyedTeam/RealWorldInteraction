//! Resolve only local bootstrap placeholders and validate the real lifecycle
//! contract before installation or worker startup. No OS side effects.
use crate::{invalid, types::Approval};
use serde_json::Value;
use std::io;

pub fn bootstrap(approval: &Approval, root: &str) -> io::Result<()> {
    fn resolve(value: &Value, approval: &Approval, root: &str, depth: usize) -> io::Result<Value> {
        if depth > 24 {
            return Err(invalid("bootstrap_template_depth"));
        }
        match value {
            Value::Object(fields)
                if fields.len() == 1 && fields.keys().next().unwrap().starts_with('$') =>
            {
                let (key, arg) = fields.iter().next().unwrap();
                match key.as_str() {
                    "$file" => {
                        let name = arg.as_str().ok_or_else(|| invalid("bootstrap_file_name"))?;
                        if !approval.files.iter().any(|f| f.name == name) {
                            return Err(invalid("bootstrap_file_not_approved"));
                        }
                        Ok(Value::String(format!(
                            "{root}\\state\\runs\\{}\\{name}",
                            approval.task
                        )))
                    }
                    "$guest_claim" if arg == true => {
                        serde_json::to_value(&approval.guest_claim).map_err(io::Error::other)
                    }
                    "$native_identity" if arg == true => serde_json::to_value(
                        &approval
                            .native_binding
                            .as_ref()
                            .ok_or_else(|| invalid("bootstrap_native_binding"))?
                            .native,
                    )
                    .map_err(io::Error::other),
                    // Structural validation emits no grant. Expired approvals
                    // remain inspectable; runtime/import keep actual clock checks.
                    "$expires" if arg == true => Ok(Value::from(
                        approval
                            .budget
                            .issued_unix_ms
                            .saturating_add(60_000)
                            .min(approval.budget.expires_unix_ms),
                    )),
                    _ => Err(invalid("bootstrap_requires_preexisting_grounded_context")),
                }
            }
            Value::Object(fields) => fields
                .iter()
                .map(|(key, value)| Ok((key.clone(), resolve(value, approval, root, depth + 1)?)))
                .collect::<io::Result<serde_json::Map<_, _>>>()
                .map(Value::Object),
            Value::Array(values) => values
                .iter()
                .map(|value| resolve(value, approval, root, depth + 1))
                .collect::<io::Result<Vec<_>>>()
                .map(Value::Array),
            _ => Ok(value.clone()),
        }
    }
    let template = approval
        .lifecycle_grant_template
        .get("bootstrap")
        .ok_or_else(|| invalid("bootstrap_lifecycle_template_missing"))?;
    let grant: rcu_core::lifecycle::LifecycleGrant =
        serde_json::from_value(resolve(template, approval, root, 0)?)?;
    let identity = &approval
        .native_binding
        .as_ref()
        .ok_or_else(|| invalid("bootstrap_native_binding"))?
        .native;
    grant
        .validate(identity, approval.budget.issued_unix_ms)
        .map_err(|e| invalid(&format!("bootstrap_lifecycle_contract: {e}")))
}
