fn main() {
    let t = std::time::Instant::now();
    let d = ffh_data::global();
    println!("loaded data in {:?}: {} units, {} promotions, {} spells, {} buildings, {} techs; {} warnings",
        t.elapsed(), d.units.len(), d.promotions.len(), d.spells.len(), d.buildings.len(), d.techs.len(), d.warnings.len());
    for w in d.warnings.iter().take(30) {
        println!("  warn: {w}");
    }
}
