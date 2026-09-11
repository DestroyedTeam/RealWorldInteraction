use rcu_native_link::{install, invalid};
use std::{io, process, time::Duration};
use tokio::io::AsyncReadExt;
use zeroize::Zeroizing;
async fn run() -> io::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.is_empty()
        || args.len() > 2
        || !matches!(
            args[0].as_str(),
            "--validate-bundle"
                | "--import"
                | "--approve-next"
                | "--make-notepad-approval"
                | "--make-bundle"
                | "--make-client-entry"
                | "--make-plugin"
        )
    {
        return Err(invalid(
            "usage: rcu-native-setup --validate-bundle | --import | --approve-next | --make-notepad-approval | --make-bundle ABS_PRIVATE_NEW_FILE; private JSON on stdin",
        ));
    }
    if (args[0] == "--make-bundle") != (args.len() == 2) {
        return Err(invalid(
            "only_make_bundle_requires_exact_private_output_path",
        ));
    }
    let mut bytes = Zeroizing::new(Vec::new());
    tokio::time::timeout(
        Duration::from_secs(30),
        tokio::io::stdin()
            .take(install::BUNDLE_CAP as u64 + 1)
            .read_to_end(&mut bytes),
    )
    .await??;
    if bytes.len() > install::BUNDLE_CAP {
        return Err(invalid("private_stdin_cap"));
    }
    let result = match args[0].as_str() {
        "--make-plugin" => rcu_native_link::plugin::export(serde_json::from_slice(&bytes)?),
        "--make-bundle" => {
            let output = std::path::Path::new(&args[1]);
            let parent = output
                .parent()
                .ok_or_else(|| invalid("private_bundle_parent"))?;
            let name = output
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| invalid("private_bundle_name"))?;
            let directory = rcu_native_link::private::Directory::open(parent)?;
            let bundle = install::make_bundle(&bytes)?;
            directory.create_new(name, &bundle, install::BUNDLE_CAP)?;
            Ok(
                serde_json::json!({"private_bundle_written":output,"bytes":bundle.len(),"sha256":rcu_native_link::sha(&bundle),"private_material_emitted_to_stdout":false}),
            )
        }
        "--make-notepad-approval" => serde_json::to_value(rcu_native_link::plan::notepad(
            serde_json::from_slice(&bytes)?,
        )?)
        .map_err(io::Error::other),
        "--make-client-entry" => {
            rcu_native_link::plan::write_client_entry(serde_json::from_slice(&bytes)?)
        }
        "--validate-bundle" => install::validate_bundle(&bytes),
        "--import" => install::import(&bytes),
        _ => install::approve_next(&bytes),
    }?;
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
fn main() {
    let result = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .and_then(|runtime| {
            let result = runtime.block_on(run());
            runtime.shutdown_timeout(Duration::from_secs(2));
            result
        });
    if let Err(e) = result {
        eprintln!("native-setup: {e}");
        process::exit(1);
    }
}
