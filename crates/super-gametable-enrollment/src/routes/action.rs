use axum::{Json, extract::State};

use crate::EnrollmentServer;

struct ActionBody {
    pub action: String,
    pub data: String,
}

async fn action_handler(
    State(mut enrollment_state): State<EnrollmentServer>,
    Json(body): Json<ActionBody>,
) {
    todo!();
}
