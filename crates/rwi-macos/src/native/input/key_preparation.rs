//! Invocation-local explicit keyboard preparation and guarded paired dispatch.
use super::*;
pub(super) struct KeyInput {
    payload: rwi_core::KeyChordPayload,
    record: Option<(
        crate::background::key_window_record::Prepared,
        rwi_core::ForegroundState,
        (CGEvent, CGEvent),
    )>,
}
impl NativeMac {
    pub(super) fn prepare_key_input(
        &self,
        intent: &Intent,
        value: Option<&str>,
    ) -> Result<Option<KeyInput>> {
        let key_payload = if matches!(intent, Intent::KeyChord) {
            Some(rwi_core::KeyChordPayload::parse(value.ok_or_else(
                || fault("value_required", "Key chord value required."),
            )?)?)
        } else {
            None
        };
        // All exact-owner lookup and event allocation precede any receiver mutation.
        let key_record = if key_payload
            .as_ref()
            .is_some_and(|p| p.preparation.is_some())
        {
            let before = activation::foreground_state()?;
            before.validate()?;
            if before.pid == self.pid {
                return Err(fault(
                    "key_record_background_target_required",
                    "Experimental preparation requires a different exact foreground process; no sibling or foreground fallback.",
                ));
            }
            let prepared = {
                let _call = operation::native_call("macos_key_record_owner_prepare_readonly")?;
                crate::background::key_window_record::prepare(self.pid, self.window)?
            };
            let p = key_payload.as_ref().expect("checked key payload");
            let (key, flags) = parse_chord(&p.key)?;
            Some((prepared, before, key_events(key, flags, None)?))
        } else {
            None
        };
        Ok(key_payload.map(|payload| KeyInput {
            payload,
            record: key_record,
        }))
    }
    pub(super) fn dispatch_key_input(
        &mut self,
        expected: &Binding,
        target: &Resolved,
        intent: &Intent,
        sequence: &mut Sequence<AxObject>,
        delivery: &crate::background::Delivery,
        prepared: KeyInput,
    ) -> Result<()> {
        let counter = self.dispatches.clone();
        let key_payload = Some(prepared.payload);
        let key_record = prepared.record;
        let (down, up) = if let Some((prepared, foreground, events)) = key_record {
            let mut statuses = Vec::with_capacity(2);
            sequence.pair(
                expected,
                target,
                &counter,
                prepared.events(),
                |first| self.read_input_batch(first, expected, target, intent),
                |event| statuses.push(prepared.post(event)),
            )?;
            if statuses.len() != 2 || statuses.iter().any(|s| *s != 0) {
                let mut error = fault(
                    "key_record_preparation_unconfirmed",
                    "Private record status is unknown/nonzero; no key or fallback, query original request and observe independently.",
                );
                error.diagnostic = Some(Box::new(
                    serde_json::json!({"native_statuses":statuses,"record_layout":"legacy_248_unattested","replay_allowed":false}),
                ));
                return Err(error);
            }
            if activation::foreground_state()? != foreground {
                return Err(fault(
                    "key_record_foreground_changed",
                    "OS foreground changed during experimental preparation; no key, no restoration or replay.",
                ));
            }
            events
        } else {
            let p = key_payload.as_ref().expect("validated key chord");
            let (key, flags) = parse_chord(&p.key)?;
            key_events(key, flags, None)?
        };
        sequence.pair(
            expected,
            target,
            &counter,
            (down, up),
            |first| self.read_input_batch(first, expected, target, intent),
            |e| crate::background::deliver_key(e, delivery),
        )?;
        Ok(())
    }
}
