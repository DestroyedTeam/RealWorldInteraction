//! Own field-editor source calibration. No TIS select/enable or input posting.
use objc2::{msg_send, rc::Retained, runtime::AnyObject};
use objc2_foundation::NSString;
use serde::Serialize;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Serialize)]
struct Observation {
    requested: String,
    phase: &'static str,
    original: Option<String>,
    current: Option<String>,
    current_client_matches: Option<bool>,
    source_catalog: Option<crate::ime_source::CatalogSample>,
    source_catalog_sampled_unix_ms: Option<u64>,
    source_catalog_client_matches: Option<bool>,
    set_calls: u32,
    restore_calls: u32,
    sampled_unix_ms: u64,
    application_active: bool,
    window_key: bool,
    context_properties_atomic: bool,
    global_source_enable_or_select_called: bool,
    input_posted: bool,
}
struct Control {
    observation: Observation,
    path: PathBuf,
    context: Option<Retained<AnyObject>>,
}
thread_local! { static CONTROL: RefCell<Option<Control>> = const { RefCell::new(None) }; }
struct Guard<'a> {
    slot: &'a RefCell<Option<Control>>,
    value: Option<Control>,
}
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        *self.slot.borrow_mut() = self.value.take();
    }
}
fn access(work: impl FnOnce(&mut Control) -> Result<(), String>) -> Result<(), String> {
    CONTROL.with(|slot| {
        let Some(value) = slot.borrow_mut().take() else {
            return Ok(());
        };
        let mut guard = Guard {
            slot,
            value: Some(value),
        };
        work(guard.value.as_mut().unwrap())
    })
}
pub fn configure(id: Option<String>, directory: &Path) -> Result<(), String> {
    let Some(requested) = id else {
        return Ok(());
    };
    if !crate::ime_source::valid_id(&requested) {
        return Err("invalid source ID".into());
    }
    CONTROL.with(|slot| {
        if slot.borrow().is_some() {
            return Err("source calibration already configured".into());
        }
        *slot.borrow_mut() = Some(Control {
            path: directory.join("input-source-control.json"),
            context: None,
            observation: Observation {
                requested,
                phase: "waiting_for_own_active_editor",
                original: None,
                current: None,
                current_client_matches: None,
                source_catalog: None,
                source_catalog_sampled_unix_ms: None,
                source_catalog_client_matches: None,
                set_calls: 0,
                restore_calls: 0,
                sampled_unix_ms: 0,
                application_active: false,
                window_key: false,
                context_properties_atomic: false,
                global_source_enable_or_select_called: false,
                input_posted: false,
            },
        });
        Ok(())
    })
}
pub fn configured() -> bool {
    CONTROL.with(|s| s.borrow().is_some())
}
unsafe fn source(context: &AnyObject) -> Option<String> {
    let value: Option<Retained<NSString>> =
        unsafe { msg_send![context, selectedKeyboardInputSource] };
    let value = value?;
    if value.length() > 256 {
        return None;
    }
    let text = value.to_string();
    crate::ime_source::valid_id(&text).then_some(text)
}
unsafe fn owns(context: &AnyObject, editor: &AnyObject) -> bool {
    let client: *mut AnyObject = unsafe { msg_send![context, client] };
    std::ptr::eq(client.cast_const(), editor)
}
unsafe fn catalog(context: &AnyObject, requested: &str) -> crate::ime_source::CatalogSample {
    let ids: Option<Retained<AnyObject>> = unsafe { msg_send![context, keyboardInputSources] };
    let Some(ids) = ids else {
        return crate::ime_source::catalog_sample(None, &[], requested);
    };
    let count: usize = unsafe { msg_send![&*ids, count] };
    let class = objc2::runtime::AnyClass::get(c"NSString");
    let mut rows = Vec::with_capacity(count.min(crate::ime_source::CATALOG_LIMIT));
    for i in 0..count.min(crate::ime_source::CATALOG_LIMIT) {
        let value: *mut AnyObject = unsafe { msg_send![&*ids,objectAtIndex:i] };
        let value = if let (Some(class), false) = (class, value.is_null()) {
            let correct: bool = unsafe { msg_send![value,isKindOfClass:class] };
            if correct {
                unsafe { Retained::<NSString>::retain(value.cast()) }
            } else {
                None
            }
        } else {
            None
        };
        rows.push(value.filter(|s| s.length() <= 256).map(|s| s.to_string()));
    }
    crate::ime_source::catalog_sample(Some(count), &rows, requested)
}

fn persist(c: &mut Control) -> Result<(), String> {
    c.observation.sampled_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as u64;
    let pending = c.path.with_extension("pending");
    let bytes = serde_json::to_vec_pretty(&c.observation).map_err(|e| e.to_string())?;
    use std::io::Write;
    let result = (|| {
        let mut f = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&pending)?;
        f.write_all(&bytes)?;
        f.flush()?;
        std::fs::rename(&pending, &c.path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&pending);
    }
    result.map_err(|e: std::io::Error| e.to_string())
}
pub fn poll(
    app: &AnyObject,
    editor: &AnyObject,
    window: &AnyObject,
    input: &AnyObject,
) -> Result<(), String> {
    access(|c| unsafe {
        c.observation.current_client_matches = None;
        c.observation.application_active = msg_send![app, isActive];
        c.observation.window_key = msg_send![window, isKeyWindow];
        let actual: *mut AnyObject = msg_send![input, currentEditor];
        let actual_window: *mut AnyObject = msg_send![editor, window];
        let exact = std::ptr::eq(actual.cast_const(), editor)
            && std::ptr::eq(actual_window.cast_const(), window);
        if !exact {
            c.observation.current_client_matches = None;
            return persist(c);
        }
        if c.context.is_none() && c.observation.phase == "waiting_for_own_active_editor" {
            if !c.observation.window_key || !c.observation.application_active {
                return persist(c);
            }
            let context: Option<Retained<AnyObject>> = msg_send![editor, inputContext];
            let Some(context) = context else {
                c.observation.phase = "context_unavailable";
                return persist(c);
            };
            c.observation.current_client_matches = Some(owns(&context, editor));
            if c.observation.current_client_matches != Some(true) {
                c.observation.phase = "client_mismatch";
                return persist(c);
            }
            c.observation.original = source(&context);
            let Some(original) = c.observation.original.as_deref() else {
                c.observation.phase = "original_unknown_no_write";
                return persist(c);
            };
            let sample = catalog(&context, &c.observation.requested);
            let available = sample.requested_available;
            c.observation.source_catalog = Some(sample);
            c.observation.source_catalog_client_matches = c.observation.current_client_matches;
            c.observation.source_catalog_sampled_unix_ms = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| e.to_string())?
                    .as_millis() as u64,
            );
            if available != Some(true) {
                c.observation.phase = if available == Some(false) {
                    "requested_source_not_available_no_write"
                } else {
                    "source_catalog_unknown_no_write"
                };
                return persist(c);
            }
            let requested = NSString::from_str(&c.observation.requested);
            if original != c.observation.requested {
                c.observation.set_calls = 1;
                let _: () = msg_send![&*context,setSelectedKeyboardInputSource:&*requested];
            }
            c.context = Some(context);
            c.observation.phase = "selection_requested";
        }
        if let Some(context) = &c.context {
            c.observation.current_client_matches = Some(owns(context, editor));
            c.observation.current = if c.observation.current_client_matches == Some(true) {
                source(context)
            } else {
                None
            };
            c.observation.phase =
                if c.observation.current.as_deref() == Some(c.observation.requested.as_str()) {
                    "selected_at_sample"
                } else {
                    "selection_not_observed_no_retry"
                };
        }
        persist(c)
    })
}
pub fn restore(editor: &AnyObject) -> Result<(), String> {
    access(|c| unsafe {
        let Some(context) = c.context.as_ref() else {
            return persist(c);
        };
        if !owns(context, editor) {
            c.observation.phase = "restore_client_mismatch_no_write";
            return persist(c);
        }
        c.observation.current = source(context);
        if crate::ime_source::should_restore(
            c.observation.set_calls == 1,
            c.observation.original.as_deref(),
            c.observation.current.as_deref(),
            &c.observation.requested,
        ) {
            let original = NSString::from_str(c.observation.original.as_deref().unwrap());
            c.observation.restore_calls = 1;
            let _: () = msg_send![&**context,setSelectedKeyboardInputSource:&*original];
            c.observation.current = source(context);
            c.observation.phase = if c.observation.current == c.observation.original {
                "restored_at_sample"
            } else {
                "restore_unconfirmed_no_retry"
            };
        } else {
            c.observation.phase = "restore_skipped_no_overwrite";
        }
        persist(c)
    })
}
