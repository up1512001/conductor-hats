//! What one `hats` invocation did, and the assertions worth making about it.
//!
//! Kept apart from the sandbox so that reading either one is a page rather than
//! a scroll.

pub struct Run {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    pub fn out(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(self.stdout.trim())
            .unwrap_or_else(|error| panic!("expected JSON, got {error}:\n{}", self.out()))
    }

    pub fn ok(&self) -> &Self {
        assert_eq!(
            self.status,
            0,
            "expected success, got {}:\n{}",
            self.status,
            self.out()
        );
        self
    }

    pub fn failed(&self) -> &Self {
        assert_ne!(self.status, 0, "expected a non-zero exit:\n{}", self.out());
        self
    }

    pub fn says(&self, needle: &str) -> &Self {
        assert!(
            self.out().contains(needle),
            "expected output containing {needle:?}, got:\n{}",
            self.out()
        );
        self
    }

    pub fn silent_about(&self, needle: &str) -> &Self {
        assert!(
            !self.out().contains(needle),
            "expected output without {needle:?}, got:\n{}",
            self.out()
        );
        self
    }
}
