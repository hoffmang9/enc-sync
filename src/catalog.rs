use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
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
                    "cell" | "Cell" => {
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
                    "cell" | "Cell" => {
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
    use chrono::DateTime;
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

fn parse_ienc_timestamp(date: &str, time: &str) -> Result<i64> {
    let date = NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d")
        .with_context(|| format!("parsing IENC date '{date}'"))?;
    let time = if time.trim().is_empty() {
        NaiveTime::from_hms_opt(0, 0, 0).unwrap()
    } else {
        NaiveTime::parse_from_str(time.trim(), "%H:%M:%S")
            .with_context(|| format!("parsing IENC time '{time}'"))?
    };
    Ok(NaiveDateTime::new(date, time).and_utc().timestamp())
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
    timestamp_error: Option<anyhow::Error>,
    states: Vec<String>,
    regions: Vec<String>,
    coast_guard_districts: Vec<String>,
    current_field: Option<String>,
    current_text: String,
    in_s57_file: bool,
    s57_url: Option<String>,
    s57_date: Option<String>,
    s57_time: Option<String>,
}

impl CellBuilder {
    fn start_field(&mut self, field: &str) {
        if field == "s57_file" {
            self.in_s57_file = true;
        }
        self.current_field = Some(field.to_string());
        self.current_text.clear();
    }

    fn end_field(&mut self, field: &str) {
        if field == "s57_file" {
            self.in_s57_file = false;
            self.current_field = None;
            self.current_text.clear();
            return;
        }

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
            "zipfile_datetime_iso8601" => match parse_catalog_timestamp(value.trim()) {
                Ok(timestamp) => self.timestamp = Some(timestamp),
                Err(error) => self.timestamp_error = Some(error),
            },
            "location" if self.in_s57_file => self.s57_url = Some(value.trim().to_string()),
            "date_posted" if self.in_s57_file => self.s57_date = Some(value.trim().to_string()),
            "time_posted" if self.in_s57_file => self.s57_time = Some(value.trim().to_string()),
            _ => {}
        }
    }

    fn push_text(&mut self, value: String) {
        self.current_text.push_str(&value);
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
        if let Some(error) = self.timestamp_error {
            let name = self.name.as_deref().unwrap_or("<unknown>");
            return Err(error.context(format!("invalid timestamp for catalog cell {name}")));
        }

        let Some(name) = self.name else {
            return Ok(None);
        };

        let url = if let Some(url) = self.url {
            url
        } else if let Some(url) = self.s57_url {
            url
        } else {
            format!("{name}.zip")
        };

        let timestamp = if let Some(timestamp) = self.timestamp {
            timestamp
        } else if let Some(date) = self.s57_date {
            parse_ienc_timestamp(&date, self.s57_time.as_deref().unwrap_or(""))?
        } else {
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

    #[test]
    fn parse_catalog_reports_invalid_required_timestamp() {
        let dir = TempDir::new().unwrap();
        let catalog_path = dir.path().join("ENCProdCat.xml");
        fs::write(
            &catalog_path,
            r#"<?xml version="1.0"?>
<catalog>
  <cell>
    <name>US5CA01M</name>
    <zipfile_location>https://charts.noaa.gov/US5CA01M.zip</zipfile_location>
    <zipfile_datetime_iso8601>not-a-date</zipfile_datetime_iso8601>
  </cell>
</catalog>
"#,
        )
        .unwrap();

        let error = parse_catalog(&catalog_path).unwrap_err();
        assert!(
            error.to_string().contains("invalid timestamp"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn parse_catalog_reads_ienc_cells() {
        let dir = TempDir::new().unwrap();
        let catalog_path = dir.path().join("IENCU37ProductsCatalog.xml");
        fs::write(
            &catalog_path,
            r#"<?xml version="1.0"?>
<IENCU37ProductCatalog>
  <Cell>
    <name>AR010001</name>
    <s57_file>
      <location>https://example.test/AR010001.zip</location>
      <date_posted>2024-06-01</date_posted>
      <time_posted>12:34:56</time_posted>
    </s57_file>
  </Cell>
</IENCU37ProductCatalog>
"#,
        )
        .unwrap();

        let cells = parse_catalog(&catalog_path).unwrap();
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].name, "AR010001");
        assert_eq!(cells[0].url, "https://example.test/AR010001.zip");
    }
}
