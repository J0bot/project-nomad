//! `tsoin` — git-of-states for any file or directory, from the shell.
//!
//! Record real bytes, replay any old state bit-exact, fork timelines for free,
//! and watch the storage collapse — all persisted under `.tsoin/` so it works
//! across separate command invocations. This binary is a thin clap front-end
//! over [`tsoin::persist::Repo`], which wires the crate's temporal recorder,
//! content-addressed store, model codec, and xerboxion detector to disk.
//!
//! See `docs/CLI-v0.md` for the on-disk format and command reference.

use clap::{Parser, Subcommand};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use tsoin::persist::{self, Kind, Repo};

#[derive(Parser)]
#[command(
    name = "tsoin",
    about = "git-of-states: record / replay / fork any file or directory; storage collapses to the surprise",
    version
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Initialize a .tsoin/ repository in the current directory.
    Init {
        /// Reset an existing repository instead of refusing.
        #[arg(long)]
        force: bool,
    },
    /// Snapshot a file or directory as a new state on the current branch.
    Record {
        /// File or directory to snapshot.
        path: PathBuf,
        /// Message describing this state.
        #[arg(short, long, default_value = "")]
        message: String,
    },
    /// List the current branch's states, newest-first.
    Log,
    /// Reconstruct a state's bytes bit-exact.
    Replay {
        /// State id (full or any unambiguous short prefix).
        id: String,
        /// Where to write: a file path, or a directory for a dir state.
        /// Default: stdout for files, required for directories.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Create a new branch (free) at a state (default: current head).
    Fork {
        /// New branch name.
        branch: String,
        /// State to fork at (default: current branch head).
        id: Option<String>,
    },
    /// Switch the current branch.
    Checkout {
        /// Branch to switch to.
        branch: String,
    },
    /// List branches (* marks the current one).
    Branch,
    /// Show store footprint vs naive, ratio, and counts.
    Stats,
    /// List detected xerboxions (identical-state coincidences).
    Xerboxions,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("tsoin: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;

    match cli.cmd {
        Cmd::Init { force } => {
            Repo::init(&cwd, force).map_err(|e| e.to_string())?;
            println!("Initialized empty tsoin repository in {}", persist::Repo::dir_for(&cwd).display());
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Record { path, message } => cmd_record(&cwd, &path, &message),
        Cmd::Log => cmd_log(&cwd),
        Cmd::Replay { id, out } => cmd_replay(&cwd, &id, out.as_deref()),
        Cmd::Fork { branch, id } => cmd_fork(&cwd, &branch, id.as_deref()),
        Cmd::Checkout { branch } => cmd_checkout(&cwd, &branch),
        Cmd::Branch => cmd_branch(&cwd),
        Cmd::Stats => cmd_stats(&cwd),
        Cmd::Xerboxions => cmd_xerboxions(&cwd),
    }
}

/// Open the repo discovered from `cwd`, or error if none.
fn open(cwd: &Path) -> Result<Repo, String> {
    let root = Repo::discover(cwd)
        .ok_or_else(|| "not a tsoin repository (run `tsoin init`)".to_string())?;
    Repo::open(&root).map_err(|e| e.to_string())
}

fn cmd_record(cwd: &Path, path: &Path, message: &str) -> Result<ExitCode, String> {
    let mut repo = open(cwd)?;
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;

    let (frame, kind) = if meta.is_dir() {
        (
            persist::serialize_dir(path).map_err(|e| e.to_string())?,
            Kind::Dir,
        )
    } else {
        (
            std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?,
            Kind::File,
        )
    };

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());

    let (id, written, xerb) = repo
        .record(&frame, kind, &name, message)
        .map_err(|e| e.to_string())?;

    let disk = repo.store_footprint_disk().map_err(|e| e.to_string())?;
    let kindstr = match kind {
        Kind::File => "file",
        Kind::Dir => "dir ",
    };
    println!(
        "[{}] recorded {} {}  raw {}  delta stored {}  cumulative store {}",
        id.short(),
        kindstr,
        name,
        human(frame.len() as u64),
        human(written as u64),
        human(disk),
    );
    if let Some(x) = xerb {
        println!(
            "  XERBOXION: this state is byte-for-byte identical to earlier state {}",
            x.first.short()
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_log(cwd: &Path) -> Result<ExitCode, String> {
    let repo = open(cwd)?;
    let chain = repo.current_chain();
    if chain.is_empty() {
        println!("branch '{}' has no states yet", repo.head);
        return Ok(ExitCode::SUCCESS);
    }
    println!("branch {} ({} states)", repo.head, chain.len());
    for m in &chain {
        let msg = if m.msg.is_empty() { "(no message)" } else { &m.msg };
        println!(
            "state {}  {}  {}  raw {}  msg: {}",
            m.id.short(),
            fmt_time(m.time),
            m.name,
            human(m.raw_size),
            msg,
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_replay(cwd: &Path, id: &str, out: Option<&Path>) -> Result<ExitCode, String> {
    let repo = open(cwd)?;
    let node = repo.resolve(id)?;
    let meta = repo
        .meta(&node)
        .ok_or_else(|| format!("state {id} has no metadata"))?
        .clone();
    let bytes = repo
        .rec
        .try_replay(&node)
        .ok_or_else(|| format!("state {id} could not be reconstructed (corrupt store?)"))?;

    match meta.kind {
        Kind::Dir => {
            let dest = out.ok_or_else(|| {
                "this is a directory state; pass --out <dir> to restore it".to_string()
            })?;
            persist::restore_dir(&bytes, dest).map_err(|e| e.to_string())?;
            println!("restored directory state {} to {}", node.short(), dest.display());
        }
        Kind::File => match out {
            Some(p) => {
                std::fs::write(p, &bytes).map_err(|e| format!("{}: {e}", p.display()))?;
                eprintln!("wrote {} ({}) to {}", node.short(), human(bytes.len() as u64), p.display());
            }
            None => {
                std::io::stdout()
                    .write_all(&bytes)
                    .map_err(|e| e.to_string())?;
            }
        },
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_fork(cwd: &Path, branch: &str, id: Option<&str>) -> Result<ExitCode, String> {
    let mut repo = open(cwd)?;
    let at = match id {
        Some(s) => Some(repo.resolve(s)?),
        None => None,
    };
    repo.fork(branch, at).map_err(|e| e.to_string())?;
    match at {
        Some(h) => println!("forked branch '{branch}' at state {} (free, shares all prior blobs)", h.short()),
        None => println!("forked branch '{branch}' at current head (free, shares all prior blobs)"),
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_checkout(cwd: &Path, branch: &str) -> Result<ExitCode, String> {
    let mut repo = open(cwd)?;
    repo.checkout(branch).map_err(|e| e.to_string())?;
    println!("switched to branch '{branch}'");
    Ok(ExitCode::SUCCESS)
}

fn cmd_branch(cwd: &Path) -> Result<ExitCode, String> {
    let repo = open(cwd)?;
    for name in repo.branch_names() {
        let marker = if name == repo.head { "*" } else { " " };
        let head = repo
            .branch_head(&name)
            .flatten()
            .map(|h| h.short())
            .unwrap_or_else(|| "(empty)".to_string());
        println!("{marker} {name}  -> {head}");
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_stats(cwd: &Path) -> Result<ExitCode, String> {
    let repo = open(cwd)?;
    let naive = repo.naive_cost();
    let disk = repo.store_footprint_disk().map_err(|e| e.to_string())?;
    let ratio = if disk == 0 {
        0.0
    } else {
        naive as f64 / disk as f64
    };
    let pct = if naive == 0 {
        0.0
    } else {
        100.0 * disk as f64 / naive as f64
    };
    println!("tsoin store statistics");
    println!("  states recorded      : {}", repo.state_count());
    println!("  branches             : {}", repo.branch_count());
    println!("  xerboxions detected  : {}", repo.xerboxions().len());
    println!("  naive cost (sum raw) : {}", human(naive));
    println!("  store on disk        : {}", human(disk));
    println!("  store / naive        : {pct:.1}%  ({ratio:.2}x collapse)");
    Ok(ExitCode::SUCCESS)
}

fn cmd_xerboxions(cwd: &Path) -> Result<ExitCode, String> {
    let repo = open(cwd)?;
    let xs = repo.xerboxions();
    if xs.is_empty() {
        println!("no xerboxions detected (no two recorded states are byte-for-byte identical)");
        return Ok(ExitCode::SUCCESS);
    }
    println!("{} xerboxion(s) — identical-state coincidences:", xs.len());
    for x in xs {
        println!(
            "  content {}  first {}  repeat {}",
            x.content_hash.short(),
            x.first.short(),
            x.repeat.short()
        );
    }
    Ok(ExitCode::SUCCESS)
}

/// Human-readable byte size.
fn human(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    if n < 1024 {
        return format!("{n} B");
    }
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    format!("{v:.1} {}", UNITS[u])
}

/// Format a unix timestamp as a compact UTC string without pulling in chrono.
fn fmt_time(secs: u64) -> String {
    // Days since epoch -> y/m/d via civil-from-days (Howard Hinnant's algorithm).
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02} {h:02}:{mi:02}:{s:02}Z")
}
