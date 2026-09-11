//! Windows-only byte transport. Native operations remain inside rcu-mcp.
fn main() {
    #[cfg(windows)]
    {
        let args: Vec<_> = std::env::args().skip(1).collect();
        let result = (|| -> std::result::Result<(), Box<dyn std::error::Error>> {
            if args.len() != 2 {
                return Err(
                    "Expected PIPE-ID SERVER-SHA256; server must already be explicitly started."
                        .into(),
                );
            }
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            let result = runtime.block_on(rcu_windows::pipe::client(&args[0], &args[1]));
            runtime.shutdown_background();
            result?;
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("{e}");
            std::process::exit(1);
        }
        // Tokio stdin uses a blocking reader. A closed server must not leave that
        // reader keeping this one-shot transport process alive.
        std::process::exit(0);
    }
    #[cfg(not(windows))]
    {
        eprintln!("rcu-pipe-client must execute inside the explicitly allocated Windows VM");
        std::process::exit(2);
    }
}
