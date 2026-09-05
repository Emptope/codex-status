use std::{fs::File, io::Read, path::Path};

const MAX_CONFIG: u64 = 1024 * 1024;
const MAX_NAME: usize = 128;

fn label(value: &str) -> Option<String> {
    (!value.is_empty() && value.len() <= MAX_NAME && !value.chars().any(char::is_control))
        .then(|| value.to_owned())
}

pub fn provider(root: Option<&str>) -> Option<String> {
    let path = Path::new(root?).join("config.toml");
    let metadata = path.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > MAX_CONFIG {
        return None;
    }
    let mut source = String::new();
    File::open(path)
        .ok()?
        .take(MAX_CONFIG + 1)
        .read_to_string(&mut source)
        .ok()?;
    if source.len() as u64 > MAX_CONFIG {
        return None;
    }
    let config: toml::Value = toml::from_str(&source).ok()?;
    let id = config.get("model_provider")?.as_str()?;
    let name = config
        .get("model_providers")
        .and_then(|providers| providers.get(id))
        .and_then(|provider| provider.get("name"))
        .and_then(toml::Value::as_str)
        .unwrap_or(id);
    label(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn provider_reads_only_the_selected_display_name() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("config.toml"),
            r#"
model_provider = "gateway"

[model_providers.gateway]
name = "Work gateway"
base_url = "https://example.invalid"
env_key = "PRIVATE_KEY"

[model_providers.other]
name = "Other"
"#,
        )
        .unwrap();

        assert_eq!(provider(root.path().to_str()), Some("Work gateway".into()));
    }

    #[test]
    fn provider_falls_back_to_a_valid_identifier() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("config.toml"),
            "model_provider = \"gateway\"\n",
        )
        .unwrap();
        assert_eq!(provider(root.path().to_str()), Some("gateway".into()));

        fs::write(
            root.path().join("config.toml"),
            "model_provider = \"bad\\nlabel\"\n",
        )
        .unwrap();
        assert_eq!(provider(root.path().to_str()), None);
    }
}
