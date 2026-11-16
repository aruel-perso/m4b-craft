use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use m4b_core::{Cancel, DesktopFfmpeg, ExportEvent, MediaBackend, Project};
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[derive(Parser)]
#[command(
    version,
    about = "m4b-Craft: local audiobook creation; never overwrites exports"
)]
struct Args {
    #[arg(long)]
    ffmpeg: Option<PathBuf>,
    #[arg(long)]
    ffprobe: Option<PathBuf>,
    #[command(subcommand)]
    command: Action,
}
#[derive(Subcommand)]
enum Action {
    /// Import tracks in natural order and save a project (paths become absolute).
    New {
        #[arg(short, long)]
        project: PathBuf,
        #[arg(long, default_value = "")]
        title: String,
        #[arg(long, default_value = "")]
        author: String,
        #[arg(long, default_value = "")]
        narrator: String,
        #[arg(long, default_value = "")]
        description: String,
        #[arg(long)]
        cover: Option<PathBuf>,
        #[arg(long, default_value_t = 96)]
        bitrate: u32,
        #[arg(required = true)]
        tracks: Vec<PathBuf>,
    },

    /// Open an existing M4B and save its metadata/chapters as a project.
    Open {
        input: PathBuf,
        #[arg(short, long)]
        project: PathBuf,
        #[arg(long)]
        extract_cover: Option<PathBuf>,
    },
    Probe {
        input: PathBuf,
    },
    Encoders,
    Check {
        project: PathBuf,
    },
    Export {
        project: PathBuf,
        output: PathBuf,
    },

    /// Sequential newline JSON-RPC MCP server; stdout contains only protocol messages.
    Mcp,
}
fn main() -> Result<()> {
    let args = Args::parse();
    let mut backend = DesktopFfmpeg::default();
    if let Some(path) = args.ffmpeg {
        backend.ffmpeg = path;
    }
    if let Some(path) = args.ffprobe {
        backend.ffprobe = path;
    }
    match args.command {
        Action::New {
            project,
            title,
            author,
            narrator,
            description,
            cover,
            bitrate,
            tracks,
        } => {
            anyhow::ensure!(!project.exists(), "Project exists");
            let mut p = Project::default();
            p.import(&backend, tracks)?;
            p.metadata = m4b_core::Metadata {
                title,
                author,
                narrator,
                description,
                ..Default::default()
            };
            p.settings.bitrate_kbps = bitrate;
            p.cover = cover.map(std::fs::canonicalize).transpose()?;
            p.validate()?;
            p.save_new(&project)?;
            println!(
                "Saved {} ({} tracks, {} ms)",
                project.display(),
                p.tracks.len(),
                p.duration_ms()
            );
        }
        Action::Open {
            input,
            project,
            extract_cover,
        } => {
            anyhow::ensure!(!project.exists(), "Project exists");
            let mut p = backend.open(&input)?;
            if let Some(cover) = extract_cover {
                backend.extract_cover(&input, &cover)?;
                p.cover = Some(std::fs::canonicalize(cover)?);
            }
            p.save_new(&project)?;
            println!("Saved {}", project.display());
        }
        Action::Probe { input } => {
            let p = backend.open(&input)?;
            println!("{}", serde_json::to_string_pretty(&p)?);
        }
        Action::Encoders => {
            for e in backend.encoders()? {
                println!("{e}");
            }
        }
        Action::Check { project } => {
            let p = Project::load(&project)?;
            p.validate()?;
            for t in &p.tracks {
                let fresh = backend.probe(&t.path)?;
                anyhow::ensure!(
                    fresh.track.duration_ms.abs_diff(t.duration_ms) <= 100,
                    "Source changed"
                );
            }
            println!(
                "Valid: {} tracks, {} ms, estimated {} bytes AAC",
                p.tracks.len(),
                p.duration_ms(),
                p.estimated_bytes()
            );
        }
        Action::Export { project, output } => {
            let p = Project::load(&project)?;
            let cancel: Cancel = Arc::new(AtomicBool::new(false));
            let c = cancel.clone();
            ctrlc::set_handler(move || c.store(true, Ordering::Relaxed))?;
            backend.export(&p, &output, cancel, &mut |e| match e {
                ExportEvent::Log(s) => eprintln!("{s}"),
                ExportEvent::Progress(p) => eprintln!("{:.0}%", p * 100.),
            })?;
        }
        Action::Mcp => mcp(&backend)?,
    }
    Ok(())
}
fn mcp(backend: &DesktopFfmpeg) -> Result<()> {
    use serde_json::{json, Value};
    // Intentionally read-only MCP: project writes and exports stay explicit CLI/UI operations.
    for line in io::stdin().lock().lines() {
        let line = line?;
        if line.len() > 1_048_576 {
            bail!("MCP request too large");
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                println!(
                    "{}",
                    json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}})
                );
                continue;
            }
        };
        if req["jsonrpc"] != "2.0"
            || !req["method"].is_string()
            || req
                .get("id")
                .is_some_and(|id| !id.is_null() && !id.is_string() && !id.is_number())
        {
            println!(
                "{}",
                json!({"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"Invalid Request"}})
            );
            continue;
        }
        let Some(id) = req.get("id") else {
            continue;
        };
        let result: Result<Value> = (|| {
            Ok(match req["method"].as_str().unwrap_or("") {
                "initialize" => {
                    json!({"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"m4b-Craft","version":env!("CARGO_PKG_VERSION")}})
                }
                "ping" => json!({}),
                "tools/list" => {
                    json!({"tools":[{"name":"probe","description":"Read local audio metadata and chapters","inputSchema":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}},{"name":"inspect_project","description":"Read a local m4b-Craft project","inputSchema":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}}]})
                }
                "tools/call" => {
                    let path = PathBuf::from(
                        req["params"]["arguments"]["path"]
                            .as_str()
                            .context("Missing path")?,
                    );
                    let data = match req["params"]["name"].as_str().unwrap_or("") {
                        "probe" => backend.open(&path),
                        "inspect_project" => Project::load(&path),
                        _ => bail!("Unknown tool"),
                    };
                    match data {
                        Ok(p) => {
                            json!({"content":[{"type":"text","text":serde_json::to_string_pretty(&p)?}]})
                        }
                        Err(e) => {
                            json!({"isError":true,"content":[{"type":"text","text":e.to_string()}]})
                        }
                    }
                }
                _ => bail!("Method not found"),
            })
        })();
        let response = match result {
            Ok(v) => json!({"jsonrpc":"2.0","id":id,"result":v}),
            Err(e) => {
                let code = if e.to_string() == "Method not found" {
                    -32601
                } else {
                    -32602
                };
                json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":e.to_string()}})
            }
        };
        println!("{response}");
        io::stdout().flush()?;
    }
    Ok(())
}
