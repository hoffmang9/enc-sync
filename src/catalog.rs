use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use chrono::DateTime;
use quick_xml::events::Event;
use quick_xml::Reader;

#[derive(Debug, Clone)]
pub struct Cell {
    pub name: String,
    pub url: String,
    pub timestamp: i64,
    pub states: Vec<String>,
    pub regions: Vec<String>,
    pub coast_guard_districts: Vec<String>,
}

pub fn parse_catalog(catalog_path: &Path) -> Result<Vec<Cell>> {
    let xml = fs::read_to_string(catalog_path)?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);

    let mut cells = Vec::new();
    let mut buf = Vec::new();
    let mut in_cell = false;
    let mut current: Option<CellBuilder> = None;
    let mut nested = None::<NestedField>;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(tag)) => {
                let name = tag.name().as_ref().to_vec();
                let tag_name = String::from_utf8_lossy(&name).into_owned();
                match tag_name.as_str() {
                    "cell" => {
                        in_cell = true;
                        current = Some(CellBuilder::default());
                    }
                    "state" if in_cell => nested = Some(NestedField::State),
                    "region" if in_cell => nested = Some(NestedField::Region),
                    "coast_guard_district" if in_cell => nested = Some(NestedField::Cgd),
                    field if in_cell => {
                        if let Some(builder) = current.as_mut() {
                            builder.start_field(field);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(text)) => {
                let value = text.unescape()?.into_owned();
                if let Some(field) = nested.take() {
                    if let Some(builder) = current.as_mut() {
                        builder.push_nested(field, value);
                    }
                    continue;
                }
                if in_cell {
                    if let Some(builder) = current.as_mut() {
                        builder.push_text(value);
                    }
                }
            }
            Ok(Event::End(tag)) => {
                let name = tag.name().as_ref().to_vec();
                let tag_name = String::from_utf8_lossy(&name).into_owned();
                match tag_name.as_str() {
                    "state" | "region" | "coast_guard_district" => {
                        nested = None;
                    }
                    "cell" => {
                        if let Some(builder) = current.take() {
                            if let Some(cell) = builder.finish()? {
                                cells.push(cell);
                            }
                        }
                        in_cell = false;
                    }
                    field if in_cell => {
                        if let Some(builder) = current.as_mut() {
                            builder.end_field(field);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buf.clear();
    }

    Ok(cells)
}

fn parse_catalog_timestamp(raw: &str) -> Result<i64> {
    let raw = raw.trim();
    if let Ok(parsed) = DateTime::parse_from_rfc3339(raw) {
        return Ok(parsed.timestamp());
    }
    if let Some(stripped) = raw.strip_suffix('Z') {
        let parsed = DateTime::parse_from_rfc3339(&format!("{stripped}+00:00"))
            .with_context(|| format!("parsing catalog timestamp '{raw}'"))?;
        return Ok(parsed.timestamp());
    }
    bail!("parsing catalog timestamp '{raw}'")
}

#[derive(Clone, Copy)]
enum NestedField {
    State,
    Region,
    Cgd,
}

#[derive(Default)]
struct CellBuilder {
    name: Option<String>,
    url: Option<String>,
    timestamp: Option<i64>,
    states: Vec<String>,
    regions: Vec<String>,
    coast_guard_districts: Vec<String>,
    current_field: Option<String>,
    current_text: String,
}

impl CellBuilder {
    fn start_field(&mut self, field: &str) {
        self.current_field = Some(field.to_string());
        self.current_text.clear();
    }

    fn push_text(&mut self, value: String) {
        self.current_text.push_str(&value);
    }

    fn end_field(&mut self, field: &str) {
        let Some(active) = self.current_field.as_deref() else {
            return;
        };
        if active != field {
            return;
        }
        let value = std::mem::take(&mut self.current_text);
        self.current_field = None;
        if value.trim().is_empty() {
            return;
        }
        match field {
            "name" => self.name = Some(value.trim().to_string()),
            "zipfile_location" => self.url = Some(value.trim().to_string()),
            "zipfile_datetime_iso8601" => {
                self.timestamp = parse_catalog_timestamp(value.trim()).ok();
            }
            _ => {}
        }
    }

    fn push_nested(&mut self, field: NestedField, value: String) {
        let value = value.trim().to_string();
        if value.is_empty() {
            return;
        }
        match field {
            NestedField::State => self.states.push(value),
            NestedField::Region => self.regions.push(value),
            NestedField::Cgd => self.coast_guard_districts.push(value),
        }
    }

    fn finish(self) -> Result<Option<Cell>> {
        let (Some(name), Some(url), Some(timestamp)) = (self.name, self.url, self.timestamp) else {
            return Ok(None);
        };
        Ok(Some(Cell {
            name,
            url,
            timestamp,
            states: self.states,
            regions: self.regions,
            coast_guard_districts: self.coast_guard_districts,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn parse_catalog_timestamp_accepts_rfc3339_and_z_suffix() {
        let with_offset = parse_catalog_timestamp("2024-01-15T12:00:00+00:00").unwrap();
        let with_z = parse_catalog_timestamp("2024-01-15T12:00:00Z").unwrap();
        assert_eq!(with_offset, with_z);
    }

    #[test]
    fn parse_catalog_timestamp_rejects_invalid_input() {
        assert!(parse_catalog_timestamp("not-a-date").is_err());
    }

    #[test]
    fn parse_catalog_reads_cells_and_metadata() {
        let dir = TempDir::new().unwrap();
        let catalog_path = dir.path().join("ENCProdCat.xml");
        fs::write(
            &catalog_path,
            r#"<?xml version="1.0"?>
<catalog>
  <cell>
    <name>US5CA01M</name>
    <zipfile_location>https://charts.noaa.gov/US5CA01M.zip</zipfile_location>
    <zipfile_datetime_iso8601>2024-06-01T00:00:00Z</zipfile_datetime_iso8601>
    <state>CA</state>
    <region>11</region>
    <coast_guard_district>11</coast_guard_district>
  </cell>
  <cell>
    <name>US5OR02M</name>
    <zipfile_location>https://charts.noaa.gov/US5OR02M.zip</zipfile_location>
    <zipfile_datetime_iso8601>2024-06-02T00:00:00Z</zipfile_datetime_iso8601>
    <state>OR</state>
  </cell>
  <cell>
    <name>INCOMPLETE</name>
  </cell>
</catalog>
"#,
        )
        .unwrap();

        let cells = parse_catalog(&catalog_path).unwrap();
        assert_eq!(cells.len(), 2);

        let ca = cells.iter().find(|c| c.name == "US5CA01M").unwrap();
        assert_eq!(ca.url, "https://charts.noaa.gov/US5CA01M.zip");
        assert_eq!(ca.states, vec!["CA"]);
        assert_eq!(ca.regions, vec!["11"]);
        assert_eq!(ca.coast_guard_districts, vec!["11"]);
        assert_eq!(
            ca.timestamp,
            parse_catalog_timestamp("2024-06-01T00:00:00Z").unwrap()
        );
    }
}
