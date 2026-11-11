#[cfg(test)]
mod tests {
    use crate::scan::{default_plugin_directories, find_ofx_binaries, load_ofx_library};

    #[test]
    fn test_discover_default_directories() {
        let dirs = default_plugin_directories();
        assert!(!dirs.is_empty(), "Expected at least one standard OFX directory");
    }

    #[test]
    fn test_find_binaries_in_system() {
        let dirs = default_plugin_directories();
        let mut total_binaries = Vec::new();
        for d in dirs {
            let found = find_ofx_binaries(&d);
            total_binaries.extend(found);
        }
        println!("Found {} OFX binaries on this system", total_binaries.len());
    }

    #[test]
    fn test_load_universe_plugins() {
        let path = std::path::Path::new(r"C:\Program Files\Common Files\OFX\Plugins\Red Giant Universe\RG Universe.ofx.bundle\Contents\Win64\RG Universe.ofx");
        if path.exists() {
            println!("Testing load of RG Universe.ofx...");
            match load_ofx_library(path) {
                Ok(effects) => {
                    println!("Successfully loaded {} effects from RG Universe!", effects.len());
                    for e in effects.iter().take(10) {
                        println!(
                            "  Effect: '{}' | id: '{}' | category: '{}' | params: {}",
                            e.manifest.name,
                            e.manifest.id,
                            e.manifest.category,
                            e.manifest.params.len()
                        );
                    }
                }
                Err(e) => println!("Failed to load RG Universe: {}", e),
            }
        }
    }
}
