//! Ouvre des images de disquette et affiche leur format, ou l'erreur.
//!
//!     cargo run -p trs80 --example diskinfo -- <image>...
//!     cargo run -p trs80 --example diskinfo -- --list <fichier contenant un chemin par ligne>

use trs80::Disk;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--list") {
        let list = std::fs::read_to_string(&args[1]).expect("liste");
        args = list
            .lines()
            .map(|l| l.trim_start_matches('\u{feff}').to_string())
            .filter(|l| !l.is_empty())
            .collect();
    }
    let (mut ok, mut failed) = (0, 0);
    for path in &args {
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => {
                println!("ILLISIBLE  {path} : {e}");
                continue;
            }
        };
        let head: Vec<String> = data.iter().take(16).map(|b| format!("{b:02X}")).collect();
        match Disk::open(data.clone()) {
            Ok(d) => {
                ok += 1;
                if args.len() < 20 {
                    println!("{:4} {:5} secteurs  {path}", d.format().name(), d.sector_count());
                }
            }
            Err(e) => {
                failed += 1;
                println!("ÉCHEC  {e}  ({} octets, en-tête {})  {path}", data.len(), head.join(" "));
            }
        }
    }
    println!("{ok} ouvertes, {failed} en échec");
}
