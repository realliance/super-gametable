//! Enrollment endpoints and channel management for the Super Gametable.

use table::EnrollmentTable;

pub mod routes;
pub mod table;

pub struct EnrollmentServer {
    table: Box<dyn EnrollmentTable>,
}
