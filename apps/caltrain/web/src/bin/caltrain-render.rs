//! Caltrain's pages as HTML documents (LLP 1048.000 D1):
//! `caltrain-render [--plan <app.plan>] <location>…`, one JSON line each.
//! The web build and the document parity check run it.

fn main() -> std::process::ExitCode {
    exact_web::document::main::<caltrain_data::Caltrain>(caltrain_web::PLAN)
}
