//! Headless app for trying the standalone debugger from another terminal.
#[cfg(unix)]
mod unix {
    use std::{io, path::PathBuf, time::Duration};

    use clap::Parser;
    use serde::Serialize;
    use serde_json::json;
    use tui_dispatch_core::{Action, ReducerResult, Store};
    use tui_dispatch_debug::{
        observe::{DebugCollector, DebugStoreExt},
        transport::DebugServer,
    };

    #[derive(Parser)]
    struct Args {
        socket: PathBuf,
        #[arg(long, default_value_t = 60)]
        ticks: u32,
    }

    #[derive(Clone, Debug, Serialize)]
    struct Increment;
    impl Action for Increment {
        fn name(&self) -> &'static str {
            "Increment"
        }
    }

    pub async fn run() -> io::Result<()> {
        let args = Args::parse();
        let collector = DebugCollector::new(json!(0), 64);
        let mut store = Store::new(0u32, |count, _: Increment| -> ReducerResult {
            *count += 1;
            ReducerResult::changed()
        })
        .with_observer(collector.observer());
        // Dispatch before binding to demonstrate that attachment preserves history.
        store.dispatch(Increment);
        let server = DebugServer::bind(&args.socket, collector).await?;
        println!("Debugger socket: {}", args.socket.display());
        for _ in 0..args.ticks {
            tokio::time::sleep(Duration::from_secs(1)).await;
            store.dispatch(Increment);
            println!("count={}", store.state());
        }
        server.shutdown().await
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        unix::run().await
    }
    #[cfg(not(unix))]
    {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "debugger attachment requires Unix",
        ))
    }
}
