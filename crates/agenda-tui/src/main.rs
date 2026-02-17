use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let db_path = match resolve_db_path() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    };

    if let Err(err) = agenda_tui::run(&db_path) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn resolve_db_path() -> Result<PathBuf, String> {
    let mut args = env::args().skip(1);
    let mut explicit: Option<PathBuf> = None;

    while let Some(arg) = args.next() {
        if arg == "--db" {
            let value = args.next().ok_or("--db requires a value".to_string())?;
            explicit = Some(PathBuf::from(value));
            continue;
        }

        return Err(format!("unexpected argument: {arg}"));
    }

    let path = if let Some(path) = explicit {
        path
    } else if let Ok(env_path) = env::var("AGENDA_DB") {
        PathBuf::from(env_path)
    } else {
        let home = env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
        PathBuf::from(home).join(".agenda").join("default.ag")
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    Ok(path)
}
