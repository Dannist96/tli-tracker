//! Debug tool: plays a game log through the parser and tracker and prints
//! what was recognised.
//!
//!     cargo run --example replay -- path/to/UE_game.log [--events]

use std::collections::{BTreeMap, HashMap, HashSet};
use tli_tracker_lib::parser::{Event, Parser};
use tli_tracker_lib::prices::{MarketTable, PriceBook};
use tli_tracker_lib::tracker::Tracker;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("usage: replay <log> [--events]");
    let show_events = args.iter().any(|a| a == "--events");
    let bytes = std::fs::read(path).expect("cannot read log");
    let text = String::from_utf8_lossy(&bytes);

    let market = MarketTable::default();
    let (local, manual, excluded) = (HashMap::new(), HashMap::new(), HashSet::new());
    let book = PriceBook { market: &market, local: &local, manual: &manual, excluded: &excluded, apply_tax: true };
    let mut parser = Parser::new();
    let mut tracker = Tracker::new(0);
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();

    let mut handle = |event: Event, tracker: &mut Tracker| {
        let kind = format!("{event:?}").split([' ', '{', '(']).next().unwrap_or("").to_string();
        *counts.entry(kind).or_default() += 1;
        if show_events {
            println!("{}", summarize(&event));
        }
        tracker.handle(event, &book);
    };
    for line in text.lines() {
        for e in parser.feed(line) {
            handle(e, &mut tracker);
        }
    }
    for e in parser.flush() {
        handle(e, &mut tracker);
    }

    println!("\n== events");
    for (k, n) in &counts {
        println!("{k:>16} {n}");
    }
    println!("\n== tracker");
    println!("player   {:?}", tracker.player.as_ref().map(|p| (&p.name, &p.id, &p.season_id)));
    println!("baseline trusted={} pages={:?}", tracker.baseline_trusted, tracker.trusted_pages);
    println!("bag stacks {}", tracker.inventory.len());
    println!("status {:?}, history {} runs, pending costs {}", tracker.status, tracker.history.len(), tracker.pending_costs.len());
    for run in tracker.history.iter().chain(tracker.current.iter()) {
        let loot: i64 = run.loot.iter().map(|d| d.qty).sum();
        println!(
            "run #{} map={} type={} area={} dur={}s loot_lines={} loot_qty={} cost={:?}",
            run.id,
            run.map_id,
            run.map_type,
            run.area_id,
            run.duration_ms / 1000,
            run.loot.len(),
            loot,
            run.cost.iter().map(|d| (d.base_id.as_str(), d.qty)).collect::<Vec<_>>()
        );
    }
}

fn summarize(e: &Event) -> String {
    match e {
        Event::Loot { ts, items } => format!("{ts} LOOT {:?}", items.iter().map(|i| (&i.base_id, i.quantity, i.page_id)).collect::<Vec<_>>()),
        Event::MapCost { ts, items } => format!("{ts} COST {:?}", items.iter().map(|i| (&i.base_id, i.quantity)).collect::<Vec<_>>()),
        Event::PageReset { ts, page, items } => format!("{ts} RESET page={page:?} items={}", items.len()),
        Event::Push2 { ts, items } => format!("{ts} PUSH2 {:?}", items.iter().map(|i| (&i.base_id, i.quantity)).collect::<Vec<_>>()),
        Event::OtherItemChange { ts, items } => format!("{ts} OTHER {}", items.len()),
        Event::JoinFight { ts, info } => format!("{ts} JOIN {info:?}"),
        other => format!("{other:?}"),
    }
}
