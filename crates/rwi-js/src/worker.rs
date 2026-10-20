use crate::protocol::*;
use rquickjs::{CaughtError, Context, Ctx, Exception, Function, Promise, Runtime, Value};
use std::{
    cell::RefCell,
    io::{self, BufRead, BufReader, Read, Write},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicIsize, Ordering},
    },
    time::{Duration, Instant},
};

fn read(input: &mut impl BufRead) -> io::Result<Option<String>> {
    let mut bytes = Vec::new();
    let n = input
        .take((FRAME_CAP + 1) as u64)
        .read_until(b'\n', &mut bytes)?;
    if n == 0 {
        return Ok(None);
    }
    if n > FRAME_CAP || bytes.last() != Some(&b'\n') {
        return Err(io::Error::other("worker_frame_limit_or_incomplete"));
    }
    String::from_utf8(bytes).map(Some).map_err(io::Error::other)
}

fn send(output: &mut impl Write, raw: &str) -> io::Result<()> {
    if raw.len() + 1 > FRAME_CAP {
        return Err(io::Error::other("worker_frame_limit"));
    }
    writeln!(output, "{raw}")?;
    output.flush()
}

pub fn run() -> io::Result<()> {
    run_io(BufReader::new(io::stdin()), io::stdout())
}

fn run_io(input: impl BufRead + 'static, output: impl Write + 'static) -> io::Result<()> {
    let input = Rc::new(RefCell::new(input));
    let output = Rc::new(RefCell::new(output));
    let runtime = Runtime::new().map_err(io::Error::other)?;
    runtime.set_memory_limit(MEMORY_CAP);
    runtime.set_max_stack_size(512 * 1024);
    let deadline = Arc::new(Mutex::new(Instant::now() + Duration::from_secs(60)));
    let interrupt = deadline.clone();
    runtime.set_interrupt_handler(Some(Box::new(move || {
        interrupt.lock().map_or(true, |d| Instant::now() >= *d)
    })));
    let context = Context::full(&runtime).map_err(io::Error::other)?;
    let rejected = Arc::new(AtomicIsize::new(0));
    let tracker = rejected.clone();
    runtime.set_host_promise_rejection_tracker(Some(Box::new(move |_, _, _, handled| {
        if handled {
            tracker.fetch_sub(1, Ordering::Relaxed);
        } else {
            tracker.fetch_add(1, Ordering::Relaxed);
        }
    })));
    context
        .with(|ctx| -> rquickjs::Result<()> {
            let tx = output.clone();
            let rx = input.clone();
            ctx.globals().set(
                "__rcuCall",
                Function::new(
                    ctx.clone(),
                    move |ctx: Ctx<'_>, tool: String, args: String| {
                        let reply = (|| -> io::Result<String> {
                            if args.len() > 1024 * 1024 || tool.len() > 128 {
                                return Err(io::Error::other("call_argument_limit"));
                            }
                            let event = Event::Call {
                                tool,
                                arguments: serde_json::from_str(&args)?,
                            };
                            let raw = serde_json::to_string(&event)?;
                            send(&mut *tx.borrow_mut(), &raw)
                                .and_then(|_| read(&mut *rx.borrow_mut()))
                                .and_then(|r| {
                                    r.ok_or_else(|| io::Error::other("supervisor_disconnected"))
                                })
                        })();
                        reply.map_err(|e| Exception::throw_message(&ctx, &e.to_string()))
                    },
                )?,
            )?;
            let tx = output.clone();
            ctx.globals().set(
                "__rcuEmit",
                Function::new(ctx.clone(), move |ctx: Ctx<'_>, raw: String| {
                    let result = (|| -> io::Result<()> {
                        if raw.len() > OUTPUT_CAP {
                            return Err(io::Error::other("js_output_limit"));
                        }
                        let event = Event::Emit {
                            value: serde_json::from_str(&raw)?,
                        };
                        send(&mut *tx.borrow_mut(), &serde_json::to_string(&event)?)
                    })();
                    result.map_err(|e| Exception::throw_message(&ctx, &e.to_string()))
                })?,
            )?;
            let tx = output.clone();
            ctx.globals().set(
                "__rcuImage",
                Function::new(
                    ctx.clone(),
                    move |ctx: Ctx<'_>, data: String, mime_type: String| {
                        send(
                            &mut *tx.borrow_mut(),
                            &serde_json::to_string(&Event::Image { data, mime_type })
                                .expect("JSON event"),
                        )
                        .map_err(|e| Exception::throw_message(&ctx, &e.to_string()))
                    },
                )?,
            )?;
            // One canonical SDK, with its ESM export declarations removed for the
            // private global bootstrap. No disk-based import or code loader.
            let sdk = include_str!("../../../integrations/javascript/rwi-client.mjs")
                .replace("export class ", "class ");
            ctx.eval::<(), _>(sdk)?;
            ctx.eval::<(), _>(include_str!("bootstrap.js"))
        })
        .map_err(io::Error::other)?;
    loop {
        let Some(raw) = read(&mut *input.borrow_mut())? else {
            return Ok(());
        };
        let cell: Cell = serde_json::from_str(&raw)?;
        if cell.code.len() > CODE_CAP || !(100..=60_000).contains(&cell.timeout_ms) {
            return Err(io::Error::other("invalid_cell_limits"));
        }
        *deadline
            .lock()
            .map_err(|_| io::Error::other("deadline_poisoned"))? =
            Instant::now() + Duration::from_millis(cell.timeout_ms);
        let mut error = context.with(|ctx| {
            // Explicit state.* persists. Lexical cell locals do not. This avoids
            // source rewriting and accidental redeclarations across model turns.
            let result = ctx
                .eval::<Promise, _>(format!("(async () => {{\n{}\n}})()", cell.code))
                .and_then(|p| p.finish::<Value>().map(|_| ()));
            result.err().map(|e| {
                // Formatting an arbitrary thrown object may run user code; the
                // same interrupt and parent deadline still apply.
                CaughtError::from_error(&ctx, e).to_string()
            })
        });
        // Never resume orphan microtasks on the next model turn. Fail closed;
        // the supervisor destroys this context after any cell error.
        if runtime.is_job_pending() && error.is_none() {
            error = Some("pending_jobs_after_cell; await every operation".into());
        }
        if rejected.load(Ordering::Relaxed) > 0 && error.is_none() {
            error = Some("unhandled_promise_rejection; await every operation".into());
        }
        let failed = error.is_some();
        let event = serde_json::to_string(&Event::Done { error })?;
        send(&mut *output.borrow_mut(), &event)?;
        if failed {
            return Ok(());
        }
    }
}
