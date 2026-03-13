use anyhow::Result;
use ignore::WalkBuilder;
use syn::{Item, parse_file};

pub async fn get_repo_overview(workspace_root: &str) -> Result<String> {
    let max = 50;
    let include_sym = true;

    let walker = WalkBuilder::new(workspace_root)
        .standard_filters(true) // respects .gitignore, .ignore, etc.
        .build();

    let mut tree = String::from("# Repository Overview\n\n");
    let mut symbols = String::from("\n## Key Rust Symbols\n\n");

    let mut file_count = 0;
    for entry in walker {
        let entry = entry?;
        let path = entry.path();
        // skip directories or files that don't end in rs, toml, and md
        if path.is_dir()
            || !path
                .extension()
                .map_or(false, |e| e == "rs" || e == "toml" || e == "md")
        {
            continue;
        }

        let rel_path = path.strip_prefix(workspace_root)?.to_string_lossy();
        tree.push_str(&format!("- {}\n", rel_path));

        if include_sym && rel_path.ends_with(".rs") {
            if let Ok(content) = tokio::fs::read_to_string(path).await {
                if let Ok(file) = parse_file(&content) {
                    for item in file.items {
                        match item {
                            Item::Fn(f) => symbols
                                .push_str(&format!("- `{}` (fn in {})\n", f.sig.ident, rel_path)),
                            Item::Struct(s) => symbols
                                .push_str(&format!("- `{}` (struct in {})\n", s.ident, rel_path)),
                            Item::Trait(t) => symbols
                                .push_str(&format!("- `{}` (trait in {})\n", t.ident, rel_path)),
                            _ => {}
                        }
                    }
                }
            }
        }
        file_count += 1;
        if file_count >= max {
            break;
        }
    }

    Ok(format!("{}{}", tree, symbols))
}
