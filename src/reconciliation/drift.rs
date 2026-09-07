use crate::errors::Result;

#[derive(Debug)]
pub struct DriftReport {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<String>,
}

pub fn detect_drift() -> Result<DriftReport> {
    Ok(DriftReport {
        added: Vec::new(),
        removed: Vec::new(),
        modified: Vec::new(),
    })
}
