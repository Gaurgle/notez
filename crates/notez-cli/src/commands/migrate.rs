//! `notez migrate-from-legacy` — one-time port of the notez-cli layout
//! (numbered dirs + symlink mirrors) into notez's scope model.

use anyhow::Result;
use notez_core::config::Config;
use notez_core::migrate;

pub fn run(dry_run: bool, config: &Config) -> Result<()> {
    let plan = migrate::plan(config);
    if plan.is_empty() {
        println!("Nothing to migrate: no numbered dirs match the legacy project registry.");
        return Ok(());
    }

    println!("Migration plan:");
    for item in &plan {
        println!("  {} → {}  [{}]", item.from, item.to, item.note);
    }

    if dry_run {
        println!("(dry run — nothing changed)");
        return Ok(());
    }

    println!();
    for line in migrate::apply(config)? {
        println!("  {line}");
    }
    println!("Done. Review conflicts above (if any), then `notez sync`.");
    Ok(())
}
