//! Inspector CPU baseline; setup/cloning outside timed sections unless named.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, Terminal};
use serde_json::json;
use std::{hint::black_box, time::Instant};
use tui_dispatch_core::EventKind;
use tui_dispatch_debug::debug::{ActionLogDisplayEntry, DebugInspector};

fn key(view: &mut DebugInspector, code: KeyCode) {
    view.handle_event(&EventKind::Key(KeyEvent::new(code, KeyModifiers::NONE)));
}
fn measure(label: &str, mut run: impl FnMut() -> f64) {
    black_box(run());
    let mut times: Vec<_> = (0..7).map(|_| run()).collect();
    times.sort_by(f64::total_cmp);
    println!(
        "{label}: us median={:.3} min={:.3} max={:.3}",
        times[3], times[0], times[6]
    );
}
fn display() {
    println!("one warmup + seven samples; microseconds/op; release; test backend 120x30");
    for (count, bytes) in [(64, 0), (1024, 4096), (1024, 65536)] {
        let entries: Vec<_> = (1..=count)
            .rev()
            .map(|sequence| ActionLogDisplayEntry {
                sequence,
                name: "Add".into(),
                params: format!("[applied] {{\"amount\":{sequence}}}"),
                params_detail: format!("{{\"amount\":{sequence}}}"),
                elapsed: "—".into(),
            })
            .collect();
        let state = json!({"count":count,"text":"x".repeat(bytes)});
        let mut view = DebugInspector::new().fullscreen();
        let label = format!("history={count} state={bytes}");
        measure(&format!("{label} display_update"), || {
            let mut total = 0.0;
            for _ in 0..32 {
                let entries = entries.clone();
                let state = state.clone();
                let start = Instant::now();
                view.update(state, entries);
                total += start.elapsed().as_secs_f64() * 1e6;
            }
            total / 32.0
        });
        measure(&format!("{label} search"), || {
            let start = Instant::now();
            for _ in 0..32 {
                key(&mut view, KeyCode::Char('/'));
                key(&mut view, KeyCode::Char('a'));
                key(&mut view, KeyCode::Backspace);
                key(&mut view, KeyCode::Enter);
            }
            start.elapsed().as_secs_f64() * 1e6 / 32.0
        });
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        measure(&format!("{label} list_draw"), || {
            let start = Instant::now();
            for _ in 0..32 {
                terminal.draw(|f| view.render(f, "benchmark")).unwrap();
            }
            start.elapsed().as_secs_f64() * 1e6 / 32.0
        });
    }
}

fn snapshot(count: u64, bytes: usize) -> tui_dispatch_debug::records::DebugSnapshot {
    use tui_dispatch_debug::{observe::DebugRecord, records::DebugSnapshot};
    let history: Vec<tui_dispatch_debug::records::SnapshotRecord> = (1..=count)
        .map(|sequence| {
            DebugRecord::dispatch(
                sequence,
                "Add",
                json!({"amount":1,"text":"a".repeat(bytes/16)}),
                json!({"count":sequence,"text":"s".repeat(bytes)}),
                true,
            )
            .into()
        })
        .collect();
    DebugSnapshot {
        state: history.last().unwrap().state.clone(),
        history,
        components: None,
        control: None,
    }
}
fn typed() {
    for (count, bytes) in [(64, 0), (1024, 4096), (1024, 65536)] {
        let data = std::sync::Arc::new(snapshot(count, bytes));
        let mut view = DebugInspector::new().fullscreen();
        let label = format!("history={count} state={bytes} payload={}", bytes / 16);
        measure(&format!("{label} snapshot_clone"), || {
            let start = Instant::now();
            for _ in 0..8 {
                black_box(data.as_ref().clone());
            }
            start.elapsed().as_secs_f64() * 1e6 / 8.0
        });
        measure(&format!("{label} typed_update"), || {
            let mut elapsed = 0.0;
            for _ in 0..8 {
                let next = data.as_ref().clone();
                let start = Instant::now();
                view.update_snapshot(next);
                elapsed += start.elapsed().as_secs_f64() * 1e6;
            }
            elapsed / 8.0
        });
        measure(&format!("{label} shared_refresh"), || {
            let start = Instant::now();
            for _ in 0..8 {
                view.update_snapshot(std::sync::Arc::clone(&data));
            }
            start.elapsed().as_secs_f64() * 1e6 / 8.0
        });
        measure(&format!("{label} selected_diff"), || {
            let start = Instant::now();
            for _ in 0..256 {
                black_box(view.selected_diff());
            }
            start.elapsed().as_secs_f64() * 1e6 / 256.0
        });
        measure(&format!("{label} open_close_detail"), || {
            let start = Instant::now();
            for _ in 0..8 {
                key(&mut view, KeyCode::Enter);
                key(&mut view, KeyCode::Esc);
            }
            start.elapsed().as_secs_f64() * 1e6 / 8.0
        });
    }
    for nodes in [64, 4096] {
        let before = json!((0..nodes).collect::<Vec<_>>());
        let mut after = before.clone();
        after[nodes / 2] = json!(-1);
        measure(&format!("array_nodes={nodes} one_change_diff"), || {
            let start = Instant::now();
            for _ in 0..64 {
                black_box(tui_dispatch_debug::inspector::state_diff(&before, &after));
            }
            start.elapsed().as_secs_f64() * 1e6 / 64.0
        });
    }
}

fn main() {
    display();
    typed();
}
