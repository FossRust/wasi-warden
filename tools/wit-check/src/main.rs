use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use wit_parser::{Resolve, WorldId, WorldItem, WorldKey};

fn main() -> Result<()> {
    let root = PathBuf::from("wit");
    if !root.exists() {
        bail!("wit/ directory not found");
    }

    let mut resolve = Resolve::default();
    let (pkg_id, files) = resolve
        .push_dir(&root)
        .with_context(|| format!("failed to parse WIT dir {}", root.display()))?;

    let pkg = &resolve.packages[pkg_id];
    println!(
        "Parsed package {}:{} with {} interfaces and {} worlds",
        pkg.name.namespace,
        pkg.name.name,
        pkg.interfaces.len(),
        pkg.worlds.len()
    );
    for (name, world_id) in &pkg.worlds {
        dump_world(&resolve, *world_id, name);
    }
    println!("Files processed:");
    for path in files {
        println!("  {}", path.display());
    }
    Ok(())
}

fn dump_world(resolve: &Resolve, world_id: WorldId, alias: &str) {
    let world = &resolve.worlds[world_id];
    println!("  world {} (alias {alias})", world.name);
    for (name, item) in &world.imports {
        println!(
            "    import {} -> {}",
            key_to_string(name),
            describe_item(item)
        );
    }
    for (name, item) in &world.exports {
        println!(
            "    export {} -> {}",
            key_to_string(name),
            describe_item(item)
        );
    }
}

fn key_to_string(key: &WorldKey) -> String {
    match key {
        WorldKey::Name(name) => name.clone(),
        WorldKey::Interface(id) => format!("interface-{}", id.index()),
    }
}

fn describe_item(item: &WorldItem) -> String {
    match item {
        WorldItem::Interface(id) => format!("interface {}", id.index()),
        WorldItem::Function(func) => format!("func {}", func.name),
        WorldItem::Type(id) => format!("type {}", id.index()),
    }
}
