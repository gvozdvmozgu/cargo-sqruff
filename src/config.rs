use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
};

use rustc_session::Session;
use sqruff_lib::core::config::{FluffConfig, Value};
use toml::Table;

pub(crate) type SqruffConfig = Result<ConfigSpec, String>;

#[derive(Clone, Debug)]
pub(crate) struct ConfigSpec {
    metadata: Option<MetadataConfig>,
}

#[derive(Clone, Debug)]
struct MetadataConfig {
    manifest_path: PathBuf,
    sqruff_source: String,
    entries: Vec<ConfigEntry>,
}

#[derive(Clone, Debug)]
struct ConfigEntry {
    section_path: Vec<String>,
    key: String,
    value: String,
}

pub(crate) fn sqruff_config(sess: &Session) -> SqruffConfig {
    sqruff_config_for_manifest(find_manifest(sess).as_deref())
}

pub(crate) fn build_config(spec: &SqruffConfig) -> Result<FluffConfig, String> {
    let spec = spec.as_ref().map_err(Clone::clone)?;
    let mut config = FluffConfig::from_root(None, false, None)
        .map_err(|err| format!("failed to load sqruff config: {err}"))?;

    if let Some(metadata) = &spec.metadata {
        validate_sqruff_source(&metadata.sqruff_source, &metadata.manifest_path)?;
        let metadata_values = metadata_entries_to_sqruff_values(&metadata.entries)?;
        merge_values(&mut config.raw, metadata_values);
        return build_fluff_config(config.raw);
    }

    Ok(config)
}

fn sqruff_config_for_manifest(manifest_path: Option<&Path>) -> SqruffConfig {
    let Some(manifest_path) = manifest_path else {
        return Ok(ConfigSpec { metadata: None });
    };

    let Some(table) = metadata_sqruff_table(manifest_path)? else {
        return Ok(ConfigSpec { metadata: None });
    };

    Ok(ConfigSpec {
        metadata: Some(MetadataConfig {
            manifest_path: manifest_path.to_owned(),
            sqruff_source: metadata_table_to_sqruff_source(&table)?,
            entries: metadata_table_to_entries(&table)?,
        }),
    })
}

fn find_manifest(sess: &Session) -> Option<PathBuf> {
    let working_dir = sess
        .source_map()
        .working_dir()
        .local_path()
        .map(Path::to_path_buf);

    if let Some(manifest_path) = working_dir
        .as_ref()
        .map(|path| path.join("Cargo.toml"))
        .filter(|path| path.is_file())
    {
        return Some(manifest_path);
    }

    if let Some(manifest_path) = sess
        .local_crate_source_file()
        .and_then(|file_name| file_name.local_path().map(Path::to_path_buf))
        .and_then(|path| find_manifest_from(path.as_path()))
    {
        return Some(manifest_path);
    }

    working_dir.and_then(|path| find_manifest_from(path.as_path()))
}

fn find_manifest_from(path: &Path) -> Option<PathBuf> {
    let mut dir = if path.is_dir() { path } else { path.parent()? };

    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.is_file() {
            return Some(candidate);
        }

        dir = dir.parent()?;
    }
}

fn metadata_sqruff_table(manifest_path: &Path) -> Result<Option<Table>, String> {
    let manifest = std::fs::read_to_string(manifest_path)
        .map_err(|err| format!("failed to read `{}`: {err}", manifest_path.display()))?;
    let manifest = manifest
        .parse::<Table>()
        .map_err(|err| format!("failed to parse `{}`: {err}", manifest_path.display()))?;

    let Some(metadata) = manifest
        .get("package")
        .and_then(|package| package.get("metadata"))
        .and_then(|metadata| metadata.get("sqruff"))
    else {
        return Ok(None);
    };

    let table = metadata
        .as_table()
        .ok_or_else(|| "`package.metadata.sqruff` must be a table".to_owned())?;

    Ok(Some(table.clone()))
}

fn metadata_table_to_sqruff_source(table: &Table) -> Result<String, String> {
    let mut sections = Vec::new();
    append_source_section(&mut sections, vec!["sqruff".to_owned()], table)?;

    Ok(sections.join("\n"))
}

fn append_source_section(
    sections: &mut Vec<String>,
    section_path: Vec<String>,
    table: &Table,
) -> Result<(), String> {
    let mut lines = Vec::new();

    for (key, value) in table {
        match value {
            toml::Value::Table(nested) => {
                let mut nested_section_path = section_path.clone();
                nested_section_path.push(key.to_owned());
                append_source_section(sections, nested_section_path, nested)?;
            }
            value => {
                let value =
                    config_value(value).map_err(|err| format!("invalid `{key}` value: {err}"))?;
                lines.push(format!("{key} = {value}"));
            }
        }
    }

    if !lines.is_empty() {
        lines.sort();
        let mut section = format!("[{}]\n", section_path.join(":"));
        section.push_str(&lines.join("\n"));
        section.push('\n');
        sections.push(section);
    }

    Ok(())
}

fn metadata_table_to_entries(table: &Table) -> Result<Vec<ConfigEntry>, String> {
    let mut entries = Vec::new();
    append_entries(&mut entries, Vec::new(), table)?;
    Ok(entries)
}

fn append_entries(
    entries: &mut Vec<ConfigEntry>,
    section_path: Vec<String>,
    table: &Table,
) -> Result<(), String> {
    for (key, value) in table {
        match value {
            toml::Value::Table(nested) => {
                let mut nested_section_path = section_path.clone();
                nested_section_path.push(key.to_owned());
                append_entries(entries, nested_section_path, nested)?;
            }
            value => {
                entries.push(ConfigEntry {
                    section_path: section_path.clone(),
                    key: key.to_owned(),
                    value: config_value(value)
                        .map_err(|err| format!("invalid `{key}` value: {err}"))?,
                });
            }
        }
    }

    Ok(())
}

fn config_value(value: &toml::Value) -> Result<String, String> {
    match value {
        toml::Value::String(value) => Ok(value.to_owned()),
        toml::Value::Integer(value) => Ok(value.to_string()),
        toml::Value::Float(value) => Ok(value.to_string()),
        toml::Value::Boolean(value) => Ok(value.to_string()),
        toml::Value::Array(values) => values
            .iter()
            .map(config_array_value)
            .collect::<Result<Vec<_>, _>>()
            .map(|values| values.join(",")),
        toml::Value::Datetime(_) => Err("datetimes are not supported".to_owned()),
        toml::Value::Table(_) => unreachable!("nested tables are handled before conversion"),
    }
}

fn config_array_value(value: &toml::Value) -> Result<String, String> {
    match value {
        toml::Value::String(value) => Ok(value.to_owned()),
        toml::Value::Integer(value) => Ok(value.to_string()),
        toml::Value::Float(value) => Ok(value.to_string()),
        toml::Value::Boolean(value) => Ok(value.to_string()),
        toml::Value::Array(_) => Err("nested arrays are not supported".to_owned()),
        toml::Value::Datetime(_) => Err("datetimes are not supported".to_owned()),
        toml::Value::Table(_) => Err("tables inside arrays are not supported".to_owned()),
    }
}

fn metadata_entries_to_sqruff_values(
    entries: &[ConfigEntry],
) -> Result<hashbrown::HashMap<String, Value>, String> {
    let mut values = hashbrown::HashMap::new();

    for entry in entries {
        let section_path = if entry.section_path.is_empty() {
            &["core".to_owned()][..]
        } else {
            entry.section_path.as_slice()
        };
        insert_value(
            &mut values,
            section_path,
            entry.key.clone(),
            entry.value.parse().unwrap(),
        )?;
    }

    Ok(values)
}

fn insert_value(
    values: &mut hashbrown::HashMap<String, Value>,
    path: &[String],
    key: String,
    value: Value,
) -> Result<(), String> {
    let Some((head, tail)) = path.split_first() else {
        values.insert(key, value);
        return Ok(());
    };

    let entry = values
        .entry(head.to_owned())
        .or_insert_with(|| Value::Map(hashbrown::HashMap::new()));
    let map = entry
        .as_map_mut()
        .ok_or_else(|| format!("`{head}` cannot be both a value and a section"))?;

    insert_value(map, tail, key, value)
}

fn validate_sqruff_source(source: &str, manifest_path: &Path) -> Result<(), String> {
    let _ = manifest_path;
    catch_unwind(AssertUnwindSafe(|| {
        FluffConfig::from_source(source, None);
    }))
    .map_err(|err| panic_message(err, "sqruff rejected `package.metadata.sqruff`"))?;

    Ok(())
}

fn build_fluff_config(raw: hashbrown::HashMap<String, Value>) -> Result<FluffConfig, String> {
    catch_unwind(AssertUnwindSafe(|| FluffConfig::new(raw, None, None)))
        .map_err(|err| panic_message(err, "failed to build sqruff config"))
}

fn panic_message(err: Box<dyn std::any::Any + Send>, fallback: &str) -> String {
    if let Some(message) = err.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = err.downcast_ref::<&str>() {
        (*message).to_owned()
    } else {
        fallback.to_owned()
    }
}

fn merge_values(
    target: &mut hashbrown::HashMap<String, Value>,
    source: hashbrown::HashMap<String, Value>,
) {
    for (key, source_value) in source {
        match (target.get_mut(&key), source_value) {
            (Some(Value::Map(target)), Value::Map(source)) => merge_values(target, source),
            (_, source_value) => {
                target.insert(key, source_value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn table(source: &str) -> Table {
        source.parse::<Table>().unwrap()
    }

    fn temp_manifest(contents: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("cargo-sqruff-test-{unique}"));
        fs::create_dir_all(&dir).unwrap();

        let manifest = dir.join("Cargo.toml");
        fs::write(&manifest, contents).unwrap();
        manifest
    }

    #[test]
    fn converts_top_level_scalars() {
        let source = metadata_table_to_sqruff_source(&table(
            r#"
dialect = "postgres"
rules = "core"
exclude_rules = "LT12"
"#,
        ))
        .unwrap();

        assert!(source.contains("[sqruff]"));
        assert!(source.contains("dialect = postgres"));
        assert!(source.contains("rules = core"));
        assert!(source.contains("exclude_rules = LT12"));
    }

    #[test]
    fn converts_nested_sections() {
        let source = metadata_table_to_sqruff_source(&table(
            r#"
[indentation]
tab_space_size = 2
indented_joins = true
"#,
        ))
        .unwrap();

        assert!(source.contains("[sqruff:indentation]"));
        assert!(source.contains("tab_space_size = 2"));
        assert!(source.contains("indented_joins = true"));
    }

    #[test]
    fn converts_deeply_nested_sections() {
        let source = metadata_table_to_sqruff_source(&table(
            r#"
[layout.type.comma]
spacing_before = "touch"
line_position = "trailing"
"#,
        ))
        .unwrap();

        assert!(source.contains("[sqruff:layout:type:comma]"));
        assert!(source.contains("spacing_before = touch"));
        assert!(source.contains("line_position = trailing"));
    }

    #[test]
    fn converts_scalar_arrays() {
        let source = metadata_table_to_sqruff_source(&table(
            r#"
exclude_rules = ["LT12", "LT13"]
"#,
        ))
        .unwrap();

        assert!(source.contains("exclude_rules = LT12,LT13"));
    }

    #[test]
    fn rejects_unsupported_values() {
        let err = metadata_table_to_sqruff_source(&table(
            r#"
unsupported = 1979-05-27T07:32:00Z
"#,
        ))
        .unwrap_err();

        assert!(err.contains("datetimes are not supported"));
    }

    #[test]
    fn rejects_nested_arrays() {
        let err = metadata_table_to_sqruff_source(&table(
            r#"
exclude_rules = [["LT12"]]
"#,
        ))
        .unwrap_err();

        assert!(err.contains("nested arrays are not supported"));
    }

    #[test]
    fn no_metadata_uses_default_config() {
        let manifest = temp_manifest(
            r#"
[package]
name = "example"
version = "0.1.0"
edition = "2024"
"#,
        );

        let spec = sqruff_config_for_manifest(Some(&manifest));
        let config = build_config(&spec).unwrap();

        assert_eq!(config.raw["core"]["rules"].as_string(), Some("core"));
    }

    #[test]
    fn metadata_overrides_default_config() {
        let manifest = temp_manifest(
            r#"
[package]
name = "example"
version = "0.1.0"
edition = "2024"

[package.metadata.sqruff]
dialect = "postgres"
"#,
        );

        let spec = sqruff_config_for_manifest(Some(&manifest));
        let config = build_config(&spec).unwrap();

        assert_eq!(config.raw["core"]["dialect"].as_string(), Some("postgres"));
    }

    #[test]
    fn metadata_nested_values_apply_to_config() {
        let manifest = temp_manifest(
            r#"
[package]
name = "example"
version = "0.1.0"
edition = "2024"

[package.metadata.sqruff.indentation]
tab_space_size = 2
"#,
        );

        let spec = sqruff_config_for_manifest(Some(&manifest));
        let config = build_config(&spec).unwrap();

        assert_eq!(
            config.raw["indentation"]["tab_space_size"].as_int(),
            Some(2)
        );
        assert_eq!(config.raw["core"]["rules"].as_string(), Some("core"));
    }

    #[test]
    fn metadata_arrays_are_normalized_by_sqruff_config() {
        let manifest = temp_manifest(
            r#"
[package]
name = "example"
version = "0.1.0"
edition = "2024"

[package.metadata.sqruff]
exclude_rules = ["LT12", "LT13"]
"#,
        );

        let spec = sqruff_config_for_manifest(Some(&manifest));
        let config = build_config(&spec).unwrap();
        let rule_denylist = config.raw["core"]["rule_denylist"].as_array().unwrap();

        assert_eq!(rule_denylist[0].as_string(), Some("LT12"));
        assert_eq!(rule_denylist[1].as_string(), Some("LT13"));
    }

    #[test]
    fn malformed_metadata_returns_error() {
        let manifest = temp_manifest(
            r#"
[package]
name = "example"
version = "0.1.0"
edition = "2024"

[package.metadata]
sqruff = 1
"#,
        );

        let err = sqruff_config_for_manifest(Some(&manifest)).unwrap_err();

        assert!(err.contains("package.metadata.sqruff"));
    }
}
