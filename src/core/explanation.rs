use crate::core::Resource;

#[derive(Debug)]
pub struct Explanation {
    pub resource: Resource,
    pub owned_by: Vec<String>,
    pub used_by: Vec<String>,
    pub required_by: Vec<String>,
    pub depends_on: Vec<String>,
    pub source: Option<String>,
    pub status: String,
}

impl Explanation {
    pub fn format(&self) -> String {
        let mut output = format!(
            "{}\n",
            self.resource
                .display_name
                .as_deref()
                .unwrap_or(&self.resource.native_id)
        );

        if !self.owned_by.is_empty() {
            output.push_str("Owned by:\n");
            for owner in &self.owned_by {
                output.push_str(&format!("  - {}\n", owner));
            }
        }

        if !self.used_by.is_empty() {
            output.push_str("Used by:\n");
            for user in &self.used_by {
                output.push_str(&format!("  - {}\n", user));
            }
        }

        if !self.required_by.is_empty() {
            output.push_str("Required by:\n");
            for req in &self.required_by {
                output.push_str(&format!("  - {}\n", req));
            }
        }

        if !self.depends_on.is_empty() {
            output.push_str("Depends on:\n");
            for dep in &self.depends_on {
                output.push_str(&format!("  - {}\n", dep));
            }
        }

        if let Some(ref source) = self.source {
            output.push_str(&format!("Source: {}\n", source));
        }

        output.push_str(&format!("Status: {}\n", self.status));
        output
    }
}
