//! List use case.

/// Use case: enumerate templates and features.
pub struct ListService;

impl Default for ListService {
    fn default() -> Self {
        Self::new()
    }
}

impl ListService {
    /// Construct a new service.
    pub fn new() -> Self {
        Self
    }
}
