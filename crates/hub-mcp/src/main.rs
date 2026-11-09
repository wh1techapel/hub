//! `hub-mcp.exe --data-dir "<Hub's data folder>" [--no-start] [--no-reports]`: Claude Code starts it (registered with
//! `claude mcp add --scope user hub -- ...` by Hub's "Pasang ke Claude Code") and talks to it over
//! stdin and stdout. Only MCP goes to stdout; anything else goes to stderr.
//!
//! The data folder is given, not guessed: Claude's desktop app may see a virtualised
//! `%LOCALAPPDATA%`, so Hub writes the real path into the registration. Hub itself is started
//! (`hub.exe --tray`, beside this program) when it is not running, except when writes into the
//! data folder are redirected (Claude's desktop app); `--no-start` turns that off. The folder is
//! `CLAUDE_PROJECT_DIR` when Claude Code sets it, otherwise the working directory (`place`).
//!
//! The bridge tells Hub about its session and calls (R5); when Claude Code closes stdin it says
//! goodbye before it exits. `--no-reports` is for Hub's own Uji jembatan, which is not a session.

use std::path::PathBuf;

use rmcp::ServiceExt;

fn usage() -> ! {
    eprintln!("usage: hub-mcp --data-dir <Hub's data folder> [--no-start] [--no-reports]");
    std::process::exit(2);
}

#[tokio::main]
async fn main() {
    let mut data_dir: Option<PathBuf> = None;
    let mut start = true;
    let mut reports = true;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data-dir" => data_dir = args.next().map(PathBuf::from),
            "--no-start" => start = false,
            "--no-reports" => reports = false,
            "--version" => {
                println!("hub-mcp {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            _ => usage(),
        }
    }
    let Some(data_dir) = data_dir else { usage() };
    let folder = hub_mcp::place::this_folder();
    // Never when writes into the data folder are moved elsewhere (Claude's desktop app is an MSIX
    // app): that Hub would keep its data in the package's private folder.
    let redirected = start && hub_mcp::place::writes_redirected(&data_dir);
    eprintln!("hub-mcp: folder {}{}", folder.display(), if redirected { "; writes are redirected, so Hub is not started from here" } else { "" });
    let hub_exe = (start && !redirected).then(|| std::env::current_exe().ok().and_then(|exe| exe.parent().map(|dir| dir.join("hub.exe")))).flatten();
    let bridge = hub_mcp::Bridge::new(data_dir, folder, hub_exe, reports);
    let closing = bridge.clone();
    match bridge.serve(rmcp::transport::stdio()).await {
        Ok(service) => {
            if let Err(error) = service.waiting().await {
                eprintln!("hub-mcp: {error}");
            }
            closing.goodbye().await;
        }
        Err(error) => {
            eprintln!("hub-mcp: {error}");
            std::process::exit(1);
        }
    }
}
