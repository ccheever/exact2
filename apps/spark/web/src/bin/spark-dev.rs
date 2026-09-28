//! The resident dev driver for Spark: `spark-dev <source> <out>`.
//! Run through `bun host/web/dev.mjs --app spark`, which pushes each plan to the page.

fn main() -> std::process::ExitCode {
    exact_web::dev::main::<spark_data::Profiles>()
}
