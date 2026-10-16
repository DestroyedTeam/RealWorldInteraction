//! Invocation-local readonly value witness; it never sends, retries or authorizes input.
use super::*;
use rwi_core::text_effect::{ExpectedEdit, State, TextEffect, UnknownReason as Reason};

pub(super) struct Attempt(std::result::Result<Witness, Reason>);
struct Witness {
    facts: Facts<AxObject>,
    expected: ExpectedEdit,
}

/// Soft budget: never start another read after expiry. A synchronous Native
/// call already in progress still has the existing AX timeout, not preemption.
struct Budget(Instant);
impl Budget {
    fn new(ms: u64) -> Self {
        Self(Instant::now() + Duration::from_millis(ms))
    }
    fn check(&self) -> std::result::Result<(), Reason> {
        if Instant::now() >= self.0 {
            return Err(Reason::ReadbackBudgetExpired);
        }
        operation::checkpoint("macos_text_effect_query").map_err(|_| Reason::ReadbackUnavailable)
    }
    fn query<T>(&self, read: impl FnOnce() -> Result<T>) -> std::result::Result<T, Reason> {
        self.check()?;
        let value = read().map_err(|e| {
            if e.code == "ax_value_budget" {
                Reason::ValueTooLarge
            } else {
                Reason::ReadbackUnavailable
            }
        })?;
        self.check()?;
        Ok(value)
    }
}

fn plain_value_with(
    mut read: impl FnMut(&str) -> std::result::Result<String, Reason>,
) -> std::result::Result<String, Reason> {
    let classify = |read: &mut dyn FnMut(&str) -> std::result::Result<String, Reason>| {
        let role = read("AXRole")?;
        let subrole = read("AXSubrole").map_err(|e| {
            if e == Reason::ReadbackUnavailable {
                Reason::NotPlainField
            } else {
                e
            }
        })?;
        if TextClass::classify(&role, Some(&subrole)) == TextClass::Plain {
            Ok(())
        } else {
            Err(Reason::NotPlainField)
        }
    };
    classify(&mut read)?;
    let value = read("AXValue")?;
    classify(&mut read)?;
    Ok(value)
}

fn attribute_string(name: &str, attribute: Result<Cf>) -> Result<String> {
    match attribute {
        Ok(value) => value.string(),
        // Match element_description's nullable-subrole semantics. Only explicit
        // AX attribute absence means no specialization; timeout, wrong type,
        // invalid object and AXUnknown must remain unknown, never plain.
        Err(error) if name == "AXSubrole" && ffi::attribute_is_absent(&error, name) => {
            Ok(String::new())
        }
        Err(error) => Err(error),
    }
}

fn value(element: &Cf, budget: &Budget) -> std::result::Result<String, Reason> {
    plain_value_with(|name| budget.query(|| attribute_string(name, element.attr(name))))
}

#[repr(C)]
#[derive(Default)]
struct NativeRange {
    location: isize,
    length: isize,
}
fn range(value: &Cf) -> std::result::Result<(u64, u64), Reason> {
    let mut out = NativeRange::default();
    if unsafe {
        ffi::CFGetTypeID(value.0) != ffi::AXValueGetTypeID()
            || ffi::AXValueGetType(value.0) != 4
            || ffi::AXValueGetValue(value.0, 4, (&mut out as *mut NativeRange).cast()) == 0
    } {
        return Err(Reason::SelectionUnknown);
    }
    Ok((
        u64::try_from(out.location).map_err(|_| Reason::InvalidUtf16Range)?,
        u64::try_from(out.length).map_err(|_| Reason::InvalidUtf16Range)?,
    ))
}
fn selection(element: &Cf, budget: &Budget) -> std::result::Result<(u64, u64), Reason> {
    range(
        &budget
            .query(|| element.attr("AXSelectedTextRange"))
            .map_err(|e| {
                if e == Reason::ReadbackUnavailable {
                    Reason::SelectionUnknown
                } else {
                    e
                }
            })?,
    )
}

fn custody(
    native: &NativeMac,
    facts: &Facts<AxObject>,
    budget: &Budget,
) -> std::result::Result<(), Reason> {
    budget.check()?;
    let same = unsafe { ffi::AXIsProcessTrusted() != 0 }
        && facts.identity == native.identity
        && facts.process == native.process
        && budget.query(console_identity)? == facts.identity
        && budget.query(|| process_epoch(native.pid))? == facts.process
        && budget
            .query(|| native.window_record())?
            .rect
            .same(facts.window_rect)
        && budget.query(|| facts.window.native().pid())? == native.pid
        && budget.query(|| facts.hit.native().pid())? == native.pid
        && budget.query(|| Ok(ffi::window_number_for_ax_element(facts.window.native())))?
            == Some(native.window)
        && unsafe {
            ffi::CFEqual(
                budget.query(|| facts.hit.native().attr("AXWindow"))?.0,
                facts.window.native().0,
            ) != 0
        };
    if same {
        Ok(())
    } else {
        Err(Reason::NativeIdentityChanged)
    }
}

impl Witness {
    fn prepare(
        native: &NativeMac,
        facts: Facts<AxObject>,
        text: &str,
    ) -> std::result::Result<Self, Reason> {
        let _call = operation::native_call("macos_text_effect_prepare")
            .map_err(|_| Reason::ReadbackUnavailable)?;
        let budget = Budget::new(50);
        custody(native, &facts, &budget)?;
        let before = value(facts.hit.native(), &budget)?;
        let selected = selection(facts.hit.native(), &budget)?;
        // A bounded bracket detects observed selection/value drift, not atomicity.
        if before != value(facts.hit.native(), &budget)?
            || selected != selection(facts.hit.native(), &budget)?
        {
            return Err(Reason::ReadbackUnavailable);
        }
        custody(native, &facts, &budget)?;
        let expected = ExpectedEdit::new(&before, Some(selected), text)?;
        Ok(Self { facts, expected })
    }

    fn sample(&self, native: &NativeMac, budget: &Budget) -> TextEffect {
        let actual = (|| {
            let _call = operation::native_call("macos_text_effect_readback")
                .map_err(|_| Reason::ReadbackUnavailable)?;
            custody(native, &self.facts, budget)?;
            let value = value(self.facts.hit.native(), budget)?;
            custody(native, &self.facts, budget)?;
            Ok(value)
        })();
        self.expected.sample(actual.as_deref().map_err(|e| *e))
    }

    fn finish(self, native: &NativeMac) -> TextEffect {
        let budget = Budget::new(75);
        let first = self.sample(native, &budget);
        if first.state != State::ValueDiffersObserved
            || budget.0.saturating_duration_since(Instant::now()) <= Duration::from_millis(25)
        {
            return first;
        }
        // Only the readonly sample may settle once. Input is never repeated.
        if operation::checkpoint("macos_text_effect_settle").is_err() {
            return self.expected.sample(Err(Reason::ReadbackUnavailable));
        }
        std::thread::sleep(Duration::from_millis(25));
        self.sample(native, &budget)
    }
}

impl NativeMac {
    pub(super) fn prepare_text_witness(
        &mut self,
        expected: &Binding,
        target: &Resolved,
        intent: &Intent,
        value: Option<&str>,
    ) -> Result<Option<Attempt>> {
        if !matches!(intent, Intent::Text | Intent::KeyChord) {
            return Ok(None);
        }
        validate_keyboard_payload(intent, value)?;
        let facts = self.read_input_batch(true, expected, target, intent)?;
        facts.validate(expected, target, Kind::of(intent))?;
        let witness = matches!(intent, Intent::Text)
            .then(|| Attempt(Witness::prepare(self, facts, value.unwrap_or_default())));
        // Optional observation may fail, but may not swallow cancellation or an
        // expired input deadline before the existing receiver preparation.
        operation::checkpoint("macos_after_text_effect_prepare")?;
        Ok(witness)
    }

    pub(super) fn record_text_effect(&mut self, attempt: Option<Attempt>, complete: bool) {
        self.text_effect = attempt.map(|attempt| {
            if !complete {
                return TextEffect::unknown(Reason::DispatchIncomplete);
            }
            match attempt.0 {
                Ok(witness) => witness.finish(self),
                Err(reason) => TextEffect::unknown(reason),
            }
        });
    }
}

#[cfg(test)]
mod tests;
