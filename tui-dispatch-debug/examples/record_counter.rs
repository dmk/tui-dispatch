//! Headless counter that writes a recording for the standalone debugger.
use std::{io, path::PathBuf, time::Duration};

use clap::Parser;
use serde::Serialize;
use serde_json::json;
use tui_dispatch_core::{Action, ReducerResult, Store};
use tui_dispatch_debug::{
    observe::{DebugCollector, DebugStoreExt},
    recording::RecordingWriter,
};

#[derive(Parser)]
#[command(about = "Record a counter's dispatches to a new .tuidbg file")]
struct Args {
    recording: PathBuf,
    #[arg(long, default_value_t = 60)]
    ticks: u32,
    #[arg(long, default_value_t = 500)]
    interval_ms: u64,
    /// Close without a finish marker, demonstrating interrupted-file recovery
    #[arg(long)]
    unfinished: bool,
}

#[derive(Clone, Debug, Serialize)]
struct Increment;
impl Action for Increment {
    fn name(&self) -> &'static str {
        "Increment"
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();
    let mut writer = RecordingWriter::create(&args.recording)?;
    let collector = DebugCollector::new(json!(0), 1);
    let mut store = Store::new(0u32, |count, _: Increment| -> ReducerResult {
        *count += 1;
        ReducerResult::changed()
    })
    .with_observer(collector.observer());
    println!("Recording: {}", args.recording.display());
    for _ in 0..args.ticks {
        store.dispatch(Increment);
        // This example emits exactly one record per dispatch. Copy it before
        // the next dispatch evicts it from the one-record collector.
        writer.append(&collector.records()[0])?;
        writer.flush()?;
        println!("count={}", store.state());
        tokio::time::sleep(Duration::from_millis(args.interval_ms)).await;
    }
    if args.unfinished {
        println!("Closed without a finish marker; open offline to recover.");
        Ok(())
    } else {
        writer.finish()?;
        println!("Recording finished.");
        Ok(())
    }
}
