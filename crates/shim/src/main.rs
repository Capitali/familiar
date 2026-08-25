//! `familiar-shim` — bridge one stdio MCP server onto a loopback HTTP port the
//! familiar's declared-server client can reach.
//!
//! ```text
//! familiar-shim [--listen 127.0.0.1:0] [--key-file PATH] -- COMMAND [ARGS…]
//! ```
//!
//! Prints the endpoint URL on startup. Every request must carry the per-run bearer
//! token: written to `--key-file` (env-format, mode 0600 — point `servers.json`'s
//! `key_file` at the same path), or printed once when no file is named.

use std::net::TcpListener;

fn main() {
    let mut listen = "127.0.0.1:0".to_string();
    let mut key_file: Option<std::path::PathBuf> = None;
    let mut command: Vec<String> = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--listen" => {
                listen = args
                    .next()
                    .unwrap_or_else(|| usage("--listen needs a value"))
            }
            "--key-file" => {
                key_file = Some(
                    args.next()
                        .unwrap_or_else(|| usage("--key-file needs a value"))
                        .into(),
                )
            }
            "--" => {
                command = args.collect();
                break;
            }
            other => usage(&format!("unknown argument {other}")),
        }
    }
    if command.is_empty() {
        usage("no child command given after --");
    }

    // Loopback only — the shim must never be a network door. Refuse any other bind.
    let listener =
        TcpListener::bind(&listen).unwrap_or_else(|e| fail(&format!("bind {listen}: {e}")));
    let addr = listener
        .local_addr()
        .unwrap_or_else(|e| fail(&format!("local addr: {e}")));
    if !addr.ip().is_loopback() {
        fail(&format!(
            "{addr} is not loopback — the shim serves 127.0.0.1 only"
        ));
    }

    let token = familiar_shim::mint_token().unwrap_or_else(|e| fail(&e.to_string()));
    match &key_file {
        Some(path) => {
            familiar_shim::write_key_file(path, &token)
                .unwrap_or_else(|e| fail(&format!("key file {}: {e}", path.display())));
            println!("token written to {} (TOKEN=…, mode 0600)", path.display());
        }
        None => println!("TOKEN={token}"),
    }

    let (mut child, bridge) = familiar_shim::ChildBridge::spawn(&command[0], &command[1..])
        .unwrap_or_else(|e| fail(&format!("spawn {}: {e}", command[0])));
    println!(
        "shim listening at http://{addr}/mcp for `{}`",
        command.join(" ")
    );

    familiar_shim::serve(listener, &token, &bridge);
    let _ = child.kill();
    let _ = child.wait();
}

fn usage(what: &str) -> ! {
    eprintln!("familiar-shim: {what}");
    eprintln!("usage: familiar-shim [--listen 127.0.0.1:0] [--key-file PATH] -- COMMAND [ARGS…]");
    std::process::exit(2);
}

fn fail(what: &str) -> ! {
    eprintln!("familiar-shim: {what}");
    std::process::exit(1);
}
