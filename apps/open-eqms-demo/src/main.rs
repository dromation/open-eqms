pub mod asset_model;
pub mod clock;
pub mod crypto;
pub mod ids;

#[cfg(test)]
mod tests;

const PRE_ALPHA_BANNER: &str =
    "Open-EQMS demo: pre-alpha, local, single-process, non-production reference application.";

fn main() {
    println!("{PRE_ALPHA_BANNER}");
    println!("VS-001 implementation scaffold. Engine orchestration is added in later commits.");
}
