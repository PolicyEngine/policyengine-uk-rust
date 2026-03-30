use std::io::BufRead;
use crate::data::Dataset;
use crate::data::clean::{parse_persons_csv, parse_benunits_csv, parse_households_csv, assemble_dataset};

/// Separator lines in the concatenated CSV protocol.
const SEP_PERSONS: &str = "===PERSONS===";
const SEP_BENUNITS: &str = "===BENUNITS===";
const SEP_HOUSEHOLDS: &str = "===HOUSEHOLDS===";

/// Load a Dataset from the concatenated CSV protocol read from a buffered reader.
///
/// Expected format:
/// ```text
/// ===PERSONS===
/// person_id,benunit_id,...
/// 0,0,...
/// ===BENUNITS===
/// benunit_id,household_id,...
/// 0,0,...
/// ===HOUSEHOLDS===
/// household_id,benunit_ids,...
/// 0,0;1,...
/// ```
pub fn load_dataset_from_reader<R: BufRead>(reader: R, year: u32) -> anyhow::Result<Dataset> {
    let mut persons_buf = Vec::new();
    let mut benunits_buf = Vec::new();
    let mut households_buf = Vec::new();

    #[derive(PartialEq)]
    enum Section { None, Persons, BenUnits, Households }
    let mut current = Section::None;

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed == SEP_PERSONS {
            current = Section::Persons;
            continue;
        } else if trimmed == SEP_BENUNITS {
            current = Section::BenUnits;
            continue;
        } else if trimmed == SEP_HOUSEHOLDS {
            current = Section::Households;
            continue;
        }
        match current {
            Section::Persons => { persons_buf.extend_from_slice(line.as_bytes()); persons_buf.push(b'\n'); }
            Section::BenUnits => { benunits_buf.extend_from_slice(line.as_bytes()); benunits_buf.push(b'\n'); }
            Section::Households => { households_buf.extend_from_slice(line.as_bytes()); households_buf.push(b'\n'); }
            Section::None => {} // skip lines before first separator
        }
    }

    if persons_buf.is_empty() {
        anyhow::bail!("No ===PERSONS=== section found in stdin data");
    }
    if benunits_buf.is_empty() {
        anyhow::bail!("No ===BENUNITS=== section found in stdin data");
    }
    if households_buf.is_empty() {
        anyhow::bail!("No ===HOUSEHOLDS=== section found in stdin data");
    }

    let people = parse_persons_csv(persons_buf.as_slice())?;
    let benunits = parse_benunits_csv(benunits_buf.as_slice())?;
    let households = parse_households_csv(households_buf.as_slice())?;

    Ok(assemble_dataset(people, benunits, households, year))
}
