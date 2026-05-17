//! List use case (built out in P5).

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
