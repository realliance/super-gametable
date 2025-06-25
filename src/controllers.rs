pub enum GameController {
    Embedded(String),
    /// TODO
    ///
    /// Implement network based controller once libmahjong-rs supports
    /// FFI controller registration
    External,
}

impl ToString for GameController {
    fn to_string(&self) -> String {
        match self {
            GameController::Embedded(name) => name.clone(),
            GameController::External => "External".to_string(),
        }
    }
}
